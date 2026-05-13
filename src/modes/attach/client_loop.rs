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
                    client = new_client;
                    client.replay_history();
                    is_replaying_history = true;
                    parity_tracker = AttachParityTracker::default();
                    app.connection_mode = restore_mode.clone();
                    app.push_system("Reconnected to daemon session.".to_string(), false);
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
