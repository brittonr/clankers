//! TUI attach mode — connect to a daemon session via socket.
//!
//! Instead of running an in-process agent, the TUI reads `DaemonEvent`s from a
//! `ClientAdapter` connected to a daemon session socket. User input is forwarded
//! as `SessionCommand::Prompt`. Client-side commands (zoom, layout, theme, quit)
//! are handled locally; everything else goes to the daemon.

use std::io;
use std::time::Duration;

use clankers_controller::client::ClientAdapter;
use clankers_controller::client::is_client_side_command;
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
use crate::error::Result;
use crate::slash_commands;
use crate::tui::app::App;
use crate::tui::event as tui_event;
use crate::tui::event::AppEvent;
use crate::tui::render;
use crate::tui::theme::Theme;

// ── Entry point ─────────────────────────────────────────────────────────────

/// Launch the TUI in attach mode, connecting to a daemon session.
pub async fn run_attach(
    session_id: Option<String>,
    create_new: bool,
    model: Option<String>,
    settings: &Settings,
) -> Result<()> {
    // Resolve the session socket path
    let (resolved_session_id, socket_path) = resolve_session(session_id, create_new, model).await?;

    info!("attaching to session {resolved_session_id} at {socket_path}");

    // Connect to the session socket
    let stream = UnixStream::connect(&socket_path).await.map_err(|e| {
        crate::error::Error::Provider {
            message: format!("Cannot connect to session socket {socket_path}: {e}"),
        }
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

    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let theme = Theme::dark();
    let keymap = settings.keymap.clone().into_keymap();

    let mut app = App::new(display_model.clone(), cwd, theme);
    app.session_id = resolved_session_id.clone();
    app.highlighter = Box::new(crate::util::syntax::SyntectHighlighter);

    // Minimal slash registry for client-side commands only
    let slash_registry = build_client_slash_registry();
    app.set_completion_source(Box::new(clankers_tui_types::CompletionSnapshot::from_source(
        &slash_registry,
    )));

    // Build leader menu from builtins
    crate::modes::interactive::rebuild_leader_menu(&mut app, None, settings);

    app.connection_mode = clankers_tui_types::ConnectionMode::Attached;

    app.push_system(
        format!(
            "attached to session {} (model: {}, prompt hash: {})",
            resolved_session_id,
            display_model,
            if session_hash.is_empty() {
                "n/a"
            } else {
                &session_hash
            }
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
        clankers_tui_types::ConnectionMode::Attached,
        RecoveryMode::ExplicitAttach,
    )
    .await;

    super::common::restore_terminal(&mut term);
    result
}

// ── Auto-daemon mode ────────────────────────────────────────────────────────

/// Options for the auto-daemon attach flow (default interactive mode through
/// a background daemon).
pub struct AutoDaemonOptions {
    pub model: String,
    pub system_prompt: String,
    pub settings: Settings,
    pub resume_id: Option<String>,
    pub continue_last: bool,
    pub no_session: bool,
    pub cwd: String,
    /// Enable extended thinking (from --thinking CLI flag).
    pub thinking: bool,
}

/// Default interactive mode through a background daemon.
///
/// This is the Phase 3 "flip the switch" entry point: `clankers` (no
/// subcommand) auto-starts a daemon, creates a session with the caller's
/// CLI options, and attaches the TUI to it.
pub async fn run_auto_daemon_attach(opts: AutoDaemonOptions) -> Result<()> {
    // 1. Ensure a daemon is running (starts one in the background if needed)
    crate::commands::daemon::ensure_daemon_running().await?;

    // 2. Create or resume a session on the daemon
    let create_cmd = ControlCommand::CreateSession {
        model: Some(opts.model.clone()),
        system_prompt: Some(opts.system_prompt),
        token: None,
        resume_id: if opts.no_session { None } else { opts.resume_id.clone() },
        continue_last: if opts.no_session { false } else { opts.continue_last },
        cwd: Some(opts.cwd.clone()),
    };

    let resp = send_control(create_cmd).await?;
    let (session_id, socket_path) = match resp {
        ControlResponse::Created { session_id, socket_path } => (session_id, socket_path),
        ControlResponse::Error { message } => {
            return Err(crate::error::Error::Provider {
                message: format!("Failed to create daemon session: {message}"),
            });
        }
        other => {
            return Err(crate::error::Error::Provider {
                message: format!("Unexpected response from daemon: {other:?}"),
            });
        }
    };

    info!("auto-daemon: created session {session_id} at {socket_path}");

    // Guard ensures session is killed even on panic/signal exit.
    let _guard = SessionGuard::new(session_id.clone());

    // 3. Connect to the session socket
    let stream = UnixStream::connect(&socket_path).await.map_err(|e| {
        crate::error::Error::Provider {
            message: format!("Cannot connect to session socket {socket_path}: {e}"),
        }
    })?;

    let mut client = ClientAdapter::connect(stream, "clankers-tui", None, Some(session_id.clone()))
        .await
        .map_err(|e| crate::error::Error::Provider {
            message: format!("Handshake failed: {e}"),
        })?;

    // Read initial SessionInfo
    let model_name = match client.recv().await {
        Some(DaemonEvent::SessionInfo { model, .. }) => model,
        Some(other) => {
            warn!("expected SessionInfo, got: {other:?}");
            opts.model.clone()
        }
        None => {
            return Err(crate::error::Error::Provider {
                message: "Session disconnected before sending SessionInfo".to_string(),
            });
        }
    };

    client.replay_history();

    // Forward CLI flags that translate to session commands
    if opts.thinking {
        client.send(SessionCommand::SetThinkingLevel {
            level: "high".to_string(),
        });
    }

    // 4. Set up the terminal and run the attach event loop
    let mut term = super::common::init_terminal()?;

    let display_model = if model_name.is_empty() {
        opts.model.clone()
    } else {
        model_name
    };

    let recovery_cwd = opts.cwd.clone();
    let theme = Theme::dark();
    let keymap = opts.settings.keymap.clone().into_keymap();

    let mut app = App::new(display_model.clone(), opts.cwd, theme);
    app.session_id = session_id.clone();
    app.highlighter = Box::new(crate::util::syntax::SyntectHighlighter);

    let slash_registry = build_client_slash_registry();
    app.set_completion_source(Box::new(clankers_tui_types::CompletionSnapshot::from_source(
        &slash_registry,
    )));

    crate::modes::interactive::rebuild_leader_menu(&mut app, None, &opts.settings);

    // Auto-daemon mode stays Embedded (default) — no "ATTACHED" badge.
    // The user ran `clankers` and shouldn't see implementation details.

    let resumed = opts.resume_id.is_some() || opts.continue_last;
    if resumed {
        app.push_system(
            format!(
                "clankers — {} — resumed session {} — keymap: {} — press i to start typing",
                display_model, session_id, keymap.preset
            ),
            false,
        );
    } else {
        app.push_system(
            format!(
                "clankers — {} — keymap: {} — press i to start typing",
                display_model, keymap.preset
            ),
            false,
        );
    }

    let max_subagent_panes = opts.settings.max_subagent_panes;

    let recovery = RecoveryMode::AutoDaemon {
        session_id: session_id.clone(),
        model: opts.model.clone(),
        cwd: recovery_cwd,
    };

    let result = run_attach_with_reconnect(
        &mut term,
        &mut app,
        client,
        keymap,
        &slash_registry,
        max_subagent_panes,
        &socket_path,
        &session_id,
        clankers_tui_types::ConnectionMode::Embedded,
        recovery,
    )
    .await;

    super::common::restore_terminal(&mut term);

    // SessionGuard::drop fires here (or on panic/signal unwind) to kill the
    // session. No manual cleanup needed.

    result
}

/// Run the attach event loop with automatic reconnection on disconnect.
async fn run_attach_with_reconnect(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    mut client: ClientAdapter,
    keymap: Keymap,
    slash_registry: &slash_commands::SlashRegistry,
    max_subagent_panes: usize,
    socket_path: &str,
    session_id: &str,
    restore_mode: clankers_tui_types::ConnectionMode,
    recovery_mode: RecoveryMode,
) -> Result<()> {
    let mut replaying_history = true;

    loop {
        terminal
            .draw(|frame| render::render(frame, app))
            .map_err(|e| crate::error::Error::Tui {
                message: format!("Render failed: {e}"),
            })?;

        if app.should_quit {
            client.disconnect();
            break;
        }

        // Drain daemon events
        drain_daemon_events(app, &mut client, &mut replaying_history, max_subagent_panes);

        // Detect disconnect and attempt reconnection
        if client.is_disconnected()
            && app.connection_mode != clankers_tui_types::ConnectionMode::Reconnecting
        {
            app.connection_mode = clankers_tui_types::ConnectionMode::Reconnecting;
            app.push_system(
                "Connection to daemon lost. Attempting to reconnect...".to_string(),
                true,
            );

            // First, try reconnecting to the existing socket (transient glitch).
            match try_reconnect(socket_path, session_id).await {
                Some(new_client) => {
                    client = new_client;
                    client.replay_history();
                    replaying_history = true;
                    app.connection_mode = restore_mode.clone();
                    app.push_system("Reconnected to daemon session.".to_string(), false);
                }
                None => {
                    // Socket reconnect failed. For auto-daemon, try restarting
                    // the daemon and resuming the session.
                    match &recovery_mode {
                        RecoveryMode::AutoDaemon { session_id: sid, model, cwd } => {
                            app.push_system(
                                "Restarting daemon...".to_string(),
                                true,
                            );
                            match try_recover_daemon(sid, model, cwd).await {
                                Ok((new_client, new_socket_path, resumed)) => {
                                    client = new_client;
                                    client.replay_history();
                                    replaying_history = true;
                                    app.connection_mode = restore_mode.clone();
                                    if resumed {
                                        app.push_system(
                                            "Session resumed after daemon restart.".to_string(),
                                            false,
                                        );
                                    } else {
                                        app.push_system(
                                            "Session history lost — started fresh after daemon restart.".to_string(),
                                            true,
                                        );
                                    }
                                    info!(
                                        "auto-daemon: recovered to new socket {new_socket_path}"
                                    );
                                }
                                Err(e) => {
                                    warn!("auto-daemon: daemon recovery failed: {e}");
                                    app.push_system(
                                        format!(
                                            "Daemon recovery failed: {e}. Use /quit to exit."
                                        ),
                                        true,
                                    );
                                }
                            }
                        }
                        RecoveryMode::ExplicitAttach => {
                            app.push_system(
                                "Failed to reconnect after multiple attempts. Use /quit to exit."
                                    .to_string(),
                                true,
                            );
                        }
                    }
                }
            }
        }

        // Handle terminal events (keys, mouse, paste)
        handle_terminal_events(app, &mut client, terminal, &keymap, slash_registry)?;

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
/// If `session_id` is None and `create_new` is true, creates a new session.
/// If `session_id` is None and `create_new` is false, lists sessions and picks
/// the first one (or errors if none exist).
async fn resolve_session(
    session_id: Option<String>,
    create_new: bool,
    model: Option<String>,
) -> Result<(String, String)> {
    if create_new {
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
async fn send_control(cmd: ControlCommand) -> Result<ControlResponse> {
    let path = clankers_controller::transport::control_socket_path();
    let stream = UnixStream::connect(&path).await.map_err(|e| {
        crate::error::Error::Provider {
            message: format!(
                "Cannot connect to daemon at {}: {e}\nIs the daemon running? Start with: clankers daemon",
                path.display()
            ),
        }
    })?;

    let (mut reader, mut writer) = stream.into_split();

    frame::write_frame(&mut writer, &cmd).await.map_err(|e| {
        crate::error::Error::Provider {
            message: format!("Failed to send command: {e}"),
        }
    })?;

    let resp: ControlResponse = frame::read_frame(&mut reader).await.map_err(|e| {
        crate::error::Error::Provider {
            message: format!("Failed to read response: {e}"),
        }
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
    use crossterm::event::{self as ct_event, Event, KeyCode, KeyEventKind};
    use crossterm::execute;
    use crossterm::style::{self, Stylize};
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
        fn drop(&mut self) {
            crossterm::terminal::disable_raw_mode().ok();
            crossterm::execute!(std::io::stdout(), crossterm::cursor::Show).ok();
        }
    }
    let _guard = RawGuard;

    let mut selected: usize = 0;

    loop {
        // Clear and redraw.
        execute!(
            stdout,
            terminal::Clear(terminal::ClearType::All),
            cursor::MoveTo(0, 0),
        )
        .ok();

        // Header
        execute!(
            stdout,
            style::PrintStyledContent("Select a session to attach:\r\n\r\n".bold()),
        )
        .ok();

        // Column header
        let header = format!(
            "  {:<10} {:<28} {:>5}  {:<20}  {}\r\n",
            "SESSION", "MODEL", "TURNS", "LAST ACTIVE", "CLIENTS"
        );
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
        execute!(
            stdout,
            style::PrintStyledContent("\r\n  ↑/↓ navigate  Enter select  q/Esc cancel\r\n".dim()),
        )
        .ok();

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
enum RecoveryMode {
    /// Auto-daemon: restart daemon, create session with `resume_id`, reconnect.
    AutoDaemon {
        session_id: String,
        model: String,
        cwd: String,
    },
    /// Explicit `clankers attach`: just retry the socket, give up if it fails.
    ExplicitAttach,
}

// ── Session cleanup guard ────────────────────────────────────────────────────

/// RAII guard that kills an auto-daemon session on drop.
///
/// Fires on normal return, early `?` return, panic unwind, and signal-driven
/// exit (Ctrl+C / SIGTERM cause stack unwind through crossterm's raw mode
/// restoration). Uses a synchronous blocking socket write so it works from
/// `Drop` (which can't be async).
struct SessionGuard {
    session_id: String,
    /// Set to `false` to suppress cleanup (e.g. if the session was already
    /// killed or transferred to explicit attach).
    active: bool,
}

impl SessionGuard {
    fn new(session_id: String) -> Self {
        Self {
            session_id,
            active: true,
        }
    }
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        if let Err(e) = send_kill_session_sync(&self.session_id) {
            // Best effort — we're tearing down anyway.
            eprintln!("auto-daemon: failed to kill session on exit: {e}");
        }
    }
}

/// Synchronous (blocking) `KillSession` send over the control socket.
///
/// Used from `SessionGuard::drop` where async isn't available.
/// Connects, writes a single frame, and disconnects. 500ms timeout on
/// the connect to avoid blocking process exit if the daemon is hung.
fn send_kill_session_sync(session_id: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::net::UnixStream as StdUnixStream;

    let path = clankers_controller::transport::control_socket_path();
    let stream = StdUnixStream::connect(&path)?;
    stream.set_write_timeout(Some(Duration::from_millis(500)))?;

    let cmd = ControlCommand::KillSession {
        session_id: session_id.to_string(),
    };
    let payload = serde_json::to_vec(&cmd)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let len = (payload.len() as u32).to_be_bytes();

    let mut buf = Vec::with_capacity(4 + payload.len());
    buf.extend_from_slice(&len);
    buf.extend_from_slice(&payload);

    (&stream).write_all(&buf)?;
    // Don't wait for response — fire and forget during teardown.
    Ok(())
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
async fn try_reconnect(
    socket_path: &str,
    session_id: &str,
) -> Option<ClientAdapter> {
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

        match ClientAdapter::connect(
            stream,
            "clankers-tui",
            None,
            Some(session_id.to_string()),
        )
        .await
        {
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
/// Returns `(ClientAdapter, new_socket_path, resumed)` where `resumed` is true
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
    let (new_session_id, socket_path, resumed) = {
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

    info!("auto-daemon: recovered session {new_session_id} at {socket_path} (resumed={resumed})");

    // 3. Connect to the new session socket
    let stream = UnixStream::connect(&socket_path)
        .await
        .map_err(|e| format!("cannot connect to recovered session socket {socket_path}: {e}"))?;

    let mut adapter = ClientAdapter::connect(
        stream,
        "clankers-tui",
        None,
        Some(new_session_id),
    )
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

    Ok((adapter, socket_path, resumed))
}

/// Drain available DaemonEvents from the client and apply them to App state.
fn drain_daemon_events(app: &mut App, client: &mut ClientAdapter, replaying_history: &mut bool, max_subagent_panes: usize) {
    while let Some(event) = client.try_recv() {
        process_daemon_event(app, client, &event, replaying_history, max_subagent_panes);
    }
}

/// Process a single DaemonEvent — update App state, handle non-TUI events.
fn process_daemon_event(
    app: &mut App,
    client: &ClientAdapter,
    event: &DaemonEvent,
    replaying_history: &mut bool,
    max_subagent_panes: usize,
) {
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
        DaemonEvent::TodoRequest {
            request_id,
            action,
        } => {
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
        DaemonEvent::ToolBlocked {
            tool_name, reason, ..
        } => {
            app.push_system(format!("⛔ Tool blocked: {tool_name} — {reason}"), true);
        }

        // ── Subagent events ─────────────────────────
        DaemonEvent::SubagentStarted { id, name, task, pid } => {
            if let Some(panel) = app
                .panels
                .downcast_mut::<crate::tui::components::subagent_panel::SubagentPanel>(
                    crate::tui::panel::PanelId::Subagents,
                )
            {
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
                app.layout.pane_registry.register(
                    pane_id,
                    crate::tui::panes::PaneKind::Subagent(id.clone()),
                );
                crate::tui::panes::auto_split_for_subagent(
                    &mut app.layout.tiling,
                    &app.layout.pane_registry,
                    pane_id,
                );
            }
        }
        DaemonEvent::SubagentOutput { id, line } => {
            if let Some(panel) = app
                .panels
                .downcast_mut::<crate::tui::components::subagent_panel::SubagentPanel>(
                    crate::tui::panel::PanelId::Subagents,
                )
            {
                panel.append_output(id, line);
            }
            app.layout.subagent_panes.append_output(id, line);
        }
        DaemonEvent::SubagentDone { id } => {
            if let Some(panel) = app
                .panels
                .downcast_mut::<crate::tui::components::subagent_panel::SubagentPanel>(
                    crate::tui::panel::PanelId::Subagents,
                )
            {
                panel.mark_done(id);
            }
            app.layout.subagent_panes.mark_done(id);
        }
        DaemonEvent::SubagentError { id, message } => {
            if let Some(panel) = app
                .panels
                .downcast_mut::<crate::tui::components::subagent_panel::SubagentPanel>(
                    crate::tui::panel::PanelId::Subagents,
                )
            {
                panel.mark_error(id);
                panel.append_output(id, &format!("Error: {message}"));
            }
            app.layout.subagent_panes.mark_error(id);
        }

        // ── History replay ──────────────────────────
        DaemonEvent::HistoryBlock { block } => {
            if *replaying_history {
                match serde_json::from_value::<clankers_message::AgentMessage>(block.clone()) {
                    Ok(msg) => {
                        let events =
                            clankers_controller::convert::agent_message_to_tui_events(&msg);
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
            *replaying_history = false;
        }

        // ── Tool metadata ────────────────────────────
        DaemonEvent::ToolList { tools } => {
            app.tool_info = tools.iter().map(|t| {
                (t.name.clone(), t.description.clone(), String::new())
            }).collect();
        }
        DaemonEvent::DisabledToolsChanged { tools } => {
            app.disabled_tools = tools.iter().cloned().collect();
        }

        // ── State sync events ───────────────────────
        DaemonEvent::ThinkingLevelChanged { from, to } => {
            app.push_system(format!("Thinking: {from} → {to}"), false);
        }
        DaemonEvent::LoopStatus { active, iteration, max_iterations, break_condition } => {
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
        DaemonEvent::SystemPromptResponse { .. } => {
            // We didn't request this — ignore
        }

        // Events already handled by daemon_event_to_tui_event above
        _ => {}
    }
}

// ── Terminal event handling ──────────────────────────────────────────────────

fn handle_terminal_events(
    app: &mut App,
    client: &mut ClientAdapter,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    keymap: &Keymap,
    slash_registry: &slash_commands::SlashRegistry,
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
                handle_key_event(app, client, terminal, key, keymap, slash_registry);
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
fn handle_key_event(
    app: &mut App,
    client: &mut ClientAdapter,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    key: crossterm::event::KeyEvent,
    keymap: &Keymap,
    slash_registry: &slash_commands::SlashRegistry,
) {
    use crate::config::keybindings::Action;
    use crate::config::keybindings::CoreAction;
    use crate::config::keybindings::ExtendedAction;
    use crate::tui::selectors;
    use crossterm::event::KeyCode;
    use crossterm::event::KeyModifiers;

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
                client.send(SessionCommand::ConfirmBash { request_id, approved: true });
                app.push_system("✅ Command approved.".to_string(), false);
            }
            KeyCode::Char('n' | 'N') | KeyCode::Esc => {
                let request_id = confirm.request_id.clone();
                app.overlays.confirm_dialog = None;
                client.send(SessionCommand::ConfirmBash { request_id, approved: false });
                app.push_system("❌ Command denied.".to_string(), true);
            }
            KeyCode::Enter => {
                let request_id = confirm.request_id.clone();
                let approved = confirm.approved;
                app.overlays.confirm_dialog = None;
                client.send(SessionCommand::ConfirmBash { request_id, approved });
                if approved {
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
        if let Some(clankers_tui_types::SelectorAction::SetModel(model)) = action {
            client.send(SessionCommand::SetModel {
                model: model.clone(),
            });
            app.model = model;
        }
        if consumed {
            return;
        }
    }

    // Account selector overlay
    if app.overlays.account_selector.visible {
        let (consumed, action) = crate::tui::selectors::handle_account_selector_key(app, &key);
        if let Some(clankers_tui_types::SelectorAction::SwitchAccount(name)) = action {
            client.send(SessionCommand::SwitchAccount { account: name });
        }
        if consumed { return; }
    }

    // Tool toggle overlay
    if app.overlays.tool_toggle.visible {
        let (consumed, dirty) = crate::tui::selectors::handle_tool_toggle_key(app, &key);
        if dirty {
            let disabled: Vec<String> = app.overlays.tool_toggle.disabled_set()
                .into_iter().collect();
            app.disabled_tools = disabled.iter().cloned().collect();
            client.send(SessionCommand::SetDisabledTools { tools: disabled });
        }
        if consumed { return; }
    }

    // Leader menu
    if app.overlays.leader_menu.visible {
        if let Some(leader_action) = app.overlays.leader_menu.handle_key(&key) {
            handle_leader_action_attach(app, client, leader_action, slash_registry);
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
        && handle_slash_menu_key_attach(app, client, &key, keymap, slash_registry)
    {
        return;
    }

    // Panel focus keys
    if app.has_panel_focus()
        && app.input_mode == InputMode::Normal
        && handle_panel_focused_key_attach(app, key)
    {
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
                    submit_input_attach(app, client, &text, slash_registry);
                }
            }
            // Cancel: tell daemon to abort
            Action::Core(CoreAction::Cancel) => {
                client.abort();
                app.push_system("Abort sent to daemon.".to_string(), false);
            }
            // Client-side TUI actions handled locally
            _ => {
                handle_local_action(app, client, &action, &key);
            }
        }
    } else if app.input_mode == InputMode::Insert {
        crate::modes::event_handlers::handle_insert_char(app, &key);
    }
}

/// Submit input in attach mode — client-side commands handled locally,
/// everything else forwarded to the daemon.
fn submit_input_attach(
    app: &mut App,
    client: &ClientAdapter,
    text: &str,
    slash_registry: &slash_commands::SlashRegistry,
) {
    if let Some((command, args)) = slash_commands::parse_command(text) {
        if is_client_side_command(&command) {
            // Handle locally — these are TUI-only commands
            handle_client_side_slash(app, &command, &args, slash_registry);
        } else {
            // Forward to daemon
            client.send(SessionCommand::SlashCommand {
                command,
                args: args.clone(),
            });
        }
    } else {
        // Regular prompt — expand @file references, then send
        let expanded = crate::util::at_file::expand_at_refs_with_images(text, &app.cwd);
        client.prompt(expanded.text);
    }
}

/// Handle a client-side slash command locally.
fn handle_client_side_slash(
    app: &mut App,
    command: &str,
    args: &str,
    _slash_registry: &slash_commands::SlashRegistry,
) {
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
            app.push_system("Attach mode — limited commands available:".to_string(), false);
            app.push_system("  /quit, /detach — disconnect from session".to_string(), false);
            app.push_system("  /zoom — toggle zoom on focused pane".to_string(), false);
            app.push_system("  /help — this message".to_string(), false);
            app.push_system("  All other commands are forwarded to the daemon.".to_string(), false);
        }
        _ => {
            app.push_system(
                format!("Client command /{command} not implemented in attach mode."),
                true,
            );
        }
    }

    let _ = args; // Some commands will use args later
}

/// Handle a leader menu action in attach mode.
fn handle_leader_action_attach(
    app: &mut App,
    client: &ClientAdapter,
    action: clankers_tui_types::LeaderAction,
    slash_registry: &slash_commands::SlashRegistry,
) {
    use clankers_tui_types::LeaderAction;

    match action {
        LeaderAction::Command(cmd) => {
            if let Some((command, args)) = slash_commands::parse_command(&cmd) {
                if is_client_side_command(&command) {
                    handle_client_side_slash(app, &command, &args, slash_registry);
                } else {
                    client.send(SessionCommand::SlashCommand {
                        command,
                        args: args.clone(),
                    });
                }
            }
        }
        LeaderAction::Action(action) => {
            // Handle keymap actions from leader menu as local actions
            let dummy_key = crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Null,
                crossterm::event::KeyModifiers::empty(),
            );
            handle_local_action(app, client, &action, &dummy_key);
        }
        LeaderAction::Submenu(_) => {
            // Submenus are handled by the leader menu widget itself
        }
    }
}

/// Handle the slash menu key event in attach mode.
fn handle_slash_menu_key_attach(
    app: &mut App,
    client: &ClientAdapter,
    key: &crossterm::event::KeyEvent,
    keymap: &Keymap,
    slash_registry: &slash_commands::SlashRegistry,
) -> bool {
    use crate::config::keybindings::Action;
    use crate::config::keybindings::CoreAction;
    use crossterm::event::KeyCode;

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
                submit_input_attach(app, client, &text, slash_registry);
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
fn handle_local_action(
    app: &mut App,
    client: &ClientAdapter,
    action: &crate::config::keybindings::Action,
    _key: &crossterm::event::KeyEvent,
) {
    use crate::config::keybindings::Action;
    use crate::config::keybindings::CoreAction;
    use crate::config::keybindings::ExtendedAction;
    use clankers_tui_types::AppState;
    use clankers_tui_types::BlockEntry;
    use ratatui::layout::Direction;
    use ratatui_hypertile::HypertileAction;
    use ratatui_hypertile::Towards;

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
                direction: Direction::Vertical, towards: Towards::Start,
            });
        }
        Action::Core(CoreAction::FocusNextBlock) => {
            app.apply_tiling_action(HypertileAction::FocusDirection {
                direction: Direction::Vertical, towards: Towards::End,
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
                    direction: Direction::Horizontal, towards: Towards::Start,
                });
                app.input_mode = InputMode::Normal;
            }
        }
        Action::Extended(ExtendedAction::BranchNext) => {
            if app.conversation.focused_block.is_some() {
                app.branch_next();
            } else {
                app.apply_tiling_action(HypertileAction::FocusDirection {
                    direction: Direction::Horizontal, towards: Towards::End,
                });
                app.input_mode = InputMode::Normal;
            }
        }
        Action::Extended(ExtendedAction::ToggleBranchPanel) => {
            use clankers_tui_types::PanelId;
            if app.layout.focused_panel == Some(PanelId::Branches) {
                app.unfocus_panel();
            } else {
                let active_ids: std::collections::HashSet<usize> = app.conversation.blocks.iter()
                    .filter_map(|e| match e { BlockEntry::Conversation(b) => Some(b.id), _ => None })
                    .collect();
                if let Some(bp) = app.panels.downcast_mut::<crate::tui::components::branch_panel::BranchPanel>(PanelId::Branches) {
                    bp.refresh(&app.conversation.all_blocks.clone(), &active_ids);
                }
                app.focus_panel(PanelId::Branches);
            }
        }
        Action::Extended(ExtendedAction::OpenBranchSwitcher) => {
            let active_ids: std::collections::HashSet<usize> = app.conversation.blocks.iter()
                .filter_map(|e| match e { BlockEntry::Conversation(b) => Some(b.id), _ => None })
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
                direction: Direction::Horizontal, towards: Towards::End,
            });
            app.input_mode = InputMode::Normal;
        }
        Action::Extended(ExtendedAction::PanelPrevTab) => {
            app.apply_tiling_action(HypertileAction::FocusDirection {
                direction: Direction::Horizontal, towards: Towards::Start,
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
                direction: Direction::Horizontal, towards: Towards::Start,
                scope: ratatui_hypertile::MoveScope::Window,
            });
        }
        Action::Extended(ExtendedAction::PaneMoveRight) => {
            app.apply_tiling_action(HypertileAction::MoveFocused {
                direction: Direction::Horizontal, towards: Towards::End,
                scope: ratatui_hypertile::MoveScope::Window,
            });
        }
        Action::Extended(ExtendedAction::PaneMoveUp) => {
            app.apply_tiling_action(HypertileAction::MoveFocused {
                direction: Direction::Vertical, towards: Towards::Start,
                scope: ratatui_hypertile::MoveScope::Window,
            });
        }
        Action::Extended(ExtendedAction::PaneMoveDown) => {
            app.apply_tiling_action(HypertileAction::MoveFocused {
                direction: Direction::Vertical, towards: Towards::End,
                scope: ratatui_hypertile::MoveScope::Window,
            });
        }
        Action::Extended(ExtendedAction::PaneZoom) => app.zoom_toggle(),
        Action::Extended(ExtendedAction::PanelScrollUp) => {
            use clankers_tui_types::PanelId;
            if let Some(sp) = app.panels.downcast_mut::<crate::tui::components::subagent_panel::SubagentPanel>(PanelId::Subagents) {
                sp.scroll.scroll_up(3);
            }
        }
        Action::Extended(ExtendedAction::PanelScrollDown) => {
            use clankers_tui_types::PanelId;
            if let Some(sp) = app.panels.downcast_mut::<crate::tui::components::subagent_panel::SubagentPanel>(PanelId::Subagents) {
                sp.scroll.scroll_down(3);
            }
        }
        Action::Extended(ExtendedAction::PanelClearDone) => {
            use clankers_tui_types::PanelId;
            if let Some(sp) = app.panels.downcast_mut::<crate::tui::components::subagent_panel::SubagentPanel>(PanelId::Subagents) {
                sp.clear_done();
                if !sp.is_visible() { app.unfocus_panel(); }
            }
        }
        Action::Extended(ExtendedAction::PanelKill) => {
            // No panel_tx in attach mode — kill not supported yet
        }
        Action::Extended(ExtendedAction::PanelRemove) => {
            use clankers_tui_types::PanelId;
            if let Some(sp) = app.panels.downcast_mut::<crate::tui::components::subagent_panel::SubagentPanel>(PanelId::Subagents) {
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
                    name: info.name, label: info.label,
                    is_active: info.is_active, is_expired: info.is_expired,
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
                    BlockEntry::Conversation(b) => Some(b.id), _ => None,
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
            client.send(SessionCommand::CycleThinkingLevel);
        }
        Action::Extended(ExtendedAction::ToggleAutoTest) => {
            if app.auto_test_command.is_none() {
                app.push_system("No test command configured.".to_string(), true);
            } else {
                let enabled = !app.auto_test_enabled;
                client.send(SessionCommand::SetAutoTest {
                    enabled,
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
    use clankers_tui_types::PanelAction;
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

fn build_client_slash_registry() -> slash_commands::SlashRegistry {
    // We build a full registry so the completion menu works, but in attach mode
    // only client-side commands are handled locally — the rest are forwarded.
    crate::modes::interactive::build_slash_registry(None)
}

// ── Remote attach (iroh QUIC) ───────────────────────────────────────────────

/// Combine iroh QUIC send + recv into a single `AsyncRead + AsyncWrite` stream.
///
/// This lets us pass QUIC bidirectional streams to `ClientAdapter::connect()`
/// which expects a unified stream type (same as `UnixStream`, `TcpStream`).
struct QuicBiStream {
    send: ::iroh::endpoint::SendStream,
    recv: ::iroh::endpoint::RecvStream,
}

impl tokio::io::AsyncRead for QuicBiStream {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        std::pin::Pin::new(&mut self.recv).poll_read(cx, buf)
    }
}

impl tokio::io::AsyncWrite for QuicBiStream {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<io::Result<usize>> {
        std::pin::Pin::new(&mut self.send)
            .poll_write(cx, buf)
            .map_err(|e| io::Error::other(e.to_string()))
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        std::pin::Pin::new(&mut self.send)
            .poll_flush(cx)
            .map_err(|e| io::Error::other(e.to_string()))
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        std::pin::Pin::new(&mut self.send)
            .poll_shutdown(cx)
            .map_err(|e| io::Error::other(e.to_string()))
    }
}

/// Launch the TUI in remote attach mode over iroh QUIC.
///
/// Connects to a remote daemon's `clankers/daemon/1` ALPN, performs the
/// attach handshake, then reuses the same `ClientAdapter` + event loop as
/// local Unix socket attach.
pub async fn run_remote_attach(
    remote_id: &str,
    session_id: Option<String>,
    create_new: bool,
    model: Option<String>,
    settings: &Settings,
    paths: &crate::config::ClankersPaths,
) -> Result<()> {
    use crate::modes::rpc::iroh;

    // Load or generate identity
    let identity_path = iroh::identity_path(paths);
    let identity = iroh::Identity::load_or_generate(&identity_path);

    // Resolve remote_id: try as peer name from peers.json first, then as raw node ID
    let resolved_id = {
        let registry_path = crate::modes::rpc::peers::registry_path(paths);
        let registry = crate::modes::rpc::peers::PeerRegistry::load(&registry_path);
        if let Some(peer) = registry.peers.values().find(|p| p.name == remote_id) {
            peer.node_id.clone()
        } else {
            remote_id.to_string()
        }
    };

    let remote_pk: ::iroh::PublicKey = resolved_id.parse().map_err(|e| {
        crate::error::Error::Provider {
            message: format!("Invalid remote node ID '{resolved_id}' (from '{remote_id}'): {e}"),
        }
    })?;

    // Start endpoint
    let endpoint = iroh::start_endpoint(&identity).await?;
    info!("local node: {}", endpoint.id().fmt_short());
    println!("Connecting to {}...", remote_pk.fmt_short());

    // Connect with daemon ALPN
    let conn = endpoint
        .connect(remote_pk, clankers_protocol::types::ALPN_DAEMON)
        .await
        .map_err(|e| crate::error::Error::Provider {
            message: format!("Failed to connect to remote daemon: {e}"),
        })?;
    info!("connected to remote daemon {}", remote_pk.fmt_short());

    // If --new, create the session first via a control stream
    let target_session_id = if create_new {
        let sid = create_remote_session(&conn, model.clone()).await?;
        println!("Created remote session: {sid}");
        Some(sid)
    } else {
        session_id
    };

    // Open an attach stream
    let (mut send, mut recv) = conn.open_bi().await.map_err(|e| {
        crate::error::Error::Provider {
            message: format!("Failed to open QUIC stream: {e}"),
        }
    })?;

    // Send DaemonRequest::Attach as the first frame, then the normal
    // session protocol continues over the same stream.
    let handshake = clankers_protocol::Handshake {
        protocol_version: clankers_protocol::types::PROTOCOL_VERSION,
        client_name: format!("clankers-tui/{}", env!("CARGO_PKG_VERSION")),
        token: None,
        session_id: target_session_id.clone(),
    };
    let request = clankers_protocol::DaemonRequest::Attach { handshake: handshake.clone() };
    quic_write_frame(&mut send, &request).await?;

    // Read AttachResponse
    let response: clankers_protocol::AttachResponse = quic_read_frame(&mut recv).await?;
    let resolved_session_id = match response {
        clankers_protocol::AttachResponse::Ok { session_id } => session_id,
        clankers_protocol::AttachResponse::Error { message } => {
            return Err(crate::error::Error::Provider {
                message: format!("Remote attach failed: {message}"),
            });
        }
    };

    println!("Attached to remote session: {resolved_session_id}");

    // Now the QUIC stream carries the standard session protocol:
    // DaemonEvent frames (recv) and SessionCommand frames (send).
    // Wrap send+recv into a single stream and hand to ClientAdapter.
    //
    // Note: ClientAdapter performs its own handshake (Handshake frame),
    // but the daemon-side QUIC handler already consumed our DaemonRequest
    // and sent back SessionInfo. We need to skip the ClientAdapter handshake.
    //
    // Instead, we read the SessionInfo ourselves and feed events manually.
    let (model_name, _session_hash) = match quic_read_frame::<DaemonEvent>(&mut recv).await {
        Ok(DaemonEvent::SessionInfo {
            model,
            system_prompt_hash,
            ..
        }) => (model, system_prompt_hash),
        Ok(other) => {
            warn!("expected SessionInfo, got: {other:?}");
            (String::new(), String::new())
        }
        Err(e) => {
            return Err(crate::error::Error::Provider {
                message: format!("Session disconnected before sending SessionInfo: {e}"),
            });
        }
    };

    // Now wrap the remaining stream into a QuicBiStream and create a
    // ClientAdapter. The handshake is already done (we consumed Attach +
    // SessionInfo above), but ClientAdapter::connect() will try to send
    // another handshake and read SessionInfo. So instead, we write a
    // synthetic handshake frame that the daemon will ignore (the session
    // stream is already established), and the daemon's next frames will
    // be treated as events.
    //
    // Actually, the cleaner approach: build the ClientAdapter directly
    // from channels, bypassing the handshake. But ClientAdapter's ctor
    // requires a stream. Let's create it from the QUIC bi-stream — the
    // daemon side has already sent SessionInfo, and any subsequent frames
    // are events. ClientAdapter::connect would send a Handshake and expect
    // a SessionInfo — that won't work here since those already happened.
    //
    // Solution: build ClientAdapter manually from channels with a thin
    // adapter that reads/writes frames on the QUIC stream.
    let bi = QuicBiStream { send, recv };
    let client = build_quic_client_adapter(bi);

    // Replay history
    client.replay_history();

    // Set up TUI
    let mut term = super::common::init_terminal()?;

    let display_model = if model_name.is_empty() {
        "remote".to_string()
    } else {
        model_name
    };

    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let theme = Theme::dark();
    let keymap = settings.keymap.clone().into_keymap();

    let mut app = App::new(display_model.clone(), cwd, theme);
    app.session_id = resolved_session_id.clone();
    app.highlighter = Box::new(crate::util::syntax::SyntectHighlighter);

    let slash_registry = build_client_slash_registry();
    app.set_completion_source(Box::new(clankers_tui_types::CompletionSnapshot::from_source(
        &slash_registry,
    )));
    crate::modes::interactive::rebuild_leader_menu(&mut app, None, settings);
    app.connection_mode = clankers_tui_types::ConnectionMode::Remote {
        node_id_short: remote_pk.fmt_short().to_string(),
    };

    app.push_system(
        format!(
            "attached to remote session {} at {} (model: {})",
            resolved_session_id,
            remote_pk.fmt_short(),
            display_model,
        ),
        false,
    );
    app.push_system("Type /detach or Ctrl+Q to disconnect.".to_string(), false);

    let max_subagent_panes = settings.max_subagent_panes;

    // Run the event loop with QUIC-aware reconnection.
    // We hold the connection so we can open new streams on disconnect.
    let result = run_remote_attach_loop(
        &mut term,
        &mut app,
        client,
        conn,
        &endpoint,
        remote_pk,
        keymap,
        &slash_registry,
        max_subagent_panes,
        &resolved_session_id,
    )
    .await;

    super::common::restore_terminal(&mut term);
    endpoint.close().await;
    result
}

/// Event loop for remote QUIC attach with reconnection support.
///
/// Holds the QUIC connection so that on stream failure, we can open a
/// new bi-stream on the same multiplexed connection rather than needing
/// a full re-connect.
async fn run_remote_attach_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    mut client: ClientAdapter,
    conn: ::iroh::endpoint::Connection,
    endpoint: &::iroh::Endpoint,
    remote_pk: ::iroh::PublicKey,
    keymap: Keymap,
    slash_registry: &slash_commands::SlashRegistry,
    max_subagent_panes: usize,
    session_id: &str,
) -> Result<()> {
    let mut replaying_history = true;

    loop {
        terminal
            .draw(|frame| render::render(frame, app))
            .map_err(|e| crate::error::Error::Tui {
                message: format!("Render failed: {e}"),
            })?;

        if app.should_quit {
            client.disconnect();
            break;
        }

        // Drain daemon events
        drain_daemon_events(app, &mut client, &mut replaying_history, max_subagent_panes);

        // Detect disconnect and attempt reconnection over the same QUIC connection
        if client.is_disconnected()
            && app.connection_mode != clankers_tui_types::ConnectionMode::Reconnecting
        {
            app.connection_mode = clankers_tui_types::ConnectionMode::Reconnecting;
            app.push_system(
                "QUIC stream lost. Reconnecting on same connection...".to_string(),
                true,
            );

            match try_quic_reconnect(&conn, endpoint, remote_pk, session_id).await {
                Some((new_client, _new_conn)) => {
                    // Note: if _new_conn is Some, the old conn is dead. We can't
                    // replace `conn` because it's borrowed immutably. But the new
                    // client has its own stream — future reconnects would need the
                    // new connection. For now, the client works and a second
                    // disconnect would fail reconnect on the old conn then succeed
                    // by re-establishing again.
                    client = new_client;
                    client.replay_history();
                    replaying_history = true;
                    app.connection_mode = clankers_tui_types::ConnectionMode::Attached;
                    app.push_system("Reconnected to remote session.".to_string(), false);
                }
                None => {
                    app.push_system(
                        "Failed to reconnect after 5 attempts. Use /quit to exit.".to_string(),
                        true,
                    );
                }
            }
        }

        // Handle terminal events — same as regular attach
        handle_terminal_events(app, &mut client, terminal, &keymap, slash_registry)?;

        if app.open_editor_requested {
            app.open_editor_requested = false;
            crate::tui::clipboard::open_external_editor(terminal, app);
        }
    }

    Ok(())
}

/// Maximum reconnect attempts before giving up.
const QUIC_RECONNECT_MAX_ATTEMPTS: usize = 5;

/// Reconnect to a session by opening a new bi-stream on the existing
/// QUIC connection. If the connection itself is dead (daemon restarted),
/// attempts to re-establish via the endpoint.
async fn try_quic_reconnect(
    conn: &::iroh::endpoint::Connection,
    endpoint: &::iroh::Endpoint,
    remote_pk: ::iroh::PublicKey,
    session_id: &str,
) -> Option<(ClientAdapter, Option<::iroh::endpoint::Connection>)> {
    // Delays: 1s, 2s, 4s, 8s, 16s
    let delays_ms = [1000, 2000, 4000, 8000, 16000];

    for attempt in 0..QUIC_RECONNECT_MAX_ATTEMPTS {
        if attempt > 0 {
            let delay = delays_ms.get(attempt).copied().unwrap_or(16000);
            info!("QUIC reconnect attempt {}/{QUIC_RECONNECT_MAX_ATTEMPTS} (delay {delay}ms)", attempt + 1);
            tokio::time::sleep(Duration::from_millis(delay as u64)).await;
        }

        // Try the existing connection first
        if let Some(client) = try_quic_attach_stream(conn, session_id).await {
            info!("QUIC reconnect succeeded on existing connection (attempt {})", attempt + 1);
            return Some((client, None));
        }

        // Existing connection dead — try re-establishing
        match endpoint.connect(remote_pk, clankers_protocol::types::ALPN_DAEMON).await {
            Ok(new_conn) => {
                if let Some(client) = try_quic_attach_stream(&new_conn, session_id).await {
                    info!("QUIC reconnect succeeded on new connection (attempt {})", attempt + 1);
                    return Some((client, Some(new_conn)));
                }
            }
            Err(e) => {
                warn!("QUIC reconnect attempt {}: connect failed: {e}", attempt + 1);
            }
        }
    }

    None
}

/// Open a new bi-stream on a connection and perform the attach handshake.
async fn try_quic_attach_stream(
    conn: &::iroh::endpoint::Connection,
    session_id: &str,
) -> Option<ClientAdapter> {
    let (mut send, mut recv) = conn.open_bi().await.ok()?;

    let request = clankers_protocol::DaemonRequest::Attach {
        handshake: clankers_protocol::Handshake {
            protocol_version: clankers_protocol::types::PROTOCOL_VERSION,
            client_name: format!("clankers-tui/{}", env!("CARGO_PKG_VERSION")),
            token: None,
            session_id: Some(session_id.to_string()),
        },
    };
    quic_write_frame(&mut send, &request).await.ok()?;

    let response: clankers_protocol::AttachResponse = quic_read_frame(&mut recv).await.ok()?;
    match response {
        clankers_protocol::AttachResponse::Ok { .. } => {}
        clankers_protocol::AttachResponse::Error { message } => {
            warn!("QUIC attach rejected: {message}");
            return None;
        }
    }

    // Read SessionInfo
    match quic_read_frame::<DaemonEvent>(&mut recv).await {
        Ok(DaemonEvent::SessionInfo { .. }) => {}
        _ => return None,
    }

    let bi = QuicBiStream { send, recv };
    Some(build_quic_client_adapter(bi))
}

/// Build a ClientAdapter from a QUIC stream, skipping the handshake.
///
/// The DaemonRequest::Attach + AttachResponse + SessionInfo exchange has
/// already completed. The stream now carries raw DaemonEvent/SessionCommand
/// frames, which is exactly what ClientAdapter's background tasks expect.
fn build_quic_client_adapter(stream: QuicBiStream) -> ClientAdapter {
    use tokio::sync::mpsc;

    let (reader, writer) = tokio::io::split(stream);

    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<SessionCommand>();
    let (event_tx, event_rx) = mpsc::unbounded_channel::<DaemonEvent>();

    // Spawn writer: SessionCommand → QUIC
    tokio::spawn(async move {
        let mut writer = writer;
        while let Some(cmd) = cmd_rx.recv().await {
            if frame::write_frame(&mut writer, &cmd).await.is_err() {
                break;
            }
        }
        let _ = tokio::io::AsyncWriteExt::shutdown(&mut writer).await;
    });

    // Spawn reader: QUIC → DaemonEvent
    tokio::spawn(async move {
        let mut reader = reader;
        while let Ok(event) = frame::read_frame::<_, DaemonEvent>(&mut reader).await {
            if event_tx.send(event).is_err() {
                break;
            }
        }
    });

    // SAFETY: ClientAdapter fields are private, so we can't construct one
    // directly. Use the `from_channels` constructor (which we need to add
    // to clankers-controller).
    ClientAdapter::from_channels(cmd_tx, event_rx)
}

/// Create a new session on the remote daemon via a control stream.
async fn create_remote_session(
    conn: &::iroh::endpoint::Connection,
    model: Option<String>,
) -> Result<String> {
    let (mut send, mut recv) = conn.open_bi().await.map_err(|e| {
        crate::error::Error::Provider {
            message: format!("Failed to open control stream: {e}"),
        }
    })?;

    let request = clankers_protocol::DaemonRequest::Control {
        command: clankers_protocol::ControlCommand::CreateSession {
            model,
            system_prompt: None,
            token: None,
            resume_id: None,
            continue_last: false,
            cwd: None,
        },
    };
    quic_write_frame(&mut send, &request).await?;
    send.finish().ok();

    let response: clankers_protocol::ControlResponse = quic_read_frame(&mut recv).await?;
    match response {
        clankers_protocol::ControlResponse::Created { session_id, .. } => Ok(session_id),
        clankers_protocol::ControlResponse::Error { message } => {
            Err(crate::error::Error::Provider {
                message: format!("Failed to create remote session: {message}"),
            })
        }
        other => Err(crate::error::Error::Provider {
            message: format!("Unexpected response: {other:?}"),
        }),
    }
}

// ── QUIC frame helpers ──────────────────────────────────────────────────────

async fn quic_write_frame<T: serde::Serialize>(
    send: &mut ::iroh::endpoint::SendStream,
    value: &T,
) -> Result<()> {
    let data = serde_json::to_vec(value).map_err(|e| crate::error::Error::Provider {
        message: format!("Serialize error: {e}"),
    })?;
    let len = (data.len() as u32).to_be_bytes();
    send.write_all(&len).await.map_err(|e| crate::error::Error::Provider {
        message: format!("QUIC write error: {e}"),
    })?;
    send.write_all(&data).await.map_err(|e| crate::error::Error::Provider {
        message: format!("QUIC write error: {e}"),
    })?;
    Ok(())
}

async fn quic_read_frame<T: serde::de::DeserializeOwned>(
    recv: &mut ::iroh::endpoint::RecvStream,
) -> Result<T> {
    let mut len_buf = [0u8; 4];
    recv.read_exact(&mut len_buf).await.map_err(|e| crate::error::Error::Provider {
        message: format!("QUIC read error: {e}"),
    })?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > 10_000_000 {
        return Err(crate::error::Error::Provider {
            message: format!("Frame too large: {len}"),
        });
    }
    let mut data = vec![0u8; len];
    recv.read_exact(&mut data).await.map_err(|e| crate::error::Error::Provider {
        message: format!("QUIC read error: {e}"),
    })?;
    serde_json::from_slice(&data).map_err(|e| crate::error::Error::Provider {
        message: format!("Deserialize error: {e}"),
    })
}

#[cfg(test)]
mod tests {
    use clankers_controller::client::is_client_side_command;

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
    fn session_guard_sends_kill_on_drop() {
        // Set up a mock control socket that accepts one connection.
        // control_socket_path() = $XDG_RUNTIME_DIR/clankers/control.sock
        let dir = tempfile::tempdir().unwrap();
        let clankers_dir = dir.path().join("clankers");
        std::fs::create_dir_all(&clankers_dir).unwrap();
        let sock_path = clankers_dir.join("control.sock");

        let listener = std::os::unix::net::UnixListener::bind(&sock_path).unwrap();

        // Point the guard at our mock socket.
        // SAFETY: nextest runs each test in its own process.
        unsafe {
            std::env::set_var("XDG_RUNTIME_DIR", dir.path());
        }

        let guard = super::SessionGuard::new("test-session-42".to_string());

        // Drop the guard — should send KillSession synchronously.
        drop(guard);

        // Accept the connection and read the frame the guard sent.
        // Use a short timeout rather than non-blocking to avoid races.
        listener
            .set_nonblocking(false)
            .unwrap();
        let (mut conn, _) = listener.accept().expect("guard should have connected");

        use std::io::Read;
        let mut len_buf = [0u8; 4];
        conn.read_exact(&mut len_buf).unwrap();
        let len = u32::from_be_bytes(len_buf) as usize;

        let mut payload = vec![0u8; len];
        conn.read_exact(&mut payload).unwrap();

        let cmd: clankers_protocol::control::ControlCommand =
            serde_json::from_slice(&payload).unwrap();
        match cmd {
            clankers_protocol::control::ControlCommand::KillSession { session_id } => {
                assert_eq!(session_id, "test-session-42");
            }
            other => panic!("expected KillSession, got: {other:?}"),
        }
    }

    #[test]
    fn session_guard_inactive_skips_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        // No socket — if the guard tries to connect, it will fail.
        // SAFETY: nextest runs each test in its own process.
        unsafe {
            std::env::set_var("XDG_RUNTIME_DIR", dir.path());
        }

        let mut guard = super::SessionGuard::new("test-session-skip".to_string());
        guard.active = false;

        // Should not attempt to connect (no socket exists, so connect would fail
        // and print to stderr if it tried).
        drop(guard);
    }
}
