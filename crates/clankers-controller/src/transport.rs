//! Unix domain socket transport for daemon-client communication.
//!
//! - Control socket: session listing, creation, attach
//! - Session sockets: per-session event streaming + command input

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use clankers_protocol::SessionCommand;
use clankers_protocol::control::ControlCommand;
use clankers_protocol::control::ControlResponse;
use clankers_protocol::control::DaemonStatus;
use clankers_protocol::control::SessionSummary;
use clankers_protocol::event::DaemonEvent;
use clankers_protocol::frame::FrameError;
use clankers_protocol::frame::{self};
use clankers_protocol::types::Handshake;
use clankers_protocol::types::PROTOCOL_VERSION;
use tokio::net::UnixListener;
use tokio::net::UnixStream;
use tokio::sync::Mutex;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

// Socket directory layout:
//   $XDG_RUNTIME_DIR/clankers/
//   ├── control.sock
//   ├── session-<id>.sock
//   └── daemon.pid

/// Resolve the socket directory path.
///
/// Falls back to `/tmp/clankers-<uid>` when `XDG_RUNTIME_DIR` is unset.
pub fn socket_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(dir).join("clankers")
    } else {
        PathBuf::from(format!("/tmp/clankers-{}", unsafe { libc::getuid() }))
    }
}

/// Path to the control socket.
pub fn control_socket_path() -> PathBuf {
    socket_dir().join("control.sock")
}

/// Path to a session socket.
pub fn session_socket_path(session_id: &str) -> PathBuf {
    socket_dir().join(format!("session-{session_id}.sock"))
}

/// Path to the PID file.
pub fn pid_file_path() -> PathBuf {
    socket_dir().join("daemon.pid")
}

/// Path to the daemon log file.
pub fn daemon_log_path() -> PathBuf {
    socket_dir().join("daemon.log")
}

/// Read the PID from the PID file, if it exists and the process is alive.
/// Returns `None` if no daemon is running.
pub fn running_daemon_pid() -> Option<u32> {
    let pid_path = pid_file_path();
    let pid_str = std::fs::read_to_string(pid_path).ok()?;
    let pid: u32 = pid_str.trim().parse().ok()?;
    if is_process_alive(pid) { Some(pid) } else { None }
}

/// Create the socket directory and write the PID file.
/// Returns an error if another daemon is already running.
pub fn init_socket_dir() -> std::io::Result<()> {
    let dir = socket_dir();
    std::fs::create_dir_all(&dir)?;

    // Check for stale PID file
    let pid_path = pid_file_path();
    if pid_path.exists() {
        let pid_str = std::fs::read_to_string(&pid_path).unwrap_or_default();
        if let Ok(pid) = pid_str.trim().parse::<u32>()
            && is_process_alive(pid)
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AddrInUse,
                format!("daemon already running (PID {pid})"),
            ));
        }
        // Stale PID file — clean up
        cleanup_stale_sockets(&dir);
    }

    // Write our PID
    std::fs::write(&pid_path, format!("{}", std::process::id()))?;
    Ok(())
}

/// Clean up socket files and PID file.
pub fn cleanup_socket_dir() {
    let dir = socket_dir();
    let _ = std::fs::remove_file(pid_file_path());
    let _ = std::fs::remove_file(control_socket_path());
    // Remove session sockets
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "sock") {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

/// Remove stale socket files from a crashed daemon.
fn cleanup_stale_sockets(dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "sock") {
                let _ = std::fs::remove_file(&path);
                info!("cleaned up stale socket: {}", path.display());
            }
        }
    }
    let _ = std::fs::remove_file(dir.join("daemon.pid"));
}

/// Check if a process with the given PID is still running.
fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // kill(pid, 0) checks existence without sending a signal
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

/// Shared state for the daemon's active sessions.
pub struct DaemonState {
    /// Active sessions: session_id → session handle
    pub sessions: HashMap<String, SessionHandle>,
    /// Daemon start time
    pub started_at: Instant,
}

/// Handle to a running session (for control socket queries).
pub struct SessionHandle {
    /// Session ID
    pub session_id: String,
    /// Current model
    pub model: String,
    /// Number of turns
    pub turn_count: usize,
    /// Last activity timestamp (ISO 8601)
    pub last_active: String,
    /// Number of connected clients
    pub client_count: usize,
    /// Command sender for the session controller
    pub cmd_tx: mpsc::UnboundedSender<SessionCommand>,
    /// Event broadcast for clients
    pub event_tx: broadcast::Sender<DaemonEvent>,
    /// Socket path
    pub socket_path: PathBuf,
}

impl DaemonState {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            started_at: Instant::now(),
        }
    }

    pub fn session_summaries(&self) -> Vec<SessionSummary> {
        self.sessions
            .values()
            .map(|h| SessionSummary {
                session_id: h.session_id.clone(),
                model: h.model.clone(),
                turn_count: h.turn_count,
                last_active: h.last_active.clone(),
                client_count: h.client_count,
                socket_path: h.socket_path.to_string_lossy().into_owned(),
            })
            .collect()
    }

    pub fn status(&self) -> DaemonStatus {
        DaemonStatus {
            uptime_secs: self.started_at.elapsed().as_secs_f64(),
            session_count: self.sessions.len(),
            total_clients: self.sessions.values().map(|h| h.client_count).sum(),
            pid: std::process::id(),
        }
    }
}

impl Default for DaemonState {
    fn default() -> Self {
        Self::new()
    }
}

/// Run the control socket listener.
///
/// Accepts connections, reads `ControlCommand`, dispatches to the daemon state,
/// and writes `ControlResponse`.
pub async fn run_control_socket(state: Arc<Mutex<DaemonState>>, shutdown: tokio::sync::watch::Receiver<bool>) {
    let path = control_socket_path();
    let _ = std::fs::remove_file(&path);

    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            error!("failed to bind control socket: {e}");
            return;
        }
    };
    info!("control socket listening at {}", path.display());

    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, _)) => {
                        let state = Arc::clone(&state);
                        tokio::spawn(async move {
                            if let Err(e) = handle_control_connection(stream, state).await {
                                debug!("control connection ended: {e}");
                            }
                        });
                    }
                    Err(e) => {
                        warn!("control socket accept error: {e}");
                    }
                }
            }
            _ = shutdown_signal(&shutdown) => {
                info!("control socket shutting down");
                break;
            }
        }
    }
}

/// Handle a single control socket connection.
async fn handle_control_connection(mut stream: UnixStream, state: Arc<Mutex<DaemonState>>) -> Result<(), FrameError> {
    let (mut reader, mut writer) = stream.split();

    let cmd: ControlCommand = frame::read_frame(&mut reader).await?;
    debug!("control command: {cmd:?}");

    let response = {
        let state = state.lock().await;
        match cmd {
            ControlCommand::ListSessions => ControlResponse::Sessions(state.session_summaries()),
            ControlCommand::Status => ControlResponse::Status(state.status()),
            ControlCommand::ProcessTree => {
                // Process tree would come from the actor registry
                ControlResponse::Tree(vec![])
            }
            ControlCommand::KillSession { session_id } => {
                if let Some(handle) = state.sessions.get(&session_id) {
                    let _ = handle.cmd_tx.send(SessionCommand::Disconnect);
                    ControlResponse::Killed
                } else {
                    ControlResponse::Error {
                        message: format!("session '{session_id}' not found"),
                    }
                }
            }
            ControlCommand::AttachSession { session_id } => {
                if let Some(handle) = state.sessions.get(&session_id) {
                    ControlResponse::Attached {
                        socket_path: handle.socket_path.to_string_lossy().into_owned(),
                    }
                } else {
                    ControlResponse::Error {
                        message: format!("session '{session_id}' not found"),
                    }
                }
            }
            ControlCommand::CreateSession { .. } => {
                // Session creation requires mutable state and agent setup.
                // The caller handles this after receiving the response.
                ControlResponse::Error {
                    message: "CreateSession must be handled by the daemon main loop".to_string(),
                }
            }
            ControlCommand::Shutdown => ControlResponse::ShuttingDown,
        }
    };

    frame::write_frame(&mut writer, &response).await?;
    Ok(())
}

/// Run a session socket listener.
///
/// Each connected client receives all DaemonEvents via broadcast and can
/// send SessionCommands.
pub async fn run_session_socket(
    session_id: String,
    cmd_tx: mpsc::UnboundedSender<SessionCommand>,
    event_tx: broadcast::Sender<DaemonEvent>,
    shutdown: tokio::sync::watch::Receiver<bool>,
) {
    let path = session_socket_path(&session_id);
    let _ = std::fs::remove_file(&path);

    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            error!("failed to bind session socket for {session_id}: {e}");
            return;
        }
    };
    info!("session socket listening at {}", path.display());

    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, _)) => {
                        let cmd_tx = cmd_tx.clone();
                        let event_rx = event_tx.subscribe();
                        let sid = session_id.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_session_client(stream, sid, cmd_tx, event_rx).await {
                                debug!("session client disconnected: {e}");
                            }
                        });
                    }
                    Err(e) => {
                        warn!("session socket accept error: {e}");
                    }
                }
            }
            _ = shutdown_signal(&shutdown) => {
                info!("session socket {session_id} shutting down");
                break;
            }
        }
    }

    let _ = std::fs::remove_file(&path);
}

/// Handle a single client connected to a session socket.
async fn handle_session_client(
    stream: UnixStream,
    session_id: String,
    cmd_tx: mpsc::UnboundedSender<SessionCommand>,
    mut event_rx: broadcast::Receiver<DaemonEvent>,
) -> Result<(), FrameError> {
    let (reader, writer) = stream.into_split();
    let reader = Arc::new(Mutex::new(reader));
    let writer = Arc::new(Mutex::new(writer));

    // 1. Read handshake
    {
        let mut r = reader.lock().await;
        let handshake: Handshake = frame::read_frame(&mut *r).await?;
        if handshake.protocol_version != PROTOCOL_VERSION {
            let mut w = writer.lock().await;
            frame::write_frame(&mut *w, &DaemonEvent::SystemMessage {
                text: format!(
                    "unsupported protocol version {} (expected {})",
                    handshake.protocol_version, PROTOCOL_VERSION
                ),
                is_error: true,
            })
            .await?;
            return Ok(());
        }
        info!("session {session_id}: client connected: {}", handshake.client_name);
    }

    // 2. Send SessionInfo
    {
        let mut w = writer.lock().await;
        frame::write_frame(&mut *w, &DaemonEvent::SessionInfo {
            session_id: session_id.clone(),
            model: String::new(),
            system_prompt_hash: String::new(),
        })
        .await?;
    }

    // 3. Bidirectional event loop: read commands, write events
    let writer_clone = Arc::clone(&writer);
    let reader_clone = Arc::clone(&reader);

    // Spawn event writer
    let write_task = tokio::spawn(async move {
        loop {
            match event_rx.recv().await {
                Ok(event) => {
                    let mut w = writer_clone.lock().await;
                    if frame::write_frame(&mut *w, &event).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!("session client lagged, missed {n} events");
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    // Read commands in the foreground
    loop {
        let mut r = reader_clone.lock().await;
        match frame::read_frame::<_, SessionCommand>(&mut *r).await {
            Ok(cmd) => {
                if matches!(cmd, SessionCommand::Disconnect) {
                    break;
                }
                if cmd_tx.send(cmd).is_err() {
                    break;
                }
            }
            Err(FrameError::Eof) => break,
            Err(e) => {
                debug!("session client read error: {e}");
                break;
            }
        }
    }

    write_task.abort();
    info!("session {session_id}: client disconnected");
    Ok(())
}

/// Wait for the shutdown signal.
async fn shutdown_signal(shutdown: &tokio::sync::watch::Receiver<bool>) {
    let mut rx = shutdown.clone();
    while !*rx.borrow_and_update() {
        if rx.changed().await.is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_socket_paths() {
        let dir = socket_dir();
        assert!(dir.to_string_lossy().contains("clankers"));

        let ctrl = control_socket_path();
        assert!(ctrl.ends_with("control.sock"));

        let sess = session_socket_path("abc123");
        assert!(sess.ends_with("session-abc123.sock"));

        let pid = pid_file_path();
        assert!(pid.ends_with("daemon.pid"));
    }

    #[test]
    fn test_daemon_state() {
        let state = DaemonState::new();
        assert_eq!(state.sessions.len(), 0);
        assert_eq!(state.session_summaries().len(), 0);

        let status = state.status();
        assert_eq!(status.session_count, 0);
        assert_eq!(status.pid, std::process::id());
    }
}
