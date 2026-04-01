//! Auto-daemon mode — default interactive mode through a background daemon.
//!
//! Extracted from `attach.rs`. Contains `AutoDaemonOptions`,
//! `run_auto_daemon_attach`, `SessionGuard`, and related helpers.

use std::time::Duration;

use clankers_controller::client::ClientAdapter;
use clankers_protocol::DaemonEvent;
use clankers_protocol::SessionCommand;
use clankers_protocol::control::ControlCommand;
use clankers_protocol::control::ControlResponse;
use tokio::net::UnixStream;
use tracing::info;
use tracing::warn;

use crate::config::settings::Settings;
use crate::error::Result;
use crate::tui::app::App;
use crate::tui::theme::Theme;

use super::attach::{
    RecoveryMode, build_client_slash_registry, run_attach_with_reconnect, send_control,
};

// ── Options ─────────────────────────────────────────────────────────────────

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

// ── Entry point ─────────────────────────────────────────────────────────────

/// Default interactive mode through a background daemon.
///
/// This is the Phase 3 "flip the switch" entry point: `clankers` (no
/// subcommand) auto-starts a daemon, creates a session with the caller's
/// CLI options, and attaches the TUI to it.
#[cfg_attr(dylint_lib = "tigerstyle", allow(function_length, reason = "sequential setup/dispatch logic — splitting would fragment readability"))]
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

    let was_resumed = opts.resume_id.is_some() || opts.continue_last;
    if was_resumed {
        app.push_system(
            format!(
                "clankers — {} — was_resumed session {} — keymap: {} — press i to start typing",
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

// ── Session cleanup guard ────────────────────────────────────────────────────

/// RAII guard that kills an auto-daemon session on drop.
///
/// Fires on normal return, early `?` return, panic unwind, and signal-driven
/// exit (Ctrl+C / SIGTERM cause stack unwind through crossterm's raw mode
/// restoration). Uses a synchronous blocking socket write so it works from
/// `Drop` (which can't be async).
pub(crate) struct SessionGuard {
    pub(crate) session_id: String,
    /// Set to `false` to suppress cleanup (e.g. if the session was already
    /// killed or transferred to explicit attach).
    pub(crate) active: bool,
}

impl SessionGuard {
    pub(crate) fn new(session_id: String) -> Self {
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

#[cfg(test)]
mod tests {
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
        let len = usize::try_from(u32::from_be_bytes(len_buf)).unwrap_or(0);

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
