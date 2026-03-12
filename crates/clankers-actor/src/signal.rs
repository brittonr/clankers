//! Signal types for inter-process communication.

use std::any::Any;
use std::fmt;
use std::time::Duration;

use crate::process::DeathReason;
use crate::process::ProcessId;

/// Signals sent between processes in the actor tree.
pub enum Signal {
    /// Opaque application message (typed via downcast).
    Message(Box<dyn Any + Send>),
    /// Immediate termination — no cleanup.
    Kill,
    /// Graceful shutdown with a timeout before Kill.
    Shutdown { timeout: Duration },
    /// Establish a bidirectional link.
    Link { tag: Option<i64>, process_id: ProcessId },
    /// Remove a link.
    UnLink { process_id: ProcessId },
    /// Notification that a linked process died.
    LinkDied {
        process_id: ProcessId,
        tag: Option<i64>,
        reason: DeathReason,
    },
    /// Start monitoring (unidirectional — monitor gets notified, target doesn't).
    Monitor { watcher: ProcessId },
    /// Stop monitoring.
    StopMonitoring { watcher: ProcessId },
    /// Notification that a monitored process exited.
    ProcessDied { process_id: ProcessId, reason: DeathReason },
}

impl fmt::Debug for Signal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Signal::Message(_) => write!(f, "Signal::Message(...)"),
            Signal::Kill => write!(f, "Signal::Kill"),
            Signal::Shutdown { timeout } => write!(f, "Signal::Shutdown {{ timeout: {timeout:?} }}"),
            Signal::Link { tag, process_id } => {
                write!(f, "Signal::Link {{ tag: {tag:?}, process_id: {process_id} }}")
            }
            Signal::UnLink { process_id } => write!(f, "Signal::UnLink {{ process_id: {process_id} }}"),
            Signal::LinkDied {
                process_id,
                tag,
                reason,
            } => write!(f, "Signal::LinkDied {{ process_id: {process_id}, tag: {tag:?}, reason: {reason:?} }}"),
            Signal::Monitor { watcher } => write!(f, "Signal::Monitor {{ watcher: {watcher} }}"),
            Signal::StopMonitoring { watcher } => {
                write!(f, "Signal::StopMonitoring {{ watcher: {watcher} }}")
            }
            Signal::ProcessDied { process_id, reason } => {
                write!(f, "Signal::ProcessDied {{ process_id: {process_id}, reason: {reason:?} }}")
            }
        }
    }
}
