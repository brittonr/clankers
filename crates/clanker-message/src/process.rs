//! Neutral process observation contracts shared by process monitors and display edges.

use std::time::Duration;

use serde::Deserialize;
use serde::Serialize;

/// Information about a process in an actor/process tree.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProcessInfo {
    pub id: u64,
    pub name: Option<String>,
    pub parent: Option<u64>,
    pub children: Vec<u64>,
    pub state: ProcessState,
    pub uptime_secs: f64,
}

/// State of a process in an actor/process tree.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProcessState {
    Running,
    ShuttingDown,
    Dead,
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_state_roundtrip() {
        for state in [ProcessState::Running, ProcessState::ShuttingDown, ProcessState::Dead] {
            let json = serde_json::to_string(&state).expect("state should serialize");
            let decoded: ProcessState = serde_json::from_str(&json).expect("state should deserialize");
            assert_eq!(state, decoded);
        }
    }

    #[test]
    fn process_info_roundtrip_preserves_tree_fields() {
        let info = ProcessInfo {
            id: 1,
            name: Some("agent".to_string()),
            parent: None,
            children: vec![2, 3],
            state: ProcessState::Running,
            uptime_secs: 1.5,
        };
        let json = serde_json::to_string(&info).expect("info should serialize");
        let decoded: ProcessInfo = serde_json::from_str(&json).expect("info should deserialize");
        assert_eq!(decoded, info);
    }
}
