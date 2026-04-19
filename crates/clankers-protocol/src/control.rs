//! Control socket protocol — session listing, creation, attach.

use serde::Deserialize;
use serde::Serialize;

use crate::types::ProcessInfo;

fn default_none_string() -> Option<String> {
    None
}

fn default_false() -> bool {
    false
}

/// Commands sent to the control socket.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ControlCommand {
    /// List active sessions.
    ListSessions,
    /// Create a new session (returns the session socket path).
    CreateSession {
        model: Option<String>,
        system_prompt: Option<String>,
        token: Option<String>,
        /// Resume a specific session by ID.
        #[serde(default = "default_none_string")]
        resume_id: Option<String>,
        /// Continue the most recent session for this cwd.
        #[serde(default = "default_false")]
        continue_last: bool,
        /// Working directory for session context.
        #[serde(default = "default_none_string")]
        cwd: Option<String>,
    },
    /// Attach to an existing session (returns the session socket path).
    AttachSession { session_id: String },
    /// Query the process tree.
    ProcessTree,
    /// Kill a specific session.
    KillSession { session_id: String },
    /// Shutdown the daemon.
    Shutdown,
    /// Daemon status (uptime, session count, resource usage).
    Status,
    /// Restart the daemon (checkpoint + exit with code 75).
    RestartDaemon,
    /// List loaded plugins.
    ListPlugins,
}

/// Responses from the control socket.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ControlResponse {
    /// List of active sessions.
    Sessions(Vec<SessionSummary>),
    /// New session created.
    Created { session_id: String, socket_path: String },
    /// Attached to existing session.
    Attached { socket_path: String },
    /// Process tree.
    Tree(Vec<ProcessInfo>),
    /// Session killed.
    Killed,
    /// Daemon shutting down.
    ShuttingDown,
    /// Daemon status.
    Status(DaemonStatus),
    /// Daemon is restarting (exit code 75).
    Restarting,
    /// Loaded plugin list.
    Plugins(Vec<crate::event::PluginSummary>),
    /// Error response.
    Error { message: String },
}

/// Summary of an active session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionSummary {
    pub session_id: String,
    pub model: String,
    pub turn_count: usize,
    pub last_active: String,
    pub client_count: usize,
    pub socket_path: String,
    /// Lifecycle state: "active", "suspended", or "recovering".
    #[serde(default = "default_session_state")]
    pub state: String,
}

fn default_session_state() -> String {
    "active".to_string()
}

/// Daemon runtime status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaemonStatus {
    pub uptime_secs: f64,
    pub session_count: usize,
    pub total_clients: usize,
    pub pid: u32,
}
