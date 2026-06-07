//! Neutral process observation contracts shared by process monitors and display edges.

use std::time::Duration;

/// A snapshot of a tracked process for display or receipts.
#[derive(Debug, Clone)]
pub struct ProcessSnapshot {
    pub pid: u32,
    pub cpu_percent: f32,
    pub rss_bytes: u64,
    pub peak_rss: u64,
    pub command: String,
    pub tool_name: String,
    pub call_id: String,
    pub elapsed: Duration,
    pub state: ProcessDisplayState,
    pub cpu_history: Vec<f32>,
    pub mem_history: Vec<f32>,
    pub children: Vec<u32>,
}

/// Process state as seen by observers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessDisplayState {
    Running,
    Exited { code: Option<i32>, wall_time: Duration },
}

/// Trait for providing process observation data without a TUI dependency.
pub trait ProcessDataSource: Send + Sync {
    /// Get currently running processes.
    fn active_processes(&self) -> Vec<ProcessSnapshot>;
    /// Get completed/historical processes.
    fn completed_processes(&self) -> Vec<ProcessSnapshot>;
}
