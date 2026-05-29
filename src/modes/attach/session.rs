//! Attach-session resolution, control-socket, socket-connect, and recovery helpers.

use std::io;
use std::path::Path;
use std::time::Duration;

use clankers_controller::client::ClientAdapter;
use clankers_protocol::DaemonEvent;
use clankers_protocol::control::ControlCommand;
use clankers_protocol::control::ControlResponse;
use clankers_protocol::frame;
use tokio::net::UnixStream;
use tracing::debug;
use tracing::info;
use tracing::warn;

use crate::error::Result;

// ── Session socket connection ───────────────────────────────────────────────

const SESSION_SOCKET_CONNECT_ATTEMPTS: usize = 20;
const SESSION_SOCKET_CONNECT_RETRY_DELAY_MS: u64 = 25;

pub(crate) fn should_retry_session_socket_connect(error: &io::Error) -> bool {
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

// ── Session resolution ──────────────────────────────────────────────────────

/// Resolve a session ID + socket path via the control socket.
///
/// If `session_id` is None and `should_create_new` is true, creates a new session.
/// If `session_id` is None and `should_create_new` is false, lists sessions and picks
/// the first one (or errors if none exist).
pub(crate) async fn resolve_session(
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
            thinking_level: None,
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
pub(crate) async fn try_reconnect(socket_path: &str, session_id: &str) -> Option<ClientAdapter> {
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
pub(crate) async fn try_recover_daemon(
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
            thinking_level: None,
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
                    thinking_level: None,
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
