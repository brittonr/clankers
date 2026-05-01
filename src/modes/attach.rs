//! TUI attach mode — connect to a daemon session via socket.
//!
//! Instead of running an in-process agent, the TUI reads `DaemonEvent`s from a
//! `ClientAdapter` connected to a daemon session socket. User input is forwarded
//! as `SessionCommand::Prompt`. Client-side commands (zoom, layout, theme, quit)
//! are handled locally; everything else goes to the daemon.

use std::io;
use std::path::Path;
use std::time::Duration;

use clankers_controller::client::ClientAdapter;
use clankers_controller::convert::daemon_event_to_tui_event;
use clankers_protocol::DaemonEvent;
use clankers_protocol::SessionCommand;
use clankers_protocol::control::ControlCommand;
use clankers_protocol::control::ControlResponse;
use clankers_protocol::frame;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::net::UnixStream;
use tracing::debug;
use tracing::info;
use tracing::warn;

use crate::config::keybindings::InputMode;
use crate::config::keybindings::Keymap;
use crate::config::settings::Settings;
use crate::config::theme::load_theme;
use crate::error::Result;
use crate::slash_commands;
use crate::tui::app::App;
use crate::tui::event as tui_event;
use crate::tui::event::AppEvent;
use crate::tui::render;

// ── Session socket connection ───────────────────────────────────────────────

const SESSION_SOCKET_CONNECT_ATTEMPTS: usize = 20;
const SESSION_SOCKET_CONNECT_RETRY_DELAY_MS: u64 = 25;

fn should_retry_session_socket_connect(error: &io::Error) -> bool {
    matches!(error.kind(), io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused)
}

pub(crate) async fn connect_session_socket(socket_path: impl AsRef<Path>) -> io::Result<UnixStream> {
    let socket_path = socket_path.as_ref();
    let retry_delay = Duration::from_millis(SESSION_SOCKET_CONNECT_RETRY_DELAY_MS);

    for attempt_index in 0..SESSION_SOCKET_CONNECT_ATTEMPTS {
        match UnixStream::connect(socket_path).await {
            Ok(stream) => return Ok(stream),
            Err(error)
                if should_retry_session_socket_connect(&error)
                    && attempt_index + 1 < SESSION_SOCKET_CONNECT_ATTEMPTS =>
            {
                tokio::time::sleep(retry_delay).await;
            }
            Err(error) => return Err(error),
        }
    }

    unreachable!("bounded retry loop must return success or final error");
}

// ── Entry point ─────────────────────────────────────────────────────────────

/// Launch the TUI in attach mode, connecting to a daemon session.
pub async fn run_attach(
    session_id: Option<String>,
    should_create_new: bool,
    model: Option<String>,
    settings: &Settings,
) -> Result<()> {
    // Resolve the session socket path
    let (resolved_session_id, socket_path) = resolve_session(session_id, should_create_new, model).await?;

    info!("attaching to session {resolved_session_id} at {socket_path}");

    // Connect to the session socket
    let stream = connect_session_socket(&socket_path).await.map_err(|e| crate::error::Error::Provider {
        message: format!("Cannot connect to session socket {socket_path}: {e}"),
    })?;

    let mut client = ClientAdapter::connect(stream, "clankers-tui", None, Some(resolved_session_id.clone()))
        .await
        .map_err(|e| crate::error::Error::Provider {
            message: format!("Handshake failed: {e}"),
        })?;

    // Read the initial SessionInfo
    let (model_name, session_hash) = match client.recv().await {
        Some(DaemonEvent::SessionInfo {
            model,
            system_prompt_hash,
            ..
        }) => (model, system_prompt_hash),
        Some(other) => {
            warn!("expected SessionInfo, got: {other:?}");
            (String::new(), String::new())
        }
        None => {
            return Err(crate::error::Error::Provider {
                message: "Session disconnected before sending SessionInfo".to_string(),
            });
        }
    };

    // Request history replay so we see the existing conversation
    client.replay_history();

    // Set up the terminal
    let mut term = super::common::init_terminal()?;

    let display_model = if model_name.is_empty() {
        "daemon".to_string()
    } else {
        model_name
    };

    let cwd = std::env::current_dir().unwrap_or_default().to_string_lossy().into_owned();
    let paths = crate::config::ClankersPaths::get();
    let theme = load_theme(settings.theme.as_deref(), &paths.global_themes_dir);
    let keymap = settings.keymap.clone().into_keymap();

    let mut app = App::new(display_model.clone(), cwd, theme);
    app.auto_theme = crate::config::theme::is_auto_theme(settings.theme.as_deref());
    app.session_id = resolved_session_id.clone();
    app.highlighter = Box::new(crate::util::syntax::SyntectHighlighter);

    // Minimal slash registry for client-side commands only
    let slash_registry = build_client_slash_registry();
    app.set_completion_source(Box::new(clanker_tui_types::CompletionSnapshot::from_source(&slash_registry)));

    // Build leader menu from builtins
    crate::modes::interactive::rebuild_leader_menu(&mut app, None, settings);

    app.connection_mode = clanker_tui_types::ConnectionMode::Attached;

    app.push_system(
        format!(
            "attached to session {} (model: {}, prompt hash: {})",
            resolved_session_id,
            display_model,
            if session_hash.is_empty() { "n/a" } else { &session_hash }
        ),
        false,
    );
    app.push_system("Type /detach or Ctrl+Q to disconnect.".to_string(), false);

    let max_subagent_panes = settings.max_subagent_panes;

    // Run the event loop with reconnection support
    let result = run_attach_with_reconnect(
        &mut term,
        &mut app,
        client,
        keymap,
        &slash_registry,
        max_subagent_panes,
        &socket_path,
        &resolved_session_id,
        clanker_tui_types::ConnectionMode::Attached,
        RecoveryMode::ExplicitAttach,
    )
    .await;

    super::scrollback_dump::finalize_terminal_and_scrollback(result, &mut term, &app.conversation.blocks, settings)
}

// ── Auto-daemon mode (extracted to auto_daemon.rs) ──────────────────────────
pub use super::auto_daemon::*;

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

// ── Session resolution ──────────────────────────────────────────────────────

/// Resolve a session ID + socket path via the control socket.
///
/// If `session_id` is None and `should_create_new` is true, creates a new session.
/// If `session_id` is None and `should_create_new` is false, lists sessions and picks
/// the first one (or errors if none exist).
async fn resolve_session(
    session_id: Option<String>,
    should_create_new: bool,
    model: Option<String>,
) -> Result<(String, String)> {
    if should_create_new {
        let resp = send_control(ControlCommand::CreateSession {
            model,
            system_prompt: None,
            token: None,
            resume_id: None,
            continue_last: false,
            cwd: None,
        })
        .await?;
        return match resp {
            ControlResponse::Created {
                session_id,
                socket_path,
            } => Ok((session_id, socket_path)),
            ControlResponse::Error { message } => Err(crate::error::Error::Provider {
                message: format!("Failed to create session: {message}"),
            }),
            other => Err(crate::error::Error::Provider {
                message: format!("Unexpected response: {other:?}"),
            }),
        };
    }

    if let Some(sid) = session_id {
        let resp = send_control(ControlCommand::AttachSession {
            session_id: sid.clone(),
        })
        .await?;
        return match resp {
            ControlResponse::Attached { socket_path } => Ok((sid, socket_path)),
            ControlResponse::Error { message } => Err(crate::error::Error::Provider {
                message: format!("Failed to attach to session: {message}"),
            }),
            other => Err(crate::error::Error::Provider {
                message: format!("Unexpected response: {other:?}"),
            }),
        };
    }

    // No session ID given — list and pick, or error.
    let resp = send_control(ControlCommand::ListSessions).await?;
    match resp {
        ControlResponse::Sessions(sessions) if sessions.is_empty() => Err(crate::error::Error::Provider {
            message: "No active sessions. Use --new to create one, or start a daemon first.".to_string(),
        }),
        ControlResponse::Sessions(sessions) if sessions.len() == 1 => {
            let s = &sessions[0];
            eprintln!("Attaching to session {} (model: {})", s.session_id, s.model);
            Ok((s.session_id.clone(), s.socket_path.clone()))
        }
        ControlResponse::Sessions(sessions) => {
            let s = pick_session(&sessions)?;
            Ok((s.session_id.clone(), s.socket_path.clone()))
        }
        ControlResponse::Error { message } => Err(crate::error::Error::Provider {
            message: format!("Failed to list sessions: {message}"),
        }),
        other => Err(crate::error::Error::Provider {
            message: format!("Unexpected response: {other:?}"),
        }),
    }
}

/// Send a control command to the daemon and return the response.
pub(crate) async fn send_control(cmd: ControlCommand) -> Result<ControlResponse> {
    let path = clankers_controller::transport::control_socket_path();
    let stream = UnixStream::connect(&path).await.map_err(|e| crate::error::Error::Provider {
        message: format!(
            "Cannot connect to daemon at {}: {e}\nIs the daemon running? Start with: clankers daemon",
            path.display()
        ),
    })?;

    let (mut reader, mut writer) = stream.into_split();

    frame::write_frame(&mut writer, &cmd).await.map_err(|e| crate::error::Error::Provider {
        message: format!("Failed to send command: {e}"),
    })?;

    let resp: ControlResponse = frame::read_frame(&mut reader).await.map_err(|e| crate::error::Error::Provider {
        message: format!("Failed to read response: {e}"),
    })?;

    Ok(resp)
}

// ── Session picker ──────────────────────────────────────────────────────────

/// Interactive terminal picker for choosing a daemon session.
///
/// Enters raw mode, draws a navigable list, returns the selected session.
/// Runs BEFORE the full TUI is initialised.
fn pick_session(sessions: &[clankers_protocol::SessionSummary]) -> Result<&clankers_protocol::SessionSummary> {
    use crossterm::cursor;
    use crossterm::event::Event;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEventKind;
    use crossterm::event::{self as ct_event};
    use crossterm::execute;
    use crossterm::style::Stylize;
    use crossterm::style::{self};
    use crossterm::terminal::{self};

    debug_assert!(!sessions.is_empty());

    // Enter raw mode with a drop guard for crash safety.
    terminal::enable_raw_mode().map_err(|e| crate::error::Error::Tui {
        message: format!("Session picker: failed to enable raw mode: {e}"),
    })?;
    let mut stdout = std::io::stdout();
    execute!(stdout, cursor::Hide).ok();

    struct RawGuard;
    impl Drop for RawGuard {
        #[cfg_attr(
            dylint_lib = "tigerstyle",
            allow(unbounded_loop, reason = "event loop; exits on quit signal")
        )]
        fn drop(&mut self) {
            crossterm::terminal::disable_raw_mode().ok();
            crossterm::execute!(std::io::stdout(), crossterm::cursor::Show).ok();
        }
    }
    let _guard = RawGuard;

    let mut selected: usize = 0;

    loop {
        // Clear and redraw.
        execute!(stdout, terminal::Clear(terminal::ClearType::All), cursor::MoveTo(0, 0),).ok();

        // Header
        execute!(stdout, style::PrintStyledContent("Select a session to attach:\r\n\r\n".bold()),).ok();

        // Column header
        let header =
            format!("  {:<10} {:<28} {:>5}  {:<20}  {}\r\n", "SESSION", "MODEL", "TURNS", "LAST ACTIVE", "CLIENTS");
        execute!(stdout, style::PrintStyledContent(header.dim())).ok();

        // Session rows
        for (i, s) in sessions.iter().enumerate() {
            let sid = if s.session_id.len() > 8 {
                &s.session_id[..8]
            } else {
                &s.session_id
            };
            let model = if s.model.len() > 26 {
                format!("{}…", &s.model[..25])
            } else {
                s.model.clone()
            };
            let line = format!(
                "  {:<10} {:<28} {:>5}  {:<20}  {}\r\n",
                sid, model, s.turn_count, s.last_active, s.client_count
            );

            if i == selected {
                execute!(stdout, style::PrintStyledContent(format!("▸ {line}").reverse())).ok();
            } else {
                execute!(stdout, style::PrintStyledContent(format!("  {line}").stylize())).ok();
            }
        }

        // Footer
        execute!(stdout, style::PrintStyledContent("\r\n  ↑/↓ navigate  Enter select  q/Esc cancel\r\n".dim()),).ok();

        // Wait for key
        if ct_event::poll(std::time::Duration::from_millis(200)).unwrap_or(false)
            && let Ok(Event::Key(key)) = ct_event::read()
        {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    selected = selected.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if selected + 1 < sessions.len() {
                        selected += 1;
                    }
                }
                KeyCode::Enter => {
                    // Clear screen before returning
                    execute!(stdout, terminal::Clear(terminal::ClearType::All), cursor::MoveTo(0, 0)).ok();
                    return Ok(&sessions[selected]);
                }
                KeyCode::Esc | KeyCode::Char('q') => {
                    execute!(stdout, terminal::Clear(terminal::ClearType::All), cursor::MoveTo(0, 0)).ok();
                    return Err(crate::error::Error::Provider {
                        message: "Session selection cancelled.".to_string(),
                    });
                }
                _ => {}
            }
        }
    }
}

// ── Recovery mode ───────────────────────────────────────────────────────────

/// Controls what happens when daemon connection is lost and socket reconnection
/// fails. Auto-daemon mode restarts the daemon and resumes; explicit attach
/// gives up.
#[derive(Clone)]
pub(crate) enum RecoveryMode {
    /// Auto-daemon: restart daemon, create session with `resume_id`, reconnect.
    AutoDaemon {
        session_id: String,
        model: String,
        cwd: String,
    },
    /// Explicit `clankers attach`: just retry the socket, give up if it fails.
    ExplicitAttach,
}

// ── Attach event loop ───────────────────────────────────────────────────────

/// Reconnection backoff parameters.
const RECONNECT_INITIAL_MS: u64 = 500;
const RECONNECT_MAX_MS: u64 = 15_000;
const RECONNECT_MAX_ATTEMPTS: u32 = 20;

/// Attempt to reconnect to a daemon session with exponential backoff.
///
/// Returns a new `ClientAdapter` on success, or `None` after exhausting
/// all retry attempts or if the user cancels.
async fn try_reconnect(socket_path: &str, session_id: &str) -> Option<ClientAdapter> {
    let mut delay_ms = RECONNECT_INITIAL_MS;

    for attempt in 1..=RECONNECT_MAX_ATTEMPTS {
        info!("reconnect attempt {attempt}/{RECONNECT_MAX_ATTEMPTS} (delay {delay_ms}ms)");

        tokio::time::sleep(Duration::from_millis(delay_ms)).await;

        let stream = match UnixStream::connect(socket_path).await {
            Ok(s) => s,
            Err(e) => {
                debug!("reconnect failed: {e}");
                delay_ms = (delay_ms * 2).min(RECONNECT_MAX_MS);
                continue;
            }
        };

        match ClientAdapter::connect(stream, "clankers-tui", None, Some(session_id.to_string())).await {
            Ok(adapter) => return Some(adapter),
            Err(e) => {
                debug!("reconnect handshake failed: {e}");
                delay_ms = (delay_ms * 2).min(RECONNECT_MAX_MS);
            }
        }
    }

    None
}

/// Restart the daemon and resume (or recreate) the session.
///
/// Called from auto-daemon mode when socket reconnection fails (daemon is dead).
/// Returns `(ClientAdapter, new_socket_path, was_resumed)` where `was_resumed` is true
/// if the session was recovered from automerge checkpoint, false if a fresh
/// session was created.
async fn try_recover_daemon(
    session_id: &str,
    model: &str,
    cwd: &str,
) -> std::result::Result<(ClientAdapter, String, bool), String> {
    // 1. Restart the daemon
    crate::commands::daemon::ensure_daemon_running()
        .await
        .map_err(|e| format!("failed to restart daemon: {e}"))?;

    // 2. Try to resume the session from automerge checkpoint
    let (new_session_id, socket_path, was_resumed) = {
        let create_cmd = ControlCommand::CreateSession {
            model: Some(model.to_string()),
            system_prompt: None,
            token: None,
            resume_id: Some(session_id.to_string()),
            continue_last: false,
            cwd: Some(cwd.to_string()),
        };

        match send_control(create_cmd).await {
            Ok(ControlResponse::Created {
                session_id: sid,
                socket_path: sp,
            }) => (sid, sp, true),
            Ok(ControlResponse::Error { message }) => {
                // Resume failed — try a fresh session
                info!("auto-daemon: resume failed ({message}), creating fresh session");
                let fresh_cmd = ControlCommand::CreateSession {
                    model: Some(model.to_string()),
                    system_prompt: None,
                    token: None,
                    resume_id: None,
                    continue_last: false,
                    cwd: Some(cwd.to_string()),
                };
                match send_control(fresh_cmd).await {
                    Ok(ControlResponse::Created {
                        session_id: sid,
                        socket_path: sp,
                    }) => (sid, sp, false),
                    Ok(other) => {
                        return Err(format!("unexpected response creating fresh session: {other:?}"));
                    }
                    Err(e) => {
                        return Err(format!("failed to create fresh session: {e}"));
                    }
                }
            }
            Ok(other) => {
                return Err(format!("unexpected response resuming session: {other:?}"));
            }
            Err(e) => {
                return Err(format!("failed to send resume command: {e}"));
            }
        }
    };

    info!("auto-daemon: recovered session {new_session_id} at {socket_path} (was_resumed={was_resumed})");

    // 3. Connect to the new session socket
    let stream = connect_session_socket(&socket_path)
        .await
        .map_err(|e| format!("cannot connect to recovered session socket {socket_path}: {e}"))?;

    let mut adapter = ClientAdapter::connect(stream, "clankers-tui", None, Some(new_session_id))
        .await
        .map_err(|e| format!("handshake failed on recovered session: {e}"))?;

    // Consume SessionInfo
    match adapter.recv().await {
        Some(DaemonEvent::SessionInfo { .. }) => {}
        Some(other) => {
            warn!("expected SessionInfo on recovery, got: {other:?}");
        }
        None => {
            return Err("recovered session disconnected before SessionInfo".to_string());
        }
    }

    Ok((adapter, socket_path, was_resumed))
}

/// Drain available DaemonEvents from the client and apply them to App state.
pub(crate) fn drain_daemon_events(
    app: &mut App,
    client: &mut ClientAdapter,
    is_replaying_history: &mut bool,
    max_subagent_panes: usize,
    parity_tracker: &mut AttachParityTracker,
) {
    while let Some(event) = client.try_recv() {
        process_daemon_event(app, client, &event, is_replaying_history, max_subagent_panes, parity_tracker);
    }
}

/// Process a single DaemonEvent — update App state, handle non-TUI events.
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(function_length, reason = "sequential event handling logic")
)]
fn process_daemon_event(
    app: &mut App,
    client: &ClientAdapter,
    event: &DaemonEvent,
    is_replaying_history: &mut bool,
    max_subagent_panes: usize,
    parity_tracker: &mut AttachParityTracker,
) {
    if parity_tracker.should_suppress(event) {
        return;
    }

    // First, try the TuiEvent conversion for all streaming/tool/session events.
    if let Some(tui_event) = daemon_event_to_tui_event(event) {
        app.handle_tui_event(&tui_event);
        return;
    }

    // Handle events that don't map to TuiEvent.
    match event {
        // ── Session metadata ────────────────────────
        DaemonEvent::SessionInfo { model, .. } => {
            if !model.is_empty() {
                app.model.clone_from(model);
            }
        }
        DaemonEvent::ModelChanged { to, .. } => {
            app.model.clone_from(to);
            app.push_system(format!("Model changed to {to}"), false);
        }

        // ── System messages ─────────────────────────
        DaemonEvent::SystemMessage { text, is_error } => {
            app.push_system(text.clone(), *is_error);
        }

        // ── Prompt lifecycle ────────────────────────
        DaemonEvent::PromptDone { error } => {
            if let Some(err) = error {
                if let Some(ref mut block) = app.conversation.active_block {
                    block.error = Some(err.clone());
                }
                app.finalize_active_block();
                app.push_system(format!("Error: {err}"), true);
            } else {
                app.finalize_active_block();
            }
            // If the user typed something while the agent was busy, send it now
            if let Some(text) = app.queued_prompt.take() {
                client.prompt(text);
            }
        }

        // ── Confirmation requests ───────────────────
        DaemonEvent::ConfirmRequest {
            request_id,
            command,
            working_dir,
        } => {
            app.overlays.confirm_dialog = Some(clankers_tui::app::BashConfirmState {
                request_id: request_id.clone(),
                command: command.clone(),
                working_dir: working_dir.clone(),
                approved: true, // default to Yes
            });
        }
        DaemonEvent::TodoRequest { request_id, action } => {
            // Todo actions are TUI-local state updates (add/update/remove items).
            // The daemon sends these for panel synchronization. Auto-respond since
            // attach mode doesn't own the todo panel state.
            debug!("todo request in attach mode: {action:?}");
            // Auto-respond with empty object — daemon handles the actual todo
            client.send(SessionCommand::TodoResponse {
                request_id: request_id.clone(),
                response: serde_json::json!({}),
            });
        }

        // ── Capability events ───────────────────────
        DaemonEvent::Capabilities { capabilities } => {
            if let Some(caps) = capabilities {
                app.push_system(format!("Capabilities: {}", caps.join(", ")), false);
            }
        }
        DaemonEvent::ToolBlocked { tool_name, reason, .. } => {
            app.push_system(format!("⛔ Tool blocked: {tool_name} — {reason}"), true);
        }

        // ── Subagent events ─────────────────────────
        DaemonEvent::SubagentStarted { id, name, task, pid } => {
            if let Some(panel) = app.panels.downcast_mut::<crate::tui::components::subagent_panel::SubagentPanel>(
                crate::tui::panel::PanelId::Subagents,
            ) {
                panel.add(id.clone(), name.clone(), task.clone(), *pid);
            }
            // Create a dedicated BSP pane for this subagent (same as embedded mode)
            if max_subagent_panes > 0 && app.layout.subagent_panes.len() < max_subagent_panes {
                let pane_id = app.layout.subagent_panes.create(
                    id.clone(),
                    name.clone(),
                    task.clone(),
                    *pid,
                    &mut app.layout.tiling,
                );
                app.layout.pane_registry.register(pane_id, crate::tui::panes::PaneKind::Subagent(id.clone()));
                crate::tui::panes::auto_split_for_subagent(&mut app.layout.tiling, &app.layout.pane_registry, pane_id);
            }
        }
        DaemonEvent::SubagentOutput { id, line } => {
            if let Some(panel) = app.panels.downcast_mut::<crate::tui::components::subagent_panel::SubagentPanel>(
                crate::tui::panel::PanelId::Subagents,
            ) {
                panel.append_output(id, line);
            }
            app.layout.subagent_panes.append_output(id, line);
        }
        DaemonEvent::SubagentDone { id } => {
            if let Some(panel) = app.panels.downcast_mut::<crate::tui::components::subagent_panel::SubagentPanel>(
                crate::tui::panel::PanelId::Subagents,
            ) {
                panel.mark_done(id);
            }
            app.layout.subagent_panes.mark_done(id);
        }
        DaemonEvent::SubagentError { id, message } => {
            if let Some(panel) = app.panels.downcast_mut::<crate::tui::components::subagent_panel::SubagentPanel>(
                crate::tui::panel::PanelId::Subagents,
            ) {
                panel.mark_error(id);
                panel.append_output(id, &format!("Error: {message}"));
            }
            app.layout.subagent_panes.mark_error(id);
        }

        // ── History replay ──────────────────────────
        DaemonEvent::HistoryBlock { block } => {
            if *is_replaying_history {
                match serde_json::from_value::<clanker_message::AgentMessage>(block.clone()) {
                    Ok(msg) => {
                        let events = clankers_controller::convert::agent_message_to_tui_events(&msg);
                        for tui_event in &events {
                            app.handle_tui_event(tui_event);
                        }
                    }
                    Err(_) => {
                        // Graceful fallback for old-format or unrecognized blocks
                        let preview = block.as_str().unwrap_or("(unrecognized block)");
                        let truncated = if preview.len() > 120 {
                            format!("{}...", &preview[..120])
                        } else {
                            preview.to_string()
                        };
                        app.push_system(format!("📜 {truncated}"), false);
                    }
                }
            }
        }
        DaemonEvent::HistoryEnd => {
            app.finalize_active_block();
            *is_replaying_history = false;
        }

        // ── Tool metadata ────────────────────────────
        DaemonEvent::ToolList { tools } => {
            app.tool_info = tools.iter().map(|t| (t.name.clone(), t.description.clone(), String::new())).collect();
        }
        DaemonEvent::DisabledToolsChanged { tools } => {
            app.disabled_tools = tools.iter().cloned().collect();
        }

        // ── State sync events ───────────────────────
        DaemonEvent::ThinkingLevelChanged { from, to } => {
            app.push_system(format!("Thinking: {from} → {to}"), false);
        }
        DaemonEvent::LoopStatus {
            active,
            iteration,
            max_iterations,
            break_condition,
        } => {
            if *active {
                let iter_str = match (iteration, max_iterations) {
                    (Some(i), Some(m)) => format!(" ({i}/{m})"),
                    (Some(i), None) => format!(" ({i})"),
                    _ => String::new(),
                };
                let cond_str = break_condition.as_deref().unwrap_or("");
                app.push_system(format!("Loop active{iter_str} {cond_str}"), false);
            } else {
                app.push_system("Loop finished".to_string(), false);
            }
        }
        DaemonEvent::AutoTestChanged { enabled, command } => {
            if *enabled {
                let cmd = command.as_deref().unwrap_or("(default)");
                app.push_system(format!("Auto-test enabled: {cmd}"), false);
            } else {
                app.push_system("Auto-test disabled".to_string(), false);
            }
            app.auto_test_enabled = *enabled;
            app.auto_test_command.clone_from(command);
        }
        DaemonEvent::CostUpdate { total_cost_usd, .. } => {
            app.push_system(format!("Session cost: ${total_cost_usd:.4}"), false);
        }

        // ── Ignored events ──────────────────────────
        // ── Plugin events ───────────────────────
        DaemonEvent::PluginWidget { plugin, widget } => {
            if let Some(widget_json) = widget {
                if let Ok(w) = serde_json::from_value::<clanker_tui_types::Widget>(widget_json.clone()) {
                    app.plugin_ui.widgets.insert(plugin.clone(), w);
                }
            } else {
                app.plugin_ui.widgets.remove(plugin);
            }
        }
        DaemonEvent::PluginStatus { plugin, text, color } => {
            if let Some(text) = text {
                app.plugin_ui.status_segments.insert(plugin.clone(), clanker_tui_types::StatusSegment {
                    text: text.clone(),
                    color: color.clone(),
                });
            } else {
                app.plugin_ui.status_segments.remove(plugin);
            }
        }
        DaemonEvent::PluginNotify { plugin, message, level } => {
            app.plugin_ui.notifications.push(clanker_tui_types::PluginNotification {
                plugin: plugin.clone(),
                message: message.clone(),
                level: level.clone(),
                created: std::time::Instant::now(),
            });
        }
        DaemonEvent::PluginList { plugins } => {
            app.daemon_plugins = Some(plugins.clone());
            // Display plugin list when it arrives (in response to /plugin)
            if plugins.is_empty() {
                app.push_system("No plugins loaded.".to_string(), false);
            } else {
                let mut lines = vec![format!("Loaded plugins ({}):", plugins.len())];
                for p in plugins {
                    let marker = match p.state.as_str() {
                        "Active" => "\u{2713}",
                        "Loaded" | "Starting" => "\u{25cb}",
                        "Backoff" => "↺",
                        "Disabled" => "−",
                        _ => "\u{2717}",
                    };
                    let kind = p.kind.as_deref().unwrap_or("unknown");
                    lines.push(format!("  {} {} v{} [{} / {}]", marker, p.name, p.version, kind, p.state));
                    let tools = if p.tools.is_empty() {
                        "none".to_string()
                    } else {
                        p.tools.join(", ")
                    };
                    lines.push(format!("      tools: {}", tools));
                    if let Some(error) = &p.last_error {
                        lines.push(format!("      last error: {}", error));
                    }
                }
                app.push_system(lines.join("\n"), false);
            }
        }

        DaemonEvent::SystemPromptResponse { .. } => {
            // We didn't request this — ignore
        }

        // Events already handled by daemon_event_to_tui_event above
        _ => {}
    }
}

// ── Terminal event handling ──────────────────────────────────────────────────

pub(crate) fn handle_terminal_events(
    app: &mut App,
    client: &mut ClientAdapter,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    keymap: &Keymap,
    slash_registry: &slash_commands::SlashRegistry,
    parity_tracker: &mut AttachParityTracker,
) -> Result<()> {
    let mut poll_timeout = Duration::from_millis(50);
    while let Some(event) = tui_event::poll_event(poll_timeout) {
        poll_timeout = Duration::ZERO;
        match event {
            AppEvent::Paste(text) => {
                app.input_mode = InputMode::Insert;
                app.selection = None;
                app.editor.insert_str(&text);
                app.update_slash_menu();
            }
            AppEvent::Key(key) => {
                handle_key_event(app, client, terminal, key, keymap, slash_registry, parity_tracker);
            }
            AppEvent::MouseDown(button, col, row) => {
                crate::tui::mouse::handle_mouse_down(app, button, col, row);
            }
            AppEvent::MouseDrag(button, col, row) => {
                crate::tui::mouse::handle_mouse_drag(app, button, col, row);
            }
            AppEvent::MouseUp(button, col, row) => {
                crate::tui::mouse::handle_mouse_up(app, button, col, row);
            }
            AppEvent::ScrollUp(col, row, n) => {
                crate::tui::mouse::handle_mouse_scroll(app, col, row, true, n);
            }
            AppEvent::ScrollDown(col, row, n) => {
                crate::tui::mouse::handle_mouse_scroll(app, col, row, false, n);
            }
            AppEvent::Resize(_, _) => {}
            AppEvent::FocusGained => {
                if app.auto_theme {
                    app.theme = crate::config::theme::detect_theme();
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// Key handler for attach mode.
///
/// Supports the same overlays, mode switching, and navigation as the embedded
/// TUI. The key difference is input submission: instead of dispatching to an
/// in-process agent, we send SessionCommand to the daemon.
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(function_length, reason = "sequential event handling logic")
)]
fn handle_key_event(
    app: &mut App,
    client: &mut ClientAdapter,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    key: crossterm::event::KeyEvent,
    keymap: &Keymap,
    slash_registry: &slash_commands::SlashRegistry,
    parity_tracker: &mut AttachParityTracker,
) {
    use crossterm::event::KeyCode;
    use crossterm::event::KeyModifiers;

    use crate::config::keybindings::Action;
    use crate::config::keybindings::CoreAction;
    use crate::config::keybindings::ExtendedAction;
    use crate::tui::selectors;

    app.selection = None;

    // Force quit (Ctrl+Q)
    if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.should_quit = true;
        return;
    }

    // Bash confirm dialog
    if let Some(ref mut confirm) = app.overlays.confirm_dialog {
        match key.code {
            KeyCode::Left | KeyCode::Right | KeyCode::Char('h' | 'l') | KeyCode::Tab => {
                confirm.approved = !confirm.approved;
            }
            KeyCode::Char('y' | 'Y') => {
                let request_id = confirm.request_id.clone();
                app.overlays.confirm_dialog = None;
                client.send(SessionCommand::ConfirmBash {
                    request_id,
                    approved: true,
                });
                app.push_system("✅ Command approved.".to_string(), false);
            }
            KeyCode::Char('n' | 'N') | KeyCode::Esc => {
                let request_id = confirm.request_id.clone();
                app.overlays.confirm_dialog = None;
                client.send(SessionCommand::ConfirmBash {
                    request_id,
                    approved: false,
                });
                app.push_system("❌ Command denied.".to_string(), true);
            }
            KeyCode::Enter => {
                let request_id = confirm.request_id.clone();
                let is_approved = confirm.approved;
                app.overlays.confirm_dialog = None;
                client.send(SessionCommand::ConfirmBash {
                    request_id,
                    approved: is_approved,
                });
                if is_approved {
                    app.push_system("✅ Command approved.".to_string(), false);
                } else {
                    app.push_system("❌ Command denied.".to_string(), true);
                }
            }
            _ => {}
        }
        return;
    }

    // Overlay intercepts — same as embedded mode
    if app.overlays.cost_overlay_visible && matches!(key.code, KeyCode::Esc | KeyCode::Char('C' | 'c' | 'q')) {
        app.overlays.cost_overlay_visible = false;
        return;
    }

    if app.overlays.model_selector.visible {
        let (consumed, action) = selectors::handle_model_selector_key(app, &key);
        if let Some(clanker_tui_types::SelectorAction::SetModel(model)) = action {
            client.send(SessionCommand::SetModel { model: model.clone() });
            app.model = model;
        }
        if consumed {
            return;
        }
    }

    // Account selector overlay
    if app.overlays.account_selector.visible {
        let (consumed, action) = crate::tui::selectors::handle_account_selector_key(app, &key);
        if let Some(clanker_tui_types::SelectorAction::SwitchAccount(name)) = action {
            client.send(SessionCommand::SwitchAccount { account: name });
        }
        if consumed {
            return;
        }
    }

    // Tool toggle overlay
    if app.overlays.tool_toggle.visible {
        let (consumed, dirty) = crate::tui::selectors::handle_tool_toggle_key(app, &key);
        if dirty {
            let disabled = apply_standalone_disabled_tools(app, app.overlays.tool_toggle.disabled_set());
            parity_tracker.expect_disabled_tools_message();
            client.send(SessionCommand::SetDisabledTools { tools: disabled });
        }
        if consumed {
            return;
        }
    }

    // Leader menu
    if app.overlays.leader_menu.visible() {
        if let Some(leader_action) = app.overlays.leader_menu.handle_key(&key) {
            handle_leader_action_attach(app, client, leader_action, slash_registry, parity_tracker);
        }
        return;
    }

    // Output search
    if app.overlays.output_search.active {
        crate::modes::event_handlers::handle_output_search_key(app, &key);
        return;
    }

    // Slash menu (insert mode only)
    if app.input_mode == InputMode::Insert
        && app.slash_menu.visible
        && handle_slash_menu_key_attach(app, client, &key, keymap, slash_registry, parity_tracker)
    {
        return;
    }

    // Panel focus keys
    if app.has_panel_focus() && app.input_mode == InputMode::Normal && handle_panel_focused_key_attach(app, key) {
        return;
    }

    // Resolve through keymap
    let action = keymap.resolve(app.input_mode, &key);
    if let Some(action) = action {
        if matches!(&action, Action::Extended(ExtendedAction::OpenEditor)) {
            crate::tui::clipboard::open_external_editor(terminal, app);
            return;
        }

        match &action {
            // Submit: send input to daemon
            Action::Core(CoreAction::Submit) => {
                app.accept_slash_completion();
                if let Some(text) = app.submit_input() {
                    submit_input_attach(app, client, &text, slash_registry, parity_tracker);
                }
            }
            // Cancel: tell daemon to abort
            Action::Core(CoreAction::Cancel) => {
                client.abort();
                app.push_system("Abort sent to daemon.".to_string(), false);
            }
            // Client-side TUI actions handled locally
            _ => {
                handle_local_action(app, client, &action, &key, parity_tracker);
            }
        }
    } else if app.input_mode == InputMode::Insert {
        crate::modes::event_handlers::handle_insert_char(app, &key);
    }
}

/// Attach-side slash routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AttachSlashRoute {
    CustomLocal,
    RegistryLocal,
    GetPlugins,
    ForwardToDaemon,
}

const ATTACH_CUSTOM_LOCAL_COMMANDS: &[&str] = &["quit", "q", "detach", "zoom", "help"];
const ATTACH_REGISTRY_LOCAL_COMMANDS: &[&str] = &[
    "status", "usage", "metrics", "insights", "version", "router", "cd", "shell", "export", "layout", "preview",
    "editor", "todo", "tools", "think", "compact", "compress",
];
const ATTACH_REGISTRY_LOCAL_EMPTY_ARG_COMMANDS: &[&str] = &["model", "role"];
const ATTACH_LOCAL_SESSION_SUBCOMMANDS: &[&str] = &["list", "ls", "delete", "rm", "purge"];

#[derive(Debug, Default)]
pub(crate) struct AttachParityTracker {
    thinking_ack_messages_to_suppress: usize,
    disabled_tools_messages_to_suppress: usize,
    manual_compactions_to_suppress: usize,
}

impl AttachParityTracker {
    pub(crate) fn expect_thinking_ack_message(&mut self) {
        self.thinking_ack_messages_to_suppress += 1;
    }

    pub(crate) fn expect_disabled_tools_message(&mut self) {
        self.disabled_tools_messages_to_suppress += 1;
    }

    pub(crate) fn expect_manual_compaction(&mut self) {
        self.manual_compactions_to_suppress += 1;
    }

    fn should_suppress(&mut self, event: &DaemonEvent) -> bool {
        if self.should_suppress_thinking_ack_message(event) {
            return true;
        }

        if self.should_suppress_disabled_tools_message(event) {
            return true;
        }

        self.should_suppress_manual_compaction(event)
    }

    fn should_suppress_thinking_ack_message(&mut self, event: &DaemonEvent) -> bool {
        if self.thinking_ack_messages_to_suppress == 0 {
            return false;
        }

        let should_suppress = is_thinking_ack_message(event);
        if should_suppress {
            self.thinking_ack_messages_to_suppress -= 1;
        }
        should_suppress
    }

    fn should_suppress_disabled_tools_message(&mut self, event: &DaemonEvent) -> bool {
        if self.disabled_tools_messages_to_suppress == 0 {
            return false;
        }

        let should_suppress = matches!(
            event,
            DaemonEvent::SystemMessage { text, is_error: false } if text.starts_with("Disabled tools updated:")
        );
        if should_suppress {
            self.disabled_tools_messages_to_suppress -= 1;
        }
        should_suppress
    }

    fn should_suppress_manual_compaction(&mut self, event: &DaemonEvent) -> bool {
        if self.manual_compactions_to_suppress == 0 {
            return false;
        }

        let should_suppress = matches!(event, DaemonEvent::SessionCompaction { .. });
        if should_suppress {
            self.manual_compactions_to_suppress -= 1;
        }
        should_suppress
    }
}

/// Decide how attach mode should handle a slash command.
fn is_thinking_ack_message(event: &DaemonEvent) -> bool {
    matches!(event, DaemonEvent::SystemMessage { text, is_error: false } if text.starts_with("Thinking"))
}

fn route_attach_slash(command: &str, args: &str) -> AttachSlashRoute {
    if ATTACH_CUSTOM_LOCAL_COMMANDS.contains(&command) {
        return AttachSlashRoute::CustomLocal;
    }

    if command == "plugin" {
        return AttachSlashRoute::GetPlugins;
    }

    if ATTACH_REGISTRY_LOCAL_COMMANDS.contains(&command) {
        return AttachSlashRoute::RegistryLocal;
    }

    if ATTACH_REGISTRY_LOCAL_EMPTY_ARG_COMMANDS.contains(&command) && args.trim().is_empty() {
        return AttachSlashRoute::RegistryLocal;
    }

    if command == "session" && is_attach_local_session_command(args) {
        return AttachSlashRoute::RegistryLocal;
    }

    AttachSlashRoute::ForwardToDaemon
}

fn is_attach_local_session_command(args: &str) -> bool {
    let trimmed = args.trim();
    if trimmed.is_empty() {
        return true;
    }

    let subcommand = trimmed.split_whitespace().next().unwrap_or_default();
    ATTACH_LOCAL_SESSION_SUBCOMMANDS.contains(&subcommand)
}

/// Submit input in attach mode — some slash commands run locally,
/// the rest are forwarded to the daemon.
fn submit_input_attach(
    app: &mut App,
    client: &ClientAdapter,
    text: &str,
    slash_registry: &slash_commands::SlashRegistry,
    parity_tracker: &mut AttachParityTracker,
) {
    if let Some((command, args)) = slash_commands::parse_command(text) {
        dispatch_attach_slash(app, client, &command, &args, slash_registry, parity_tracker);
    } else {
        // Regular prompt — expand @file/context references, then send text plus any image blocks.
        let expanded = crate::util::at_file::expand_at_refs_with_images(text, &app.cwd);
        let images = expanded
            .images
            .into_iter()
            .filter_map(|content| match content {
                crate::provider::message::Content::Image {
                    source: crate::provider::message::ImageSource::Base64 { media_type, data },
                } => Some(clankers_protocol::ImageData { data, media_type }),
                _ => None,
            })
            .collect();
        client.prompt_with_images(expanded.text, images);
    }
}

fn dispatch_attach_slash(
    app: &mut App,
    client: &ClientAdapter,
    command: &str,
    args: &str,
    slash_registry: &slash_commands::SlashRegistry,
    parity_tracker: &mut AttachParityTracker,
) {
    match route_attach_slash(command, args) {
        AttachSlashRoute::CustomLocal => handle_client_side_slash(app, command, args),
        AttachSlashRoute::RegistryLocal => {
            handle_attach_registry_slash(app, client, command, args, slash_registry, parity_tracker)
        }
        AttachSlashRoute::GetPlugins => {
            client.send(SessionCommand::GetPlugins);
        }
        AttachSlashRoute::ForwardToDaemon => {
            client.send(SessionCommand::SlashCommand {
                command: command.to_string(),
                args: args.to_string(),
            });
        }
    }
}

/// Handle a client-side slash command locally.
fn handle_client_side_slash(app: &mut App, command: &str, args: &str) {
    match command {
        "quit" | "q" => {
            app.should_quit = true;
        }
        "detach" => {
            app.should_quit = true;
            app.push_system("Detaching from session.".to_string(), false);
        }
        "zoom" => {
            app.zoom_toggle();
        }
        "help" => {
            app.push_system("Attach mode — locally handled slash commands include:".to_string(), false);
            app.push_system(
                "  /status /usage /version /router /model (no args) /role (no args) /session [list|delete|purge] /cd /shell /export"
                    .to_string(),
                false,
            );
            app.push_system(
                "  /layout /preview /editor /todo /tools /think [level] /compact /compress /plugin".to_string(),
                false,
            );
            app.push_system(
                "  /think with no args cycles level; /plugin fetches daemon plugin inventory; /quit /detach /zoom stay client-side."
                    .to_string(),
                false,
            );
            app.push_system("  Unlisted commands generally forward to daemon.".to_string(), false);
        }
        _ => {
            app.push_system(format!("Client command /{command} not implemented in attach mode."), true);
        }
    }

    let _ = args;
}

fn handle_attach_registry_slash(
    app: &mut App,
    client: &ClientAdapter,
    command: &str,
    args: &str,
    slash_registry: &slash_commands::SlashRegistry,
    parity_tracker: &mut AttachParityTracker,
) {
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel();
    let (panel_tx, _panel_rx) = tokio::sync::mpsc::unbounded_channel();
    let db: Option<crate::db::Db> = None;
    let mut session_manager = None;
    let mut ctx = slash_commands::handlers::SlashContext {
        app,
        cmd_tx: &cmd_tx,
        plugin_manager: None,
        panel_tx: &panel_tx,
        db: &db,
        session_manager: &mut session_manager,
    };
    slash_registry.dispatch(command, args, &mut ctx);
    flush_attach_agent_commands(app, client, &mut cmd_rx, command, parity_tracker);
}

fn flush_attach_agent_commands(
    app: &mut App,
    client: &ClientAdapter,
    cmd_rx: &mut tokio::sync::mpsc::UnboundedReceiver<crate::modes::interactive::AgentCommand>,
    command: &str,
    parity_tracker: &mut AttachParityTracker,
) {
    while let Ok(agent_cmd) = cmd_rx.try_recv() {
        match agent_cmd {
            crate::modes::interactive::AgentCommand::SetThinkingLevel(level) => {
                bridge_attach_thinking_level_change(
                    app,
                    client,
                    parity_tracker,
                    SessionCommand::SetThinkingLevel {
                        level: level.label().to_string(),
                    },
                    level,
                );
            }
            crate::modes::interactive::AgentCommand::CycleThinkingLevel => {
                let next_level = app.thinking_level.next();
                bridge_attach_thinking_level_change(
                    app,
                    client,
                    parity_tracker,
                    SessionCommand::CycleThinkingLevel,
                    next_level,
                );
            }
            crate::modes::interactive::AgentCommand::SetDisabledTools(disabled) => {
                let tools = apply_standalone_disabled_tools(app, disabled);
                parity_tracker.expect_disabled_tools_message();
                client.send(SessionCommand::SetDisabledTools { tools });
            }
            crate::modes::interactive::AgentCommand::CompressContext => {
                parity_tracker.expect_manual_compaction();
                client.send(SessionCommand::CompactHistory);
            }
            other => {
                if let Some(session_command) = translate_attach_agent_command(other) {
                    client.send(session_command);
                } else {
                    warn!("attach local slash /{command} emitted unsupported agent command");
                }
            }
        }
    }
}

fn bridge_attach_thinking_level_change(
    app: &mut App,
    client: &ClientAdapter,
    parity_tracker: &mut AttachParityTracker,
    session_command: SessionCommand,
    level: crate::provider::ThinkingLevel,
) {
    apply_standalone_thinking_level(app, level);
    parity_tracker.expect_thinking_ack_message();
    client.send(session_command);
}

fn apply_standalone_disabled_tools(app: &mut App, disabled: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut tools: Vec<String> = disabled.into_iter().collect();
    tools.sort();
    app.disabled_tools = tools.iter().cloned().collect();
    tools
}

fn apply_standalone_thinking_level(app: &mut App, level: crate::provider::ThinkingLevel) {
    app.thinking_enabled = level.is_enabled();
    app.thinking_level = level;
    app.push_system(format_attach_thinking_message(level), false);
}

fn format_attach_thinking_message(level: crate::provider::ThinkingLevel) -> String {
    match level.budget_tokens() {
        Some(tokens) => format!("Thinking: {} ({} tokens)", level.label(), tokens),
        None => "Thinking: off".to_string(),
    }
}

fn translate_attach_agent_command(agent_cmd: crate::modes::interactive::AgentCommand) -> Option<SessionCommand> {
    match agent_cmd {
        crate::modes::interactive::AgentCommand::SetModel(model) => Some(SessionCommand::SetModel { model }),
        _ => None,
    }
}

/// Handle a leader menu action in attach mode.
fn handle_leader_action_attach(
    app: &mut App,
    client: &ClientAdapter,
    action: clanker_tui_types::LeaderAction,
    slash_registry: &slash_commands::SlashRegistry,
    parity_tracker: &mut AttachParityTracker,
) {
    use clanker_tui_types::LeaderAction;

    match action {
        LeaderAction::Command(cmd) => {
            if let Some((command, args)) = slash_commands::parse_command(&cmd) {
                dispatch_attach_slash(app, client, &command, &args, slash_registry, parity_tracker);
            }
        }
        LeaderAction::Action(action) => {
            // Handle keymap actions from leader menu as local actions
            let dummy_key = crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Null,
                crossterm::event::KeyModifiers::empty(),
            );
            handle_local_action(app, client, &action, &dummy_key, parity_tracker);
        }
        LeaderAction::Submenu(_) => {
            // Submenus are handled by the leader menu widget itself
        }
    }
}

/// Handle the slash menu key event in attach mode.
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(catch_all_on_enum, reason = "default handler covers many variants uniformly")
)]
fn handle_slash_menu_key_attach(
    app: &mut App,
    client: &ClientAdapter,
    key: &crossterm::event::KeyEvent,
    keymap: &Keymap,
    slash_registry: &slash_commands::SlashRegistry,
    parity_tracker: &mut AttachParityTracker,
) -> bool {
    use crossterm::event::KeyCode;

    use crate::config::keybindings::Action;
    use crate::config::keybindings::CoreAction;

    // Menu navigation keys
    match key.code {
        KeyCode::Up => {
            app.slash_menu.select_prev();
            return true;
        }
        KeyCode::Down => {
            app.slash_menu.select_next();
            return true;
        }
        _ => {}
    }

    let action = keymap.resolve(app.input_mode, key);
    match action {
        Some(Action::Core(CoreAction::MenuUp)) => {
            app.slash_menu.select_prev();
            true
        }
        Some(Action::Core(CoreAction::MenuDown)) => {
            app.slash_menu.select_next();
            true
        }
        Some(Action::Core(CoreAction::MenuClose)) => {
            app.slash_menu.hide();
            true
        }
        Some(Action::Core(CoreAction::EnterNormal)) => {
            app.slash_menu.hide();
            app.input_mode = InputMode::Normal;
            true
        }
        Some(Action::Core(CoreAction::Submit)) => {
            app.accept_slash_completion();
            if let Some(text) = app.submit_input() {
                submit_input_attach(app, client, &text, slash_registry, parity_tracker);
            }
            true
        }
        Some(Action::Core(CoreAction::DeleteBack)) => {
            app.editor.delete_back();
            app.update_slash_menu();
            true
        }
        _ => false,
    }
}

/// Handle local TUI actions (mode switching, navigation, etc.).
///
/// Handles all client-side actions. Daemon-dependent actions (thinking
/// toggle, rerun, auto-test) are forwarded via the client.
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        function_length,
        reason = "sequential setup/dispatch logic — splitting would fragment readability"
    )
)]
fn handle_local_action(
    app: &mut App,
    client: &ClientAdapter,
    action: &crate::config::keybindings::Action,
    _key: &crossterm::event::KeyEvent,
    parity_tracker: &mut AttachParityTracker,
) {
    use clanker_tui_types::AppState;
    use clanker_tui_types::BlockEntry;
    use ratatui::layout::Direction;
    use ratatui_hypertile::HypertileAction;
    use ratatui_hypertile::Towards;

    use crate::config::keybindings::Action;
    use crate::config::keybindings::CoreAction;
    use crate::config::keybindings::ExtendedAction;

    match action {
        // ── Mode switching ──────────────────────────
        Action::Core(CoreAction::EnterInsert) => {
            app.input_mode = InputMode::Insert;
        }
        Action::Core(CoreAction::EnterNormal) => {
            app.input_mode = InputMode::Normal;
            app.slash_menu.hide();
        }

        // ── Navigation / scroll ─────────────────────
        Action::Core(CoreAction::ScrollUp) => app.conversation.scroll.scroll_up(3),
        Action::Core(CoreAction::ScrollDown) => app.conversation.scroll.scroll_down(3),
        Action::Core(CoreAction::ScrollPageUp) => app.conversation.scroll.scroll_up(15),
        Action::Core(CoreAction::ScrollPageDown) => app.conversation.scroll.scroll_down(15),
        Action::Core(CoreAction::ScrollToTop) => app.conversation.scroll.scroll_to_top(),
        Action::Core(CoreAction::ScrollToBottom) => app.conversation.scroll.scroll_to_bottom(),
        Action::Core(CoreAction::FocusPrevBlock) => {
            app.apply_tiling_action(HypertileAction::FocusDirection {
                direction: Direction::Vertical,
                towards: Towards::Start,
            });
        }
        Action::Core(CoreAction::FocusNextBlock) => {
            app.apply_tiling_action(HypertileAction::FocusDirection {
                direction: Direction::Vertical,
                towards: Towards::End,
            });
        }

        // ── Editor ──────────────────────────────────
        Action::Core(CoreAction::MoveLeft) => app.editor.move_left(),
        Action::Core(CoreAction::MoveRight) => app.editor.move_right(),
        Action::Core(CoreAction::MoveHome) => app.editor.move_home(),
        Action::Core(CoreAction::MoveEnd) => app.editor.move_end(),
        Action::Core(CoreAction::DeleteBack) => {
            app.editor.delete_back();
            app.update_slash_menu();
        }
        Action::Core(CoreAction::DeleteForward) => {
            app.editor.delete_forward();
            app.update_slash_menu();
        }
        Action::Core(CoreAction::DeleteWord) => {
            app.editor.delete_word_back();
            app.update_slash_menu();
        }
        Action::Core(CoreAction::ClearLine) => {
            app.editor.clear();
            app.input_mode = InputMode::Insert;
        }
        Action::Core(CoreAction::HistoryUp) => app.editor.history_up(),
        Action::Core(CoreAction::HistoryDown) => app.editor.history_down(),
        Action::Core(CoreAction::Unfocus) => app.unfocus_panel(),

        // ── Search ──────────────────────────────────
        Action::Extended(ExtendedAction::SearchOutput) => {
            app.overlays.output_search.activate();
        }
        Action::Extended(ExtendedAction::SearchNext) => {
            if !app.overlays.output_search.matches.is_empty() {
                app.overlays.output_search.next_match();
                app.overlays.output_search.scroll_to_current = true;
            }
        }
        Action::Extended(ExtendedAction::SearchPrev) => {
            if !app.overlays.output_search.matches.is_empty() {
                app.overlays.output_search.prev_match();
                app.overlays.output_search.scroll_to_current = true;
            }
        }

        // ── Block operations ────────────────────────
        Action::Extended(ExtendedAction::ToggleBlockCollapse) => {
            if app.conversation.focused_block.is_some() {
                app.toggle_focused_block();
            }
        }
        Action::Extended(ExtendedAction::CollapseAllBlocks) => app.collapse_all_blocks(),
        Action::Extended(ExtendedAction::ExpandAllBlocks) => app.expand_all_blocks(),
        Action::Extended(ExtendedAction::CopyBlock) => app.copy_focused_block(),
        Action::Extended(ExtendedAction::RerunBlock) => {
            if let Some(prompt) = app.get_focused_block_prompt() {
                client.prompt(prompt);
            }
        }
        Action::Extended(ExtendedAction::EditBlock) => {
            if app.conversation.focused_block.is_some()
                && app.state == AppState::Idle
                && app.edit_focused_block_prompt()
            {
                app.input_mode = InputMode::Insert;
            }
        }
        Action::Extended(ExtendedAction::ToggleBlockIds) => {
            app.overlays.show_block_ids = !app.overlays.show_block_ids;
        }
        Action::Extended(ExtendedAction::ToggleShowThinking) => {
            app.show_thinking = !app.show_thinking;
            let state = if app.show_thinking { "visible" } else { "hidden" };
            app.push_system(format!("Thinking content now {state}."), false);
        }

        // ── Branch navigation ───────────────────────
        Action::Extended(ExtendedAction::BranchPrev) => {
            if app.conversation.focused_block.is_some() {
                app.branch_prev();
            } else {
                app.apply_tiling_action(HypertileAction::FocusDirection {
                    direction: Direction::Horizontal,
                    towards: Towards::Start,
                });
                app.input_mode = InputMode::Normal;
            }
        }
        Action::Extended(ExtendedAction::BranchNext) => {
            if app.conversation.focused_block.is_some() {
                app.branch_next();
            } else {
                app.apply_tiling_action(HypertileAction::FocusDirection {
                    direction: Direction::Horizontal,
                    towards: Towards::End,
                });
                app.input_mode = InputMode::Normal;
            }
        }
        Action::Extended(ExtendedAction::ToggleBranchPanel) => {
            use clanker_tui_types::PanelId;
            if app.layout.focused_panel == Some(PanelId::Branches) {
                app.unfocus_panel();
            } else {
                let active_ids: std::collections::HashSet<usize> = app
                    .conversation
                    .blocks
                    .iter()
                    .filter_map(|e| match e {
                        BlockEntry::Conversation(b) => Some(b.id),
                        _ => None,
                    })
                    .collect();
                if let Some(bp) =
                    app.panels.downcast_mut::<crate::tui::components::branch_panel::BranchPanel>(PanelId::Branches)
                {
                    bp.refresh(&app.conversation.all_blocks.clone(), &active_ids);
                }
                app.focus_panel(PanelId::Branches);
            }
        }
        Action::Extended(ExtendedAction::OpenBranchSwitcher) => {
            let active_ids: std::collections::HashSet<usize> = app
                .conversation
                .blocks
                .iter()
                .filter_map(|e| match e {
                    BlockEntry::Conversation(b) => Some(b.id),
                    _ => None,
                })
                .collect();
            app.branching.switcher.open(&app.conversation.all_blocks.clone(), &active_ids);
        }

        // ── Panel focus ─────────────────────────────
        Action::Extended(ExtendedAction::TogglePanelFocus) => {
            if app.has_panel_focus() {
                app.unfocus_panel();
            } else {
                app.apply_tiling_action(HypertileAction::FocusNext);
                app.input_mode = InputMode::Normal;
            }
        }
        Action::Extended(ExtendedAction::PanelNextTab) => {
            app.apply_tiling_action(HypertileAction::FocusDirection {
                direction: Direction::Horizontal,
                towards: Towards::End,
            });
            app.input_mode = InputMode::Normal;
        }
        Action::Extended(ExtendedAction::PanelPrevTab) => {
            app.apply_tiling_action(HypertileAction::FocusDirection {
                direction: Direction::Horizontal,
                towards: Towards::Start,
            });
            app.input_mode = InputMode::Normal;
        }

        // ── Pane tiling ─────────────────────────────
        Action::Extended(ExtendedAction::PaneSplitVertical) => {
            app.split_focused_pane(Direction::Vertical);
        }
        Action::Extended(ExtendedAction::PaneSplitHorizontal) => {
            app.split_focused_pane(Direction::Horizontal);
        }
        Action::Extended(ExtendedAction::PaneClose) => app.close_focused_pane(),
        Action::Extended(ExtendedAction::PaneEqualize) => {
            app.apply_tiling_action(HypertileAction::SetFocusedRatio { ratio: 0.5 });
        }
        Action::Extended(ExtendedAction::PaneGrow) => {
            app.apply_tiling_action(HypertileAction::ResizeFocused { delta: 0.05 });
        }
        Action::Extended(ExtendedAction::PaneShrink) => {
            app.apply_tiling_action(HypertileAction::ResizeFocused { delta: -0.05 });
        }
        Action::Extended(ExtendedAction::PaneMoveLeft) => {
            app.apply_tiling_action(HypertileAction::MoveFocused {
                direction: Direction::Horizontal,
                towards: Towards::Start,
                scope: ratatui_hypertile::MoveScope::Window,
            });
        }
        Action::Extended(ExtendedAction::PaneMoveRight) => {
            app.apply_tiling_action(HypertileAction::MoveFocused {
                direction: Direction::Horizontal,
                towards: Towards::End,
                scope: ratatui_hypertile::MoveScope::Window,
            });
        }
        Action::Extended(ExtendedAction::PaneMoveUp) => {
            app.apply_tiling_action(HypertileAction::MoveFocused {
                direction: Direction::Vertical,
                towards: Towards::Start,
                scope: ratatui_hypertile::MoveScope::Window,
            });
        }
        Action::Extended(ExtendedAction::PaneMoveDown) => {
            app.apply_tiling_action(HypertileAction::MoveFocused {
                direction: Direction::Vertical,
                towards: Towards::End,
                scope: ratatui_hypertile::MoveScope::Window,
            });
        }
        Action::Extended(ExtendedAction::PaneZoom) => app.zoom_toggle(),
        Action::Extended(ExtendedAction::PanelScrollUp) => {
            use clanker_tui_types::PanelId;
            if let Some(sp) =
                app.panels.downcast_mut::<crate::tui::components::subagent_panel::SubagentPanel>(PanelId::Subagents)
            {
                sp.scroll.scroll_up(3);
            }
        }
        Action::Extended(ExtendedAction::PanelScrollDown) => {
            use clanker_tui_types::PanelId;
            if let Some(sp) =
                app.panels.downcast_mut::<crate::tui::components::subagent_panel::SubagentPanel>(PanelId::Subagents)
            {
                sp.scroll.scroll_down(3);
            }
        }
        Action::Extended(ExtendedAction::PanelClearDone) => {
            use clanker_tui_types::PanelId;
            if let Some(sp) =
                app.panels.downcast_mut::<crate::tui::components::subagent_panel::SubagentPanel>(PanelId::Subagents)
            {
                sp.clear_done();
                if !sp.is_visible() {
                    app.unfocus_panel();
                }
            }
        }
        Action::Extended(ExtendedAction::PanelKill) => {
            // No panel_tx in attach mode — kill not supported yet
        }
        Action::Extended(ExtendedAction::PanelRemove) => {
            use clanker_tui_types::PanelId;
            if let Some(sp) =
                app.panels.downcast_mut::<crate::tui::components::subagent_panel::SubagentPanel>(PanelId::Subagents)
            {
                sp.remove_selected();
            }
        }

        // ── Overlays ────────────────────────────────
        Action::Extended(ExtendedAction::OpenLeaderMenu) => app.overlays.leader_menu.open(),
        Action::Extended(ExtendedAction::OpenModelSelector) => {
            let models = app.available_models.clone();
            if models.is_empty() {
                app.push_system("No models available.".to_string(), true);
            } else {
                app.overlays.model_selector = crate::tui::components::model_selector::ModelSelector::new(models);
                app.overlays.model_selector.open();
            }
        }
        Action::Extended(ExtendedAction::OpenAccountSelector) => {
            use crate::provider::auth::AuthStoreExt;
            let paths = crate::config::ClankersPaths::get();
            let store = crate::provider::auth::AuthStore::load(&paths.global_auth);
            let accounts: Vec<crate::tui::components::account_selector::AccountItem> = store
                .list_anthropic_accounts()
                .into_iter()
                .map(|info| crate::tui::components::account_selector::AccountItem {
                    name: info.name,
                    label: info.label,
                    is_active: info.is_active,
                    is_expired: info.is_expired,
                })
                .collect();
            if accounts.is_empty() {
                app.push_system("No accounts configured.".to_string(), true);
            } else {
                app.overlays.account_selector.open(accounts);
            }
        }
        Action::Extended(ExtendedAction::ToggleCostOverlay) => {
            app.overlays.cost_overlay_visible = !app.overlays.cost_overlay_visible;
        }
        Action::Extended(ExtendedAction::ToggleSessionPopup) => {
            app.overlays.session_popup_visible = !app.overlays.session_popup_visible;
            if app.overlays.session_popup_visible && app.conversation.focused_block.is_none() {
                let last_id = app.conversation.blocks.iter().rev().find_map(|e| match e {
                    BlockEntry::Conversation(b) => Some(b.id),
                    _ => None,
                });
                app.conversation.focused_block = last_id;
            }
        }
        Action::Extended(ExtendedAction::OpenToolToggle) => {
            let tools = app.tool_info.clone();
            app.overlays.tool_toggle.open(tools, &app.disabled_tools);
        }
        Action::Extended(ExtendedAction::TogglePromptImprove) => {
            app.prompt_improve = !app.prompt_improve;
            let state = if app.prompt_improve { "on" } else { "off" };
            app.push_system(format!("Prompt improve: {state}."), false);
        }

        // ── Daemon-forwarded toggles ────────────────
        Action::Extended(ExtendedAction::ToggleThinking) => {
            let next_level = app.thinking_level.next();
            bridge_attach_thinking_level_change(
                app,
                client,
                parity_tracker,
                SessionCommand::CycleThinkingLevel,
                next_level,
            );
        }
        Action::Extended(ExtendedAction::ToggleAutoTest) => {
            if app.auto_test_command.is_none() {
                app.push_system("No test command configured.".to_string(), true);
            } else {
                let is_enabled = !app.auto_test_enabled;
                client.send(SessionCommand::SetAutoTest {
                    enabled: is_enabled,
                    command: None,
                });
            }
        }

        // ── Quit ────────────────────────────────────
        Action::Core(CoreAction::Quit) => app.should_quit = true,
        Action::Core(CoreAction::Cancel) => {
            // In attach mode, Cancel/abort is handled in handle_key_event
        }

        _ => {}
    }
}

/// Handle panel-focused key events in attach mode.
///
/// Returns true if the key was consumed.
fn handle_panel_focused_key_attach(app: &mut App, key: crossterm::event::KeyEvent) -> bool {
    use clanker_tui_types::PanelAction;
    use crossterm::event::KeyCode;

    // Tab / Shift+Tab cycles focus
    if matches!(key.code, KeyCode::Tab) {
        app.apply_tiling_action(ratatui_hypertile::HypertileAction::FocusNext);
        return true;
    }
    if matches!(key.code, KeyCode::BackTab) {
        app.apply_tiling_action(ratatui_hypertile::HypertileAction::FocusPrev);
        return true;
    }

    // Delegate to focused panel
    if let Some(focused_id) = app.layout.focused_panel
        && let Some(panel) = app.panel_mut(focused_id)
    {
        let result = panel.handle_key_event(key);
        match result {
            Some(PanelAction::Consumed) => return true,
            Some(PanelAction::Unfocus) => {
                app.unfocus_panel();
                return true;
            }
            Some(PanelAction::SlashCommand(_cmd)) => return true,
            Some(PanelAction::FocusPanel(id)) => {
                app.focus_panel(id);
                return true;
            }
            _ => {}
        }
    }

    false
}

// ── Slash registry for attach mode ──────────────────────────────────────────

pub(crate) fn build_client_slash_registry() -> slash_commands::SlashRegistry {
    // We build the same registry as standalone mode so completion/help stay in
    // sync. Attach mode still decides per command whether to run locally,
    // bridge AgentCommand -> SessionCommand, or forward to the daemon.
    crate::modes::interactive::build_slash_registry(None)
}

// ── Remote attach (extracted to attach_remote.rs) ─────────────────────────
pub use super::attach_remote::*;

#[cfg(test)]
mod tests {
    use std::io::ErrorKind;
    use std::sync::Arc;

    use clanker_tui_types::BlockEntry;
    use clanker_tui_types::ConversationBlock;
    use clanker_tui_types::DisplayMessage;
    use clankers_agent::Agent;
    use clankers_controller::SessionController;
    use clankers_controller::client::ClientAdapter;
    use clankers_controller::client::is_client_side_command;
    use clankers_controller::config::ControllerConfig;
    use clankers_protocol::DaemonEvent;
    use clankers_protocol::PluginSummary;
    use clankers_protocol::SessionCommand;
    use clankers_tui::app::App;

    struct MockProvider;

    #[async_trait::async_trait]
    impl clankers_provider::Provider for MockProvider {
        async fn complete(
            &self,
            _request: clankers_provider::CompletionRequest,
            _tx: tokio::sync::mpsc::Sender<clankers_provider::streaming::StreamEvent>,
        ) -> clankers_provider::error::Result<()> {
            Ok(())
        }

        fn models(&self) -> &[clankers_provider::Model] {
            &[]
        }

        fn name(&self) -> &str {
            "mock"
        }
    }

    fn make_daemon_controller() -> SessionController {
        let provider = Arc::new(MockProvider);
        let agent = Agent::new(
            provider,
            vec![],
            clankers_config::settings::Settings::default(),
            "test-model".to_string(),
            "You are a test assistant.".to_string(),
        );
        SessionController::new(agent, ControllerConfig {
            session_id: "test-session".to_string(),
            model: "test-model".to_string(),
            ..Default::default()
        })
    }

    fn dummy_client() -> ClientAdapter {
        let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::unbounded_channel();
        let (_event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        ClientAdapter::from_channels(cmd_tx, event_rx)
    }

    fn capturing_client() -> (ClientAdapter, tokio::sync::mpsc::UnboundedReceiver<SessionCommand>) {
        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
        let (_event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        (ClientAdapter::from_channels(cmd_tx, event_rx), cmd_rx)
    }

    fn test_app() -> App {
        let mut app = App::new("test-model".to_string(), "/tmp".to_string(), crate::config::theme::detect_theme());
        app.session_id = "session-123".to_string();
        app.total_tokens = 321;
        app.total_cost = 1.25;
        app
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

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct ConversationSnapshot {
        blocks: Vec<BlockEntrySnapshot>,
        all_blocks: Vec<ConversationBlockSnapshot>,
        active_block: Option<ConversationBlockSnapshot>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum BlockEntrySnapshot {
        Conversation(ConversationBlockSnapshot),
        System(MessageSnapshot),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct ConversationBlockSnapshot {
        id: usize,
        prompt: String,
        responses: Vec<MessageSnapshot>,
        collapsed: bool,
        streaming: bool,
        error: Option<String>,
        tokens: usize,
        parent_block_id: Option<usize>,
        agent_msg_checkpoint: usize,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct MessageSnapshot {
        role: clanker_tui_types::MessageRole,
        content: String,
        tool_name: Option<String>,
        is_error: bool,
        image_count: usize,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct BlockMetadataSnapshot {
        started_at: chrono::DateTime<chrono::Utc>,
        finalized_hash: Option<String>,
    }

    fn conversation_snapshot(app: &App) -> ConversationSnapshot {
        ConversationSnapshot {
            blocks: app.conversation.blocks.iter().map(block_entry_snapshot).collect(),
            all_blocks: app.conversation.all_blocks.iter().map(conversation_block_snapshot).collect(),
            active_block: app.conversation.active_block.as_ref().map(conversation_block_snapshot),
        }
    }

    fn block_entry_snapshot(entry: &BlockEntry) -> BlockEntrySnapshot {
        match entry {
            BlockEntry::Conversation(block) => BlockEntrySnapshot::Conversation(conversation_block_snapshot(block)),
            BlockEntry::System(message) => BlockEntrySnapshot::System(message_snapshot(message)),
        }
    }

    fn conversation_block_snapshot(block: &ConversationBlock) -> ConversationBlockSnapshot {
        ConversationBlockSnapshot {
            id: block.id,
            prompt: block.prompt.clone(),
            responses: block.responses.iter().map(message_snapshot).collect(),
            collapsed: block.collapsed,
            streaming: block.streaming,
            error: block.error.clone(),
            tokens: block.tokens,
            parent_block_id: block.parent_block_id,
            agent_msg_checkpoint: block.agent_msg_checkpoint,
        }
    }

    fn message_snapshot(message: &DisplayMessage) -> MessageSnapshot {
        MessageSnapshot {
            role: message.role.clone(),
            content: message.content.clone(),
            tool_name: message.tool_name.clone(),
            is_error: message.is_error,
            image_count: message.images.len(),
        }
    }

    fn block_metadata_snapshot(app: &App) -> Vec<BlockMetadataSnapshot> {
        app.conversation
            .blocks
            .iter()
            .filter_map(|entry| match entry {
                BlockEntry::Conversation(block) => Some(BlockMetadataSnapshot {
                    started_at: block.started_at,
                    finalized_hash: block.finalized_hash.clone(),
                }),
                BlockEntry::System(_) => None,
            })
            .collect()
    }

    fn drain_session_commands(rx: &mut tokio::sync::mpsc::UnboundedReceiver<SessionCommand>) -> Vec<SessionCommand> {
        let mut commands = Vec::new();
        while let Ok(command) = rx.try_recv() {
            commands.push(command);
        }
        commands
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct LayoutSnapshot {
        pane_kinds: Vec<String>,
        focused_panel: Option<crate::tui::panel::PanelId>,
        zoomed: bool,
    }

    fn layout_snapshot(app: &App) -> LayoutSnapshot {
        let mut pane_kinds = app
            .layout
            .pane_registry
            .pane_ids()
            .into_iter()
            .map(|pane_id| {
                let kind = app
                    .layout
                    .pane_registry
                    .kind(pane_id)
                    .map(|kind| format!("{kind:?}"))
                    .unwrap_or_else(|| "Missing".to_string());
                format!("{pane_id:?}:{kind}")
            })
            .collect::<Vec<_>>();
        pane_kinds.sort();
        LayoutSnapshot {
            pane_kinds,
            focused_panel: app.layout.focused_panel,
            zoomed: app.layout.zoom_state.is_some(),
        }
    }

    fn todo_summary(app: &App) -> String {
        app.panels
            .downcast_ref::<crate::tui::components::todo_panel::TodoPanel>(crate::tui::panel::PanelId::Todo)
            .expect("todo panel registered at startup")
            .summary()
    }

    fn run_standalone_slash(app: &mut App, text: &str) {
        let registry = crate::modes::interactive::build_slash_registry(None);
        let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel();
        let (panel_tx, _panel_rx) = tokio::sync::mpsc::unbounded_channel();
        let db: Option<crate::db::Db> = None;
        let mut session_manager = None;
        let (command, args) = crate::slash_commands::parse_command(text).expect("slash command parses");
        {
            let mut ctx = crate::slash_commands::handlers::SlashContext {
                app,
                cmd_tx: &cmd_tx,
                plugin_manager: None,
                panel_tx: &panel_tx,
                db: &db,
                session_manager: &mut session_manager,
            };
            registry.dispatch(&command, &args, &mut ctx);
        }
        apply_standalone_agent_commands(app, &mut cmd_rx);
    }

    fn apply_standalone_agent_commands(
        app: &mut App,
        cmd_rx: &mut tokio::sync::mpsc::UnboundedReceiver<crate::modes::interactive::AgentCommand>,
    ) {
        while let Ok(agent_cmd) = cmd_rx.try_recv() {
            match agent_cmd {
                crate::modes::interactive::AgentCommand::SetThinkingLevel(level) => {
                    super::apply_standalone_thinking_level(app, level);
                }
                crate::modes::interactive::AgentCommand::CycleThinkingLevel => {
                    let next_level = app.thinking_level.next();
                    super::apply_standalone_thinking_level(app, next_level);
                }
                _ => {}
            }
        }
    }

    fn run_attach_slash_locally(app: &mut App, text: &str) -> Vec<SessionCommand> {
        let registry = super::build_client_slash_registry();
        let (client, mut cmd_rx) = capturing_client();
        let mut parity_tracker = super::AttachParityTracker::default();
        let (command, args) = crate::slash_commands::parse_command(text).expect("slash command parses");

        super::dispatch_attach_slash(app, &client, &command, &args, &registry, &mut parity_tracker);
        drain_session_commands(&mut cmd_rx)
    }

    fn parse_test_timestamp(rfc3339: &str) -> chrono::DateTime<chrono::Utc> {
        match chrono::DateTime::parse_from_rfc3339(rfc3339) {
            Ok(timestamp) => timestamp.with_timezone(&chrono::Utc),
            Err(error) => panic!("test timestamp must parse: {error}"),
        }
    }

    fn replay_messages() -> Vec<clanker_message::AgentMessage> {
        vec![
            clanker_message::AgentMessage::User(clanker_message::UserMessage {
                id: clanker_message::MessageId::new("u1"),
                content: vec![clanker_message::Content::Text {
                    text: "hello".to_string(),
                }],
                timestamp: parse_test_timestamp("2026-04-22T12:34:56Z"),
            }),
            clanker_message::AgentMessage::Assistant(clanker_message::AssistantMessage {
                id: clanker_message::MessageId::new("a1"),
                content: vec![
                    clanker_message::Content::ToolUse {
                        id: "call-1".to_string(),
                        name: "bash".to_string(),
                        input: serde_json::json!({"command": "ls"}),
                    },
                    clanker_message::Content::Text {
                        text: "done".to_string(),
                    },
                ],
                model: "test-model".to_string(),
                usage: clanker_message::Usage::default(),
                stop_reason: clanker_message::StopReason::Stop,
                timestamp: parse_test_timestamp("2026-04-22T12:35:10Z"),
            }),
            clanker_message::AgentMessage::ToolResult(clanker_message::ToolResultMessage {
                id: clanker_message::MessageId::new("t1"),
                call_id: "call-1".to_string(),
                tool_name: "bash".to_string(),
                content: vec![clanker_message::Content::Text {
                    text: "tool output".to_string(),
                }],
                is_error: false,
                details: None,
                timestamp: parse_test_timestamp("2026-04-22T12:35:20Z"),
            }),
        ]
    }

    fn assert_local_attach_matches_standalone<FSetup, FAssert>(text: &str, setup: FSetup, assert_extra: FAssert)
    where
        FSetup: Fn(&mut App),
        FAssert: Fn(&App, &App),
    {
        let mut standalone = test_app();
        let mut attached = test_app();

        setup(&mut standalone);
        setup(&mut attached);

        run_standalone_slash(&mut standalone, text);
        let session_commands = run_attach_slash_locally(&mut attached, text);

        assert!(session_commands.is_empty(), "expected no daemon commands for {text}, got {session_commands:?}");
        assert_eq!(conversation_snapshot(&attached), conversation_snapshot(&standalone), "{text}");
        assert_extra(&attached, &standalone);
    }

    async fn run_attach_slash_through_daemon(app: &mut App, text: &str) {
        let registry = super::build_client_slash_registry();
        let (client, mut cmd_rx) = capturing_client();
        let event_client = dummy_client();
        let mut controller = make_daemon_controller();
        let mut is_replaying_history = false;
        let mut parity_tracker = super::AttachParityTracker::default();
        let (command, args) = crate::slash_commands::parse_command(text).expect("slash command parses");

        super::dispatch_attach_slash(app, &client, &command, &args, &registry, &mut parity_tracker);

        for session_command in drain_session_commands(&mut cmd_rx) {
            controller.handle_command(session_command).await;
            for event in controller.drain_events() {
                super::process_daemon_event(
                    app,
                    &event_client,
                    &event,
                    &mut is_replaying_history,
                    0,
                    &mut parity_tracker,
                );
            }
        }
    }

    #[test]
    fn history_replay_matches_session_restore_block_metadata() {
        let messages = replay_messages();
        let mut standalone = test_app();
        let mut attached = test_app();
        let client = dummy_client();
        let mut is_replaying_history = true;
        let mut parity_tracker = super::AttachParityTracker::default();

        crate::modes::session_restore::restore_display_blocks(&mut standalone, &messages);

        for message in &messages {
            let block = serde_json::to_value(message).expect("history message serializes");
            super::process_daemon_event(
                &mut attached,
                &client,
                &DaemonEvent::HistoryBlock { block },
                &mut is_replaying_history,
                0,
                &mut parity_tracker,
            );
        }
        super::process_daemon_event(
            &mut attached,
            &client,
            &DaemonEvent::HistoryEnd,
            &mut is_replaying_history,
            0,
            &mut parity_tracker,
        );

        assert_eq!(block_metadata_snapshot(&attached), block_metadata_snapshot(&standalone));
    }

    #[test]
    fn session_socket_retry_policy_covers_transient_errors() {
        let missing = std::io::Error::from(ErrorKind::NotFound);
        let refused = std::io::Error::from(ErrorKind::ConnectionRefused);
        let denied = std::io::Error::from(ErrorKind::PermissionDenied);

        assert!(super::should_retry_session_socket_connect(&missing));
        assert!(super::should_retry_session_socket_connect(&refused));
        assert!(!super::should_retry_session_socket_connect(&denied));
    }

    #[test]
    fn test_client_side_commands_classified_correctly() {
        // Client-side commands stay local
        assert!(is_client_side_command("quit"));
        assert!(is_client_side_command("detach"));
        assert!(is_client_side_command("zoom"));
        assert!(is_client_side_command("layout"));
        assert!(is_client_side_command("theme"));
        assert!(is_client_side_command("help"));
        assert!(is_client_side_command("copy"));

        // Agent-side commands go to daemon
        assert!(!is_client_side_command("model"));
        assert!(!is_client_side_command("thinking"));
        assert!(!is_client_side_command("clear"));
        assert!(!is_client_side_command("compact"));
        assert!(!is_client_side_command("autotest"));
        assert!(!is_client_side_command("loop"));
    }

    #[test]
    fn attach_regular_prompt_routes_to_daemon_session_prompt() {
        let mut app = test_app();
        let (client, mut cmd_rx) = capturing_client();
        let registry = super::build_client_slash_registry();
        let mut parity_tracker = super::AttachParityTracker::default();

        super::submit_input_attach(&mut app, &client, "hello daemon", &registry, &mut parity_tracker);

        assert!(matches!(
            drain_session_commands(&mut cmd_rx).as_slice(),
            [SessionCommand::Prompt { text, images }] if text == "hello daemon" && images.is_empty()
        ));
    }

    #[test]
    fn route_attach_slash_keeps_safe_session_subcommands_local() {
        assert_eq!(super::route_attach_slash("session", ""), super::AttachSlashRoute::RegistryLocal);
        assert_eq!(super::route_attach_slash("session", "list 5"), super::AttachSlashRoute::RegistryLocal);
        assert_eq!(super::route_attach_slash("session", "delete abc"), super::AttachSlashRoute::RegistryLocal);
        assert_eq!(super::route_attach_slash("session", "resume abc"), super::AttachSlashRoute::ForwardToDaemon);
        assert_eq!(super::route_attach_slash("model", ""), super::AttachSlashRoute::RegistryLocal);
        assert_eq!(super::route_attach_slash("model", "sonnet"), super::AttachSlashRoute::ForwardToDaemon);
        assert_eq!(super::route_attach_slash("role", ""), super::AttachSlashRoute::RegistryLocal);
        assert_eq!(super::route_attach_slash("role", "planner"), super::AttachSlashRoute::ForwardToDaemon);
        assert_eq!(super::route_attach_slash("think", ""), super::AttachSlashRoute::RegistryLocal);
        assert_eq!(super::route_attach_slash("think", "high"), super::AttachSlashRoute::RegistryLocal);
        assert_eq!(super::route_attach_slash("compress", ""), super::AttachSlashRoute::RegistryLocal);
        assert_eq!(super::route_attach_slash("plugin", ""), super::AttachSlashRoute::GetPlugins);
    }

    #[test]
    fn attach_help_advertised_local_commands_have_matching_routes() {
        let advertised_routes = [
            ("status", ""),
            ("usage", ""),
            ("version", ""),
            ("router", ""),
            ("model", ""),
            ("role", ""),
            ("session", "list"),
            ("session", "delete missing-session"),
            ("session", "purge"),
            ("cd", ""),
            ("shell", ""),
            ("export", ""),
            ("layout", ""),
            ("preview", ""),
            ("editor", ""),
            ("todo", ""),
            ("tools", ""),
            ("think", "high"),
            ("compact", ""),
            ("compress", ""),
            ("plugin", ""),
            ("quit", ""),
            ("detach", ""),
            ("zoom", ""),
        ];

        for (command, args) in advertised_routes {
            assert_ne!(
                super::route_attach_slash(command, args),
                super::AttachSlashRoute::ForwardToDaemon,
                "advertised attach command /{command} {args} must stay local or use plugin inventory fetch"
            );
        }
        assert_eq!(super::route_attach_slash("session", "resume abc"), super::AttachSlashRoute::ForwardToDaemon);
        assert_eq!(super::route_attach_slash("unknown", ""), super::AttachSlashRoute::ForwardToDaemon);
    }

    #[test]
    fn thinking_ack_suppression_stays_narrow_and_budgeted() {
        let thinking_level_changed = DaemonEvent::ThinkingLevelChanged {
            from: "off".to_string(),
            to: "high".to_string(),
        };
        let thinking_ack = DaemonEvent::SystemMessage {
            text: "Thinking: off → high".to_string(),
            is_error: false,
        };
        let mut parity_tracker = super::AttachParityTracker::default();

        parity_tracker.expect_thinking_ack_message();

        assert!(!parity_tracker.should_suppress(&thinking_level_changed));
        assert!(parity_tracker.should_suppress(&thinking_ack));
        assert!(!parity_tracker.should_suppress(&thinking_ack));
    }

    #[tokio::test]
    async fn set_thinking_level_bridge_emits_only_system_message_ack() {
        let mut controller = make_daemon_controller();

        controller
            .handle_command(SessionCommand::SetThinkingLevel {
                level: "high".to_string(),
            })
            .await;

        let events = controller.drain_events();
        assert!(events.iter().any(|event| super::is_thinking_ack_message(event)));
        assert!(!events.iter().any(|event| matches!(event, DaemonEvent::ThinkingLevelChanged { .. })));
    }

    #[tokio::test]
    async fn cycle_thinking_level_bridge_emits_only_system_message_ack() {
        let mut controller = make_daemon_controller();

        controller.handle_command(SessionCommand::CycleThinkingLevel).await;

        let events = controller.drain_events();
        assert!(events.iter().any(|event| super::is_thinking_ack_message(event)));
        assert!(!events.iter().any(|event| matches!(event, DaemonEvent::ThinkingLevelChanged { .. })));
    }

    #[test]
    fn attach_think_cycle_bridge_updates_local_state_and_emits_cycle_command() {
        let mut attached = test_app();
        let initial_level = attached.thinking_level;
        let session_commands = run_attach_slash_locally(&mut attached, "/think");

        assert!(matches!(session_commands.as_slice(), [SessionCommand::CycleThinkingLevel]));
        assert_eq!(attached.thinking_level, initial_level.next());
        assert_eq!(
            system_texts(&attached).last().cloned(),
            Some(super::format_attach_thinking_message(initial_level.next())),
        );
    }

    #[test]
    fn attach_plugin_route_requests_plugin_inventory() {
        let mut attached = test_app();
        let session_commands = run_attach_slash_locally(&mut attached, "/plugin");

        assert!(matches!(session_commands.as_slice(), [SessionCommand::GetPlugins]));
    }

    #[test]
    fn attach_help_describes_local_handling_not_parity() {
        let mut app = test_app();

        super::handle_client_side_slash(&mut app, "help", "");

        let messages = system_texts(&app);
        assert!(messages.iter().any(|message| message.contains("locally handled slash commands include")));
        assert!(messages.iter().any(|message| message.contains("/model (no args)")));
        assert!(messages.iter().any(|message| message.contains("/role (no args)")));
        assert!(messages.iter().any(|message| message.contains("/think [level]")));
        assert!(messages.iter().any(|message| message.contains("/compress")));
        assert!(messages.iter().any(|message| message.contains("/plugin")));
        assert!(messages.iter().any(|message| message.contains("/think with no args cycles level")));
        assert!(messages.iter().any(|message| message.contains("Unlisted commands generally forward to daemon")));
        assert!(!messages.iter().any(|message| message.contains("local parity commands")));
        assert!(!messages.iter().any(|message| message.contains("other commands forward")));
    }

    #[test]
    fn attach_local_informational_commands_match_standalone_output() {
        for text in [
            "/status", "/usage", "/version", "/router", "/model", "/role", "/session", "/cd", "/shell", "/layout",
            "/todo",
        ] {
            assert_local_attach_matches_standalone(text, |_| {}, |_, _| {});
        }
    }

    #[test]
    fn attach_local_session_management_commands_match_standalone_output() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let cwd = tempdir.path().join("attach-session-sandbox");
        std::fs::create_dir_all(&cwd).expect("sandbox cwd created");
        let cwd = cwd.to_string_lossy().to_string();

        for text in [
            "/session list 1",
            "/session delete definitely-missing-session",
            "/session purge",
        ] {
            assert_local_attach_matches_standalone(text, |app| app.cwd = cwd.clone(), |_, _| {});
        }
    }

    #[test]
    fn attach_local_cd_change_matches_standalone_state() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let child = tempdir.path().join("child");
        std::fs::create_dir_all(&child).expect("child dir created");
        let root = tempdir.path().to_string_lossy().to_string();
        let expected = child.canonicalize().expect("child canonicalized").to_string_lossy().to_string();

        assert_local_attach_matches_standalone(
            "/cd child",
            |app| app.cwd = root.clone(),
            |attached, standalone| {
                assert_eq!(attached.cwd, standalone.cwd);
                assert_eq!(attached.cwd, expected);
            },
        );
    }

    #[test]
    fn attach_local_shell_exec_matches_standalone_output() {
        assert_local_attach_matches_standalone("/shell printf attach-shell-ok", |_| {}, |_, _| {});
    }

    #[test]
    fn attach_local_export_matches_standalone_output_and_file() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let cwd = tempdir.path().to_string_lossy().to_string();
        let export_path = tempdir.path().join("attach-export.md");
        let export_arg = export_path.to_string_lossy().to_string();
        let command = format!("/export {export_arg}");

        assert_local_attach_matches_standalone(
            &command,
            |app| {
                app.cwd = cwd.clone();
                app.push_system("export me".to_string(), false);
            },
            |_, _| {},
        );

        let exported = std::fs::read_to_string(&export_path).expect("exported file readable");
        assert!(exported.contains("## System\nexport me"));
    }

    #[test]
    fn attach_local_layout_change_matches_standalone_state() {
        assert_local_attach_matches_standalone(
            "/layout wide",
            |_| {},
            |attached, standalone| {
                assert_eq!(layout_snapshot(attached), layout_snapshot(standalone));
            },
        );
    }

    #[test]
    fn attach_local_preview_matches_standalone_output() {
        assert_local_attach_matches_standalone("/preview ## Attach Preview", |_| {}, |_, _| {});
    }

    #[test]
    fn attach_local_editor_matches_standalone_state() {
        assert_local_attach_matches_standalone(
            "/editor",
            |_| {},
            |attached, standalone| {
                assert_eq!(attached.open_editor_requested, standalone.open_editor_requested);
                assert!(attached.open_editor_requested);
            },
        );
    }

    #[test]
    fn attach_local_todo_add_matches_standalone_state() {
        assert_local_attach_matches_standalone(
            "/todo add write parity coverage",
            |_| {},
            |attached, standalone| {
                assert_eq!(todo_summary(attached), todo_summary(standalone));
            },
        );
    }

    #[test]
    fn attach_local_tools_listing_matches_standalone_output() {
        let tool_rows = vec![
            ("bash".to_string(), "Run shell commands".to_string(), "built-in".to_string()),
            ("read".to_string(), "Read a file".to_string(), "built-in".to_string()),
        ];

        assert_local_attach_matches_standalone(
            "/tools",
            |app| {
                app.tool_info = tool_rows.clone();
            },
            |_, _| {},
        );
    }

    #[test]
    fn attach_tools_disable_updates_local_state_before_daemon_ack() {
        let mut standalone = test_app();
        let mut attached = test_app();
        let tool_rows = vec![
            ("bash".to_string(), "Run shell commands".to_string(), "built-in".to_string()),
            ("read".to_string(), "Read a file".to_string(), "built-in".to_string()),
        ];
        standalone.tool_info = tool_rows.clone();
        attached.tool_info = tool_rows;

        run_standalone_slash(&mut standalone, "/tools disable bash");
        let session_commands = run_attach_slash_locally(&mut attached, "/tools disable bash");

        assert_eq!(attached.disabled_tools, standalone.disabled_tools);
        assert_eq!(conversation_snapshot(&attached), conversation_snapshot(&standalone));
        assert!(matches!(
            session_commands.as_slice(),
            [SessionCommand::SetDisabledTools { tools }] if tools == &vec!["bash".to_string()]
        ));
    }

    #[tokio::test]
    async fn attach_tools_disable_matches_standalone_after_daemon_roundtrip() {
        let mut standalone = test_app();
        let mut attached = test_app();
        let tool_rows = vec![
            ("bash".to_string(), "Run shell commands".to_string(), "built-in".to_string()),
            ("read".to_string(), "Read a file".to_string(), "built-in".to_string()),
        ];
        standalone.tool_info = tool_rows.clone();
        attached.tool_info = tool_rows;

        run_standalone_slash(&mut standalone, "/tools disable bash");
        run_attach_slash_through_daemon(&mut attached, "/tools disable bash").await;

        assert_eq!(attached.disabled_tools, standalone.disabled_tools);
        assert_eq!(conversation_snapshot(&attached), conversation_snapshot(&standalone));
    }

    #[tokio::test]
    async fn attach_compact_matches_standalone_after_daemon_roundtrip() {
        let mut standalone = test_app();
        let mut attached = test_app();

        run_standalone_slash(&mut standalone, "/compact");
        run_attach_slash_through_daemon(&mut attached, "/compact").await;

        assert_eq!(conversation_snapshot(&attached), conversation_snapshot(&standalone));
    }

    #[tokio::test]
    async fn attach_compress_matches_standalone_after_daemon_roundtrip() {
        let mut standalone = test_app();
        let mut attached = test_app();

        run_standalone_slash(&mut standalone, "/compress");
        run_attach_slash_through_daemon(&mut attached, "/compress").await;

        assert_eq!(conversation_snapshot(&attached), conversation_snapshot(&standalone));
    }

    #[tokio::test]
    async fn attach_think_matches_standalone_after_daemon_roundtrip() {
        let mut standalone = test_app();
        let mut attached = test_app();

        run_standalone_slash(&mut standalone, "/think high");
        run_attach_slash_through_daemon(&mut attached, "/think high").await;

        assert_eq!(attached.thinking_enabled, standalone.thinking_enabled);
        assert_eq!(attached.thinking_level, standalone.thinking_level);
        assert_eq!(conversation_snapshot(&attached), conversation_snapshot(&standalone));
    }

    #[tokio::test]
    async fn attach_think_cycle_matches_standalone_after_daemon_roundtrip() {
        let mut standalone = test_app();
        let mut attached = test_app();

        run_standalone_slash(&mut standalone, "/think");
        run_attach_slash_through_daemon(&mut attached, "/think").await;

        assert_eq!(attached.thinking_enabled, standalone.thinking_enabled);
        assert_eq!(attached.thinking_level, standalone.thinking_level);
        assert_eq!(conversation_snapshot(&attached), conversation_snapshot(&standalone));
    }

    #[test]
    fn plugin_list_event_renders_stdio_metadata_in_attach_mode() {
        let mut app = App::new("test-model".to_string(), ".".to_string(), crate::config::theme::detect_theme());
        let client = dummy_client();
        let mut is_replaying_history = false;
        let mut parity_tracker = super::AttachParityTracker::default();
        let event = DaemonEvent::PluginList {
            plugins: vec![PluginSummary {
                name: "stdio-demo".to_string(),
                version: "0.1.0".to_string(),
                state: "Backoff".to_string(),
                tools: vec!["stdio_demo_tool".to_string()],
                permissions: vec!["ui".to_string()],
                kind: Some("stdio".to_string()),
                last_error: Some("launch failed".to_string()),
            }],
        };

        super::process_daemon_event(&mut app, &client, &event, &mut is_replaying_history, 0, &mut parity_tracker);

        let plugins = app.daemon_plugins.expect("daemon plugins stored");
        assert_eq!(plugins[0].kind.as_deref(), Some("stdio"));
        assert_eq!(plugins[0].state, "Backoff");
        assert_eq!(plugins[0].tools, vec!["stdio_demo_tool".to_string()]);

        match app.conversation.blocks.last().expect("plugin list message appended") {
            BlockEntry::System(message) => {
                assert!(message.content.contains("stdio-demo"));
                assert!(message.content.contains("stdio"));
                assert!(message.content.contains("Backoff"));
                assert!(message.content.contains("stdio_demo_tool"));
                assert!(message.content.contains("launch failed"));
            }
            other => panic!("expected system message, got {other:?}"),
        }
    }

    #[test]
    fn plugin_runtime_events_update_attach_plugin_ui_state() {
        let mut app = App::new("test-model".to_string(), ".".to_string(), crate::config::theme::detect_theme());
        let client = dummy_client();
        let mut is_replaying_history = false;
        let mut parity_tracker = super::AttachParityTracker::default();

        super::process_daemon_event(
            &mut app,
            &client,
            &DaemonEvent::PluginStatus {
                plugin: "stdio-demo".to_string(),
                text: Some("ready".to_string()),
                color: Some("green".to_string()),
            },
            &mut is_replaying_history,
            0,
            &mut parity_tracker,
        );
        super::process_daemon_event(
            &mut app,
            &client,
            &DaemonEvent::PluginNotify {
                plugin: "stdio-demo".to_string(),
                message: "hello".to_string(),
                level: "info".to_string(),
            },
            &mut is_replaying_history,
            0,
            &mut parity_tracker,
        );
        super::process_daemon_event(
            &mut app,
            &client,
            &DaemonEvent::PluginWidget {
                plugin: "stdio-demo".to_string(),
                widget: Some(serde_json::json!({
                    "type": "Text",
                    "content": "widget body",
                    "bold": false,
                    "color": null
                })),
            },
            &mut is_replaying_history,
            0,
            &mut parity_tracker,
        );
        super::process_daemon_event(
            &mut app,
            &client,
            &DaemonEvent::SystemMessage {
                text: "🔌 stdio-demo: saw tool_call".to_string(),
                is_error: false,
            },
            &mut is_replaying_history,
            0,
            &mut parity_tracker,
        );

        assert_eq!(app.plugin_ui.status_segments["stdio-demo"].text, "ready");
        assert_eq!(app.plugin_ui.status_segments["stdio-demo"].color.as_deref(), Some("green"));
        assert_eq!(app.plugin_ui.notifications.len(), 1);
        assert!(app.plugin_ui.widgets.contains_key("stdio-demo"));
        match app.conversation.blocks.last().expect("system message appended") {
            BlockEntry::System(message) => assert!(message.content.contains("stdio-demo")),
            other => panic!("expected system message, got {other:?}"),
        }
    }
}
