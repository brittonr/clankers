//! Neutral process observation contracts shared by process monitors and display edges.

use std::time::Duration;

use serde::Deserialize;
use serde::Serialize;

/// Events emitted by a process monitor.
#[derive(Debug, Clone)]
pub enum ProcessEvent {
    /// A new process was registered.
    Spawn { pid: u32, meta: ProcessMeta },
    /// A resource usage sample for a tracked process.
    Sample {
        pid: u32,
        cpu_percent: f32,
        rss_bytes: u64,
        children: Vec<u32>,
    },
    /// A tracked process has exited.
    Exit {
        pid: u32,
        exit_code: Option<i32>,
        wall_time: Duration,
        peak_rss: u64,
    },
}

/// Metadata about why a process was spawned.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessMeta {
    /// Tool name that spawned the process.
    pub tool_name: String,
    /// Command that was executed.
    pub command: String,
    /// Tool call ID for correlation.
    pub call_id: String,
}

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
