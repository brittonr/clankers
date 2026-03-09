//! Core process monitor for tracking child processes and resource usage.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use parking_lot::RwLock;
use serde::Deserialize;
use serde::Serialize;
use sysinfo::Pid;
use sysinfo::ProcessesToUpdate;
use sysinfo::System;
use tokio::sync::broadcast;
use tokio::time;
use tokio_util::sync::CancellationToken;

use crate::agent::events::AgentEvent;

/// Metadata about why a process was spawned.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessMeta {
    /// Tool name that spawned the process
    pub tool_name: String,
    /// Command that was executed
    pub command: String,
    /// Tool call ID for correlation
    pub call_id: String,
}

/// A single resource usage sample.
#[derive(Debug, Clone)]
pub struct ResourceSnapshot {
    /// CPU usage as a percentage (0.0 - 100.0)
    pub cpu_percent: f32,
    /// Resident set size in bytes
    pub rss_bytes: u64,
    /// When this snapshot was taken
    pub timestamp: Instant,
}

/// Process lifecycle state.
#[derive(Debug, Clone)]
pub enum ProcessState {
    Running,
    Exited { code: Option<i32>, wall_time: Duration },
}

/// Full tracked state for a single process.
#[derive(Debug, Clone)]
pub struct TrackedProcess {
    /// Metadata about why this process was spawned
    pub meta: ProcessMeta,
    /// Current lifecycle state
    pub state: ProcessState,
    /// Resource usage history (limited by max_history)
    pub snapshots: Vec<ResourceSnapshot>,
    /// Direct child PIDs discovered via parent-walking
    pub children: Vec<u32>,
    /// Peak RSS observed across all samples
    pub peak_rss: u64,
    /// When tracking started
    pub start_time: Instant,
}

/// Aggregate statistics across all tracked processes.
#[derive(Debug, Clone)]
pub struct AggregateStats {
    /// Number of currently running processes
    pub active_count: usize,
    /// Number of finished processes in history
    pub finished_count: usize,
    /// Total RSS across all running processes
    pub total_rss: u64,
    /// Total CPU usage across all running processes
    pub total_cpu_percent: f32,
}

/// Configuration for the process monitor.
#[derive(Debug, Clone)]
pub struct ProcessMonitorConfig {
    /// How often to poll for updates
    pub poll_interval: Duration,
    /// Maximum number of snapshots to keep per process
    pub max_history: usize,
}

impl Default for ProcessMonitorConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_secs(2),
            max_history: 100,
        }
    }
}

/// Shared state for the process monitor.
struct ProcessMonitorInner {
    /// sysinfo System for querying process info
    system: System,
    /// Currently tracked processes
    tracked: HashMap<u32, TrackedProcess>,
    /// Finished processes
    history: Vec<(u32, TrackedProcess)>,
    /// Configuration
    config: ProcessMonitorConfig,
    /// Event bus for emitting events
    event_tx: Option<broadcast::Sender<AgentEvent>>,
}

/// Main process monitor struct.
pub struct ProcessMonitor {
    inner: Arc<RwLock<ProcessMonitorInner>>,
    cancel_token: CancellationToken,
}

/// Handle type for sharing the monitor.
pub type ProcessMonitorHandle = Arc<ProcessMonitor>;

impl ProcessMonitor {
    /// Create a new process monitor with the given configuration.
    pub fn new(config: ProcessMonitorConfig, event_tx: Option<broadcast::Sender<AgentEvent>>) -> Self {
        let inner = ProcessMonitorInner {
            system: System::new(),
            tracked: HashMap::new(),
            history: Vec::new(),
            config,
            event_tx,
        };

        Self {
            inner: Arc::new(RwLock::new(inner)),
            cancel_token: CancellationToken::new(),
        }
    }

    /// Register a new process to track.
    pub fn register(&self, pid: u32, meta: ProcessMeta) {
        let mut inner = self.inner.write();

        // Emit spawn event
        if let Some(ref tx) = inner.event_tx {
            let _ = tx.send(AgentEvent::ProcessSpawn {
                pid,
                meta: meta.clone(),
            });
        }

        let tracked = TrackedProcess {
            meta,
            state: ProcessState::Running,
            snapshots: Vec::new(),
            children: Vec::new(),
            peak_rss: 0,
            start_time: Instant::now(),
        };

        inner.tracked.insert(pid, tracked);
    }

    /// Get a snapshot of all currently tracked processes.
    pub fn snapshot(&self) -> Vec<(u32, TrackedProcess)> {
        let inner = self.inner.read();
        inner.tracked.iter().map(|(pid, proc)| (*pid, proc.clone())).collect()
    }

    /// Get aggregate statistics.
    pub fn aggregate(&self) -> AggregateStats {
        let inner = self.inner.read();

        let mut total_rss = 0;
        let mut total_cpu_percent = 0.0;

        for proc in inner.tracked.values() {
            if let Some(last) = proc.snapshots.last() {
                total_rss += last.rss_bytes;
                total_cpu_percent += last.cpu_percent;
            }
        }

        AggregateStats {
            active_count: inner.tracked.len(),
            finished_count: inner.history.len(),
            total_rss,
            total_cpu_percent,
        }
    }

    /// Get historical (finished) processes.
    pub fn history(&self) -> Vec<(u32, TrackedProcess)> {
        let inner = self.inner.read();
        inner.history.clone()
    }

    /// Start the background polling task.
    pub fn start(self: Arc<Self>) {
        let inner = Arc::clone(&self.inner);
        let cancel = self.cancel_token.clone();

        tokio::spawn(async move {
            Self::poll_loop(inner, cancel).await;
        });
    }

    /// Shutdown the monitor.
    pub fn shutdown(&self) {
        self.cancel_token.cancel();
    }

    /// Main poll loop that runs in the background.
    async fn poll_loop(inner: Arc<RwLock<ProcessMonitorInner>>, cancel: CancellationToken) {
        let poll_interval = {
            let guard = inner.read();
            guard.config.poll_interval
        };

        let mut interval = time::interval(poll_interval);
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                () = cancel.cancelled() => {
                    break;
                }
                _ = interval.tick() => {
                    Self::poll_once(&inner);
                }
            }
        }
    }

    /// Perform a single poll cycle.
    fn poll_once(inner: &Arc<RwLock<ProcessMonitorInner>>) {
        let mut guard = inner.write();

        // Refresh all processes to get both tracked PIDs and children
        guard.system.refresh_processes(ProcessesToUpdate::All, true);

        let now = Instant::now();
        let max_history = guard.config.max_history;

        // First pass: collect samples from sysinfo (immutable system access)
        // We collect the data we need, then apply mutations separately.
        let pids: Vec<u32> = guard.tracked.keys().copied().collect();

        struct SampleResult {
            pid: u32,
            cpu_percent: f32,
            rss_bytes: u64,
            children: Vec<u32>,
        }

        let mut samples: Vec<SampleResult> = Vec::new();
        let mut exited: Vec<(u32, Duration)> = Vec::new();

        for &pid in &pids {
            let sys_pid = Pid::from_u32(pid);
            if let Some(process) = guard.system.process(sys_pid) {
                samples.push(SampleResult {
                    pid,
                    cpu_percent: process.cpu_usage(),
                    rss_bytes: process.memory(),
                    children: Self::find_children(&guard.system, pid),
                });
            } else if let Some(tracked) = guard.tracked.get(&pid) {
                exited.push((pid, now - tracked.start_time));
            }
        }

        // Second pass: apply mutations to tracked state
        let mut events: Vec<AgentEvent> = Vec::new();

        for sample in &samples {
            if let Some(tracked) = guard.tracked.get_mut(&sample.pid) {
                if sample.rss_bytes > tracked.peak_rss {
                    tracked.peak_rss = sample.rss_bytes;
                }
                tracked.children = sample.children.clone();
                tracked.snapshots.push(ResourceSnapshot {
                    cpu_percent: sample.cpu_percent,
                    rss_bytes: sample.rss_bytes,
                    timestamp: now,
                });
                if tracked.snapshots.len() > max_history {
                    tracked.snapshots.remove(0);
                }
                events.push(AgentEvent::ProcessSample {
                    pid: sample.pid,
                    cpu_percent: sample.cpu_percent,
                    rss_bytes: sample.rss_bytes,
                    children: sample.children.clone(),
                });
            }
        }

        for &(pid, wall_time) in &exited {
            let peak_rss = guard.tracked.get(&pid).map(|t| t.peak_rss).unwrap_or(0);
            if let Some(tracked) = guard.tracked.get_mut(&pid) {
                tracked.state = ProcessState::Exited { code: None, wall_time };
            }
            events.push(AgentEvent::ProcessExit {
                pid,
                exit_code: None,
                wall_time,
                peak_rss,
            });
        }

        // Move finished processes to history
        for &(pid, _) in &exited {
            if let Some(tracked) = guard.tracked.remove(&pid) {
                guard.history.push((pid, tracked));
            }
        }

        // Emit events (no more mutable borrows of tracked)
        if let Some(ref tx) = guard.event_tx {
            for event in events {
                let _ = tx.send(event);
            }
        }
    }

    /// Find all child PIDs of a given parent PID.
    fn find_children(system: &System, parent_pid: u32) -> Vec<u32> {
        let mut children = Vec::new();
        let parent_sys_pid = Pid::from_u32(parent_pid);

        for (pid, process) in system.processes() {
            if let Some(ppid) = process.parent()
                && ppid == parent_sys_pid
            {
                children.push(pid.as_u32());
            }
        }

        children
    }

    // ── Test helpers ──────────────────────────────────────────────────────

    /// Inject a resource snapshot for a tracked process (test-only).
    #[cfg(test)]
    pub fn inject_snapshot(&self, pid: u32, snapshot: ResourceSnapshot) {
        let mut inner = self.inner.write();
        if let Some(tracked) = inner.tracked.get_mut(&pid) {
            if snapshot.rss_bytes > tracked.peak_rss {
                tracked.peak_rss = snapshot.rss_bytes;
            }
            tracked.snapshots.push(snapshot);
        }
    }

    /// Mark a tracked process as exited and move it to history (test-only).
    #[cfg(test)]
    pub fn mark_exited(&self, pid: u32, code: Option<i32>) {
        let mut inner = self.inner.write();
        let wall_time = inner.tracked.get(&pid).map(|t| t.start_time.elapsed()).unwrap_or_default();
        if let Some(tracked) = inner.tracked.get_mut(&pid) {
            tracked.state = ProcessState::Exited { code, wall_time };
        }
        if let Some(tracked) = inner.tracked.remove(&pid) {
            inner.history.push((pid, tracked));
        }
    }

    /// Add a child PID to a tracked process (test-only).
    #[cfg(test)]
    pub fn add_child(&self, parent_pid: u32, child_pid: u32) {
        let mut inner = self.inner.write();
        if let Some(tracked) = inner.tracked.get_mut(&parent_pid) {
            tracked.children.push(child_pid);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_adds_process() {
        let monitor = ProcessMonitor::new(ProcessMonitorConfig::default(), None);

        let meta = ProcessMeta {
            tool_name: "bash".to_string(),
            command: "echo test".to_string(),
            call_id: "test-123".to_string(),
        };

        monitor.register(12345, meta.clone());

        let snapshot = monitor.snapshot();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].0, 12345);
        assert_eq!(snapshot[0].1.meta.tool_name, "bash");
    }

    #[test]
    fn test_snapshot_returns_tracked() {
        let monitor = ProcessMonitor::new(ProcessMonitorConfig::default(), None);

        let meta1 = ProcessMeta {
            tool_name: "bash".to_string(),
            command: "cmd1".to_string(),
            call_id: "call-1".to_string(),
        };

        let meta2 = ProcessMeta {
            tool_name: "bash".to_string(),
            command: "cmd2".to_string(),
            call_id: "call-2".to_string(),
        };

        monitor.register(100, meta1);
        monitor.register(200, meta2);

        let snapshot = monitor.snapshot();
        assert_eq!(snapshot.len(), 2);

        let pids: Vec<u32> = snapshot.iter().map(|(pid, _)| *pid).collect();
        assert!(pids.contains(&100));
        assert!(pids.contains(&200));
    }

    #[test]
    fn test_aggregate_stats() {
        let monitor = ProcessMonitor::new(ProcessMonitorConfig::default(), None);

        let meta = ProcessMeta {
            tool_name: "test".to_string(),
            command: "test".to_string(),
            call_id: "test".to_string(),
        };

        monitor.register(123, meta);

        let stats = monitor.aggregate();
        assert_eq!(stats.active_count, 1);
        assert_eq!(stats.finished_count, 0);
    }

    #[test]
    fn test_history_empty_initially() {
        let monitor = ProcessMonitor::new(ProcessMonitorConfig::default(), None);
        let history = monitor.history();
        assert_eq!(history.len(), 0);
    }

    #[test]
    fn test_snapshot_limit_enforced() {
        // Create a tracked process and add snapshots beyond max_history
        let config = ProcessMonitorConfig {
            poll_interval: Duration::from_secs(1),
            max_history: 5,
        };

        let mut tracked = TrackedProcess {
            meta: ProcessMeta {
                tool_name: "test".to_string(),
                command: "test".to_string(),
                call_id: "test".to_string(),
            },
            state: ProcessState::Running,
            snapshots: Vec::new(),
            children: Vec::new(),
            peak_rss: 0,
            start_time: Instant::now(),
        };

        // Add 10 snapshots
        for i in 0..10 {
            tracked.snapshots.push(ResourceSnapshot {
                cpu_percent: i as f32,
                rss_bytes: i * 1000,
                timestamp: Instant::now(),
            });

            // Simulate trimming
            if tracked.snapshots.len() > config.max_history {
                tracked.snapshots.remove(0);
            }
        }

        // Should have only max_history snapshots
        assert_eq!(tracked.snapshots.len(), 5);
        // First snapshot should be index 5 (0-4 removed)
        assert_eq!(tracked.snapshots[0].cpu_percent, 5.0);
    }
}
