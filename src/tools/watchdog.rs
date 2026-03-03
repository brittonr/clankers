//! Subagent health watchdog
//!
//! Monitors subagent liveness by tracking output activity. If a subagent
//! produces no output for a configurable timeout, it is flagged as stalled.
//! After a longer timeout, the watchdog can kill and restart the subagent.
//!
//! This sits alongside `subagent.rs` — it wraps the existing subprocess
//! spawning with health monitoring.

use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use parking_lot::Mutex;
use tokio_util::sync::CancellationToken;
use tracing::info;
use tracing::warn;

use crate::tui::components::subagent_event::SubagentEvent;

type PanelTx = tokio::sync::mpsc::UnboundedSender<SubagentEvent>;

/// Health state of a monitored subagent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthState {
    /// Actively producing output
    Healthy,
    /// No output for `stall_timeout` — may be stuck
    Stalled,
    /// No output for `kill_timeout` — will be killed
    Unresponsive,
    /// Completed (success or failure)
    Finished,
}

impl std::fmt::Display for HealthState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthState::Healthy => write!(f, "healthy"),
            HealthState::Stalled => write!(f, "stalled"),
            HealthState::Unresponsive => write!(f, "unresponsive"),
            HealthState::Finished => write!(f, "finished"),
        }
    }
}

/// Watchdog configuration.
#[derive(Debug, Clone)]
pub struct WatchdogConfig {
    /// Time without output before flagging as stalled (default: 120s)
    pub stall_timeout: Duration,
    /// Time without output before killing (default: 300s, None = never kill)
    pub kill_timeout: Option<Duration>,
    /// How often to check health (default: 10s)
    pub check_interval: Duration,
}

impl Default for WatchdogConfig {
    fn default() -> Self {
        Self {
            stall_timeout: Duration::from_secs(120),
            kill_timeout: Some(Duration::from_secs(300)),
            check_interval: Duration::from_secs(10),
        }
    }
}

/// Shared liveness state updated by the output reader, checked by the watchdog.
#[derive(Debug)]
pub struct LivenessTracker {
    last_output: Mutex<Instant>,
    state: Mutex<HealthState>,
    subagent_id: String,
}

impl LivenessTracker {
    pub fn new(subagent_id: String) -> Arc<Self> {
        Arc::new(Self {
            last_output: Mutex::new(Instant::now()),
            state: Mutex::new(HealthState::Healthy),
            subagent_id,
        })
    }

    /// Call this every time the subagent produces output.
    pub fn record_output(&self) {
        *self.last_output.lock() = Instant::now();
        let mut state = self.state.lock();
        if *state == HealthState::Stalled {
            info!("subagent {} recovered from stall", self.subagent_id);
            *state = HealthState::Healthy;
        }
    }

    /// Mark as finished (prevents further state transitions).
    pub fn mark_finished(&self) {
        *self.state.lock() = HealthState::Finished;
    }

    /// Get the current health state.
    pub fn state(&self) -> HealthState {
        self.state.lock().clone()
    }

    /// Get time since last output.
    pub fn idle_duration(&self) -> Duration {
        self.last_output.lock().elapsed()
    }

    pub fn id(&self) -> &str {
        &self.subagent_id
    }
}

/// Spawn a background watchdog task that monitors a subagent's liveness.
///
/// Returns a `CancellationToken` to stop the watchdog when the subagent ends.
/// The watchdog emits `SubagentEvent::Stalled` / `SubagentEvent::Error` to
/// the panel.
pub fn spawn_watchdog(
    tracker: Arc<LivenessTracker>,
    config: WatchdogConfig,
    panel_tx: Option<PanelTx>,
    kill_signal: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut stall_notified = false;

        loop {
            tokio::select! {
                _ = tokio::time::sleep(config.check_interval) => {}
                _ = kill_signal.cancelled() => break,
            }

            let state = tracker.state();
            if state == HealthState::Finished {
                break;
            }

            let idle = tracker.idle_duration();

            // Check kill timeout first
            if let Some(kill_timeout) = config.kill_timeout {
                if idle >= kill_timeout {
                    warn!(
                        "subagent {} unresponsive for {:.0}s (kill timeout {:.0}s), killing",
                        tracker.id(),
                        idle.as_secs_f64(),
                        kill_timeout.as_secs_f64()
                    );
                    *tracker.state.lock() = HealthState::Unresponsive;
                    if let Some(ref tx) = panel_tx {
                        let _ = tx.send(SubagentEvent::Error {
                            id: tracker.id().to_string(),
                            message: format!(
                                "Watchdog: no output for {:.0}s, killing subagent",
                                idle.as_secs_f64()
                            ),
                        });
                        let _ = tx.send(SubagentEvent::KillRequest {
                            id: tracker.id().to_string(),
                        });
                    }
                    break;
                }
            }

            // Check stall timeout
            if idle >= config.stall_timeout && !stall_notified {
                warn!(
                    "subagent {} stalled: no output for {:.0}s",
                    tracker.id(),
                    idle.as_secs_f64()
                );
                *tracker.state.lock() = HealthState::Stalled;
                stall_notified = true;
                if let Some(ref tx) = panel_tx {
                    let _ = tx.send(SubagentEvent::Output {
                        id: tracker.id().to_string(),
                        line: format!(
                            "⚠️  WATCHDOG: no output for {:.0}s — subagent may be stuck",
                            idle.as_secs_f64()
                        ),
                    });
                }
            }

            // Reset stall notification if output resumed
            if idle < config.stall_timeout && stall_notified {
                stall_notified = false;
            }
        }
    })
}

/// Status summary for all tracked subagents.
#[derive(Debug, Default)]
pub struct WatchdogRegistry {
    trackers: Mutex<Vec<Arc<LivenessTracker>>>,
}

impl WatchdogRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new tracker. Called when a subagent is spawned.
    pub fn register(&self, tracker: Arc<LivenessTracker>) {
        self.trackers.lock().push(tracker);
    }

    /// Get a snapshot of all active (non-finished) subagent health.
    pub fn health_report(&self) -> Vec<(String, HealthState, Duration)> {
        let trackers = self.trackers.lock();
        trackers
            .iter()
            .filter(|t| t.state() != HealthState::Finished)
            .map(|t| (t.id().to_string(), t.state(), t.idle_duration()))
            .collect()
    }

    /// Remove finished trackers.
    pub fn gc(&self) {
        self.trackers.lock().retain(|t| t.state() != HealthState::Finished);
    }

    /// Count of active (non-finished) subagents.
    pub fn active_count(&self) -> usize {
        self.trackers.lock().iter().filter(|t| t.state() != HealthState::Finished).count()
    }

    /// Count of stalled subagents.
    pub fn stalled_count(&self) -> usize {
        self.trackers
            .lock()
            .iter()
            .filter(|t| matches!(t.state(), HealthState::Stalled | HealthState::Unresponsive))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_liveness_tracker_initial_state() {
        let tracker = LivenessTracker::new("test-1".into());
        assert_eq!(tracker.state(), HealthState::Healthy);
        assert!(tracker.idle_duration() < Duration::from_secs(1));
    }

    #[test]
    fn test_liveness_tracker_record_output() {
        let tracker = LivenessTracker::new("test-2".into());
        // Artificially set last_output to the past
        *tracker.last_output.lock() = Instant::now() - Duration::from_secs(200);
        *tracker.state.lock() = HealthState::Stalled;

        tracker.record_output();
        assert_eq!(tracker.state(), HealthState::Healthy);
        assert!(tracker.idle_duration() < Duration::from_secs(1));
    }

    #[test]
    fn test_liveness_tracker_mark_finished() {
        let tracker = LivenessTracker::new("test-3".into());
        tracker.mark_finished();
        assert_eq!(tracker.state(), HealthState::Finished);

        // record_output should not change state once finished
        tracker.record_output();
        // state stays finished because record_output only changes Stalled→Healthy
        assert_eq!(tracker.state(), HealthState::Finished);
    }

    #[test]
    fn test_watchdog_registry() {
        let registry = WatchdogRegistry::new();
        let t1 = LivenessTracker::new("a".into());
        let t2 = LivenessTracker::new("b".into());

        registry.register(t1.clone());
        registry.register(t2.clone());
        assert_eq!(registry.active_count(), 2);

        t1.mark_finished();
        assert_eq!(registry.active_count(), 1);

        registry.gc();
        assert_eq!(registry.health_report().len(), 1);
        assert_eq!(registry.health_report()[0].0, "b");
    }

    #[tokio::test]
    async fn test_watchdog_detects_stall() {
        let tracker = LivenessTracker::new("stall-test".into());
        // Set last output to 200s ago
        *tracker.last_output.lock() = Instant::now() - Duration::from_secs(200);

        let config = WatchdogConfig {
            stall_timeout: Duration::from_millis(50),
            kill_timeout: None,
            check_interval: Duration::from_millis(20),
        };

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let cancel = CancellationToken::new();

        let handle = spawn_watchdog(tracker.clone(), config, Some(tx), cancel.clone());

        // Wait for stall detection
        let event = tokio::time::timeout(Duration::from_secs(1), rx.recv()).await.unwrap().unwrap();
        match event {
            SubagentEvent::Output { id, line } => {
                assert_eq!(id, "stall-test");
                assert!(line.contains("WATCHDOG"));
            }
            other => panic!("expected Output event, got {:?}", other),
        }

        cancel.cancel();
        let _ = handle.await;
    }

    #[tokio::test]
    async fn test_watchdog_kills_unresponsive() {
        let tracker = LivenessTracker::new("kill-test".into());
        *tracker.last_output.lock() = Instant::now() - Duration::from_secs(500);

        let config = WatchdogConfig {
            stall_timeout: Duration::from_millis(10),
            kill_timeout: Some(Duration::from_millis(20)),
            check_interval: Duration::from_millis(5),
        };

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let cancel = CancellationToken::new();

        let handle = spawn_watchdog(tracker.clone(), config, Some(tx), cancel.clone());

        // Collect events until we see KillRequest
        let mut got_kill = false;
        for _ in 0..20 {
            if let Ok(Some(event)) = tokio::time::timeout(Duration::from_millis(100), rx.recv()).await {
                if matches!(event, SubagentEvent::KillRequest { .. }) {
                    got_kill = true;
                    break;
                }
            }
        }

        assert!(got_kill, "should have received KillRequest");
        assert_eq!(tracker.state(), HealthState::Unresponsive);

        cancel.cancel();
        let _ = handle.await;
    }
}
