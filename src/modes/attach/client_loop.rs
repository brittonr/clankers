use std::io;

use clankers_controller::client::ClientAdapter;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tracing::info;
use tracing::warn;

use super::commands::AttachParityTracker;
use super::events::drain_daemon_events;
use super::handle_terminal_events;
use super::session::RecoveryMode;
use super::session::try_reconnect;
use super::session::try_recover_daemon;
use crate::config::keybindings::Keymap;
use crate::error::Result;
use crate::slash_commands;
use crate::tui::app::App;
use crate::tui::render;

/// Run the attach event loop with automatic reconnection on disconnect.
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        nested_conditionals,
        reason = "complex control flow — extracting helpers would obscure logic"
    )
)]
pub(crate) async fn run_attach_with_reconnect(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    mut client: ClientAdapter,
    keymap: Keymap,
    slash_registry: &slash_commands::SlashRegistry,
    max_subagent_panes: usize,
    socket_path: &str,
    session_id: &str,
    restore_mode: clanker_tui_types::ConnectionMode,
    recovery_mode: RecoveryMode,
) -> Result<()> {
    let mut is_replaying_history = true;
    let mut parity_tracker = AttachParityTracker::default();

    loop {
        terminal.draw(|frame| render::render(frame, app)).map_err(|e| crate::error::Error::Tui {
            message: format!("Render failed: {e}"),
        })?;

        if app.should_quit {
            client.disconnect();
            break;
        }

        // Drain daemon events
        drain_daemon_events(app, &mut client, &mut is_replaying_history, max_subagent_panes, &mut parity_tracker);

        // Detect disconnect and attempt reconnection
        if client.is_disconnected() && app.connection_mode != clanker_tui_types::ConnectionMode::Reconnecting {
            app.connection_mode = clanker_tui_types::ConnectionMode::Reconnecting;
            app.push_system("Connection to daemon lost. Attempting to reconnect...".to_string(), true);

            // First, try reconnecting to the existing socket (transient glitch).
            match try_reconnect(socket_path, session_id).await {
                Some(new_client) => {
                    finish_local_reconnect(
                        app,
                        &mut client,
                        new_client,
                        &mut is_replaying_history,
                        &mut parity_tracker,
                        restore_mode.clone(),
                    );
                }
                None => {
                    // Socket reconnect failed. For auto-daemon, try restarting
                    // the daemon and resuming the session.
                    match &recovery_mode {
                        RecoveryMode::AutoDaemon {
                            session_id: sid,
                            model,
                            cwd,
                        } => {
                            app.push_system("Restarting daemon...".to_string(), true);
                            match try_recover_daemon(sid, model, cwd).await {
                                Ok((new_client, new_socket_path, was_resumed)) => {
                                    client = new_client;
                                    client.replay_history();
                                    is_replaying_history = true;
                                    parity_tracker = AttachParityTracker::default();
                                    app.connection_mode = restore_mode.clone();
                                    if was_resumed {
                                        app.push_system("Session was_resumed after daemon restart.".to_string(), false);
                                    } else {
                                        app.push_system(
                                            "Session history lost — started fresh after daemon restart.".to_string(),
                                            true,
                                        );
                                    }
                                    info!("auto-daemon: recovered to new socket {new_socket_path}");
                                }
                                Err(e) => {
                                    warn!("auto-daemon: daemon recovery failed: {e}");
                                    app.push_system(format!("Daemon recovery failed: {e}. Use /quit to exit."), true);
                                }
                            }
                        }
                        RecoveryMode::ExplicitAttach => {
                            app.push_system(
                                "Failed to reconnect after multiple attempts. Use /quit to exit.".to_string(),
                                true,
                            );
                        }
                    }
                }
            }
        }

        // Handle terminal events (keys, mouse, paste)
        handle_terminal_events(app, &mut client, terminal, &keymap, slash_registry, &mut parity_tracker)?;

        if app.open_editor_requested {
            app.open_editor_requested = false;
            crate::tui::clipboard::open_external_editor(terminal, app);
        }
    }

    Ok(())
}

pub(crate) fn finish_local_reconnect(
    app: &mut App,
    client: &mut ClientAdapter,
    new_client: ClientAdapter,
    is_replaying_history: &mut bool,
    parity_tracker: &mut AttachParityTracker,
    restore_mode: clanker_tui_types::ConnectionMode,
) {
    *client = new_client;
    client.replay_history();
    *is_replaying_history = true;
    *parity_tracker = AttachParityTracker::default();
    app.connection_mode = restore_mode;
    app.push_system("Reconnected to daemon session.".to_string(), false);
}

#[cfg(test)]
mod tests {
    use clanker_tui_types::BlockEntry;
    use clanker_tui_types::ConnectionMode;
    use clankers_controller::client::ClientAdapter;
    use clankers_protocol::DaemonEvent;
    use clankers_tui::app::App;

    use super::finish_local_reconnect;
    use crate::modes::attach::AttachParityTracker;
    use crate::modes::attach::drain_daemon_events;

    fn test_app() -> App {
        App::new("test-model".to_string(), ".".to_string(), crate::config::theme::detect_theme())
    }

    fn client_with_events(events: Vec<DaemonEvent>) -> ClientAdapter {
        let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::unbounded_channel();
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        for event in events {
            event_tx.send(event).expect("event queued");
        }
        ClientAdapter::from_channels(cmd_tx, event_rx)
    }

    fn system_texts(app: &App) -> Vec<String> {
        app.conversation
            .blocks
            .iter()
            .filter_map(|entry| match entry {
                BlockEntry::System(message) => Some(message.content.clone()),
                BlockEntry::Conversation(_) => None,
            })
            .collect()
    }

    #[test]
    fn local_reconnect_resets_parity_tracker_before_new_events_arrive() {
        let mut app = test_app();
        app.connection_mode = ConnectionMode::Reconnecting;
        let mut client = client_with_events(vec![]);
        let reconnect_client = client_with_events(vec![DaemonEvent::SystemMessage {
            text: "Disabled tools updated: bash".to_string(),
            is_error: false,
        }]);
        let mut is_replaying_history = false;
        let mut parity_tracker = AttachParityTracker::default();
        parity_tracker.expect_disabled_tools_message();

        finish_local_reconnect(
            &mut app,
            &mut client,
            reconnect_client,
            &mut is_replaying_history,
            &mut parity_tracker,
            ConnectionMode::Attached,
        );
        drain_daemon_events(&mut app, &mut client, &mut is_replaying_history, 0, &mut parity_tracker);

        assert!(is_replaying_history);
        assert_eq!(app.connection_mode, ConnectionMode::Attached);
        let messages = system_texts(&app);
        assert!(messages.iter().any(|message| message == "Reconnected to daemon session."));
        assert!(messages.iter().any(|message| message == "Disabled tools updated: bash"));
    }
}
