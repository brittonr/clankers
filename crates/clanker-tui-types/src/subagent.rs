//! Events from subagent processes to the TUI panel.

/// Events sent from subagent subprocesses to update the TUI panel.
#[derive(Debug, Clone)]
pub enum SubagentEvent {
    /// A new subagent was spawned (includes PID for kill support).
    Started {
        id: String,
        name: String,
        task: String,
        pid: Option<u32>,
    },
    /// A line of output from a subagent.
    Output { id: String, line: String },
    /// Subagent completed successfully.
    Done { id: String },
    /// Subagent failed.
    Error { id: String, message: String },
    /// Request to kill a running subagent (sent from TUI → tool layer).
    KillRequest { id: String },
    /// Send input text to a running subagent's stdin.
    InputRequest { id: String, text: String },
}
