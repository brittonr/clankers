//! Process identity and handle types.

use std::fmt;
use std::time::Instant;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::signal::Signal;

/// Unique monotonic process identifier. Never reused within a runtime instance.
pub type ProcessId = u64;

/// Why a process terminated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeathReason {
    /// Process finished its work normally.
    Normal,
    /// Process failed with an error.
    Failed(String),
    /// Process was killed by a signal.
    Killed,
    /// Process was shut down gracefully.
    Shutdown,
}

impl fmt::Display for DeathReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeathReason::Normal => write!(f, "normal"),
            DeathReason::Failed(msg) => write!(f, "failed: {msg}"),
            DeathReason::Killed => write!(f, "killed"),
            DeathReason::Shutdown => write!(f, "shutdown"),
        }
    }
}

/// Handle to interact with a running actor process.
pub struct ProcessHandle {
    pub id: ProcessId,
    pub signal_tx: mpsc::UnboundedSender<Signal>,
    pub join: Option<JoinHandle<DeathReason>>,
    pub name: Option<String>,
    pub parent: Option<ProcessId>,
    pub started_at: Instant,
}

impl ProcessHandle {
    /// Send a signal to this process. Fire-and-forget — does not error
    /// if the process has terminated (matches Erlang semantics).
    pub fn send(&self, signal: Signal) -> bool {
        self.signal_tx.send(signal).is_ok()
    }

    /// How long this process has been running.
    pub fn uptime(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }
}

impl fmt::Debug for ProcessHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProcessHandle")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("parent", &self.parent)
            .finish()
    }
}
