//! Loop engine — manages active loops and dispatches events.
//!
//! The engine tracks active loops and provides the iteration driver.
//! Actual execution of each iteration is the caller's responsibility;
//! the engine handles state transitions, condition checking, and
//! event emission.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use parking_lot::Mutex;
use tokio::sync::broadcast;
use tracing::info;

use crate::iteration::IterationResult;
use crate::iteration::LoopDef;
use crate::iteration::LoopId;
use crate::iteration::LoopState;
use crate::iteration::LoopStatus;
use crate::iteration::MAX_ACTIVE_LOOPS;

/// Tiger Style: named constant for event channel capacity.
const EVENT_CHANNEL_CAPACITY: usize = 256;

/// Event emitted during loop execution.
#[derive(Debug, Clone)]
pub enum LoopEvent {
    /// A loop was started.
    Started {
        loop_id: LoopId,
        name: String,
    },
    /// An iteration completed.
    IterationComplete {
        loop_id: LoopId,
        name: String,
        iteration: u32,
        output: String,
        break_matched: bool,
    },
    /// The loop finished (completed, stopped, or failed).
    Finished {
        loop_id: LoopId,
        name: String,
        status: LoopStatus,
        total_iterations: u32,
        elapsed_secs: i64,
    },
}

type Loops = Arc<Mutex<HashMap<LoopId, LoopState>>>;

/// Manages active loops and their lifecycle.
pub struct LoopEngine {
    loops: Loops,
    event_tx: broadcast::Sender<LoopEvent>,
}

impl LoopEngine {
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            loops: Arc::new(Mutex::new(HashMap::new())),
            event_tx,
        }
    }

    /// Subscribe to loop events.
    pub fn subscribe(&self) -> broadcast::Receiver<LoopEvent> {
        self.event_tx.subscribe()
    }

    /// Register a new loop (in Pending state). Returns the loop ID.
    ///
    /// # Tiger Style
    ///
    /// Enforces `MAX_ACTIVE_LOOPS` to prevent unbounded resource usage.
    /// Returns `None` if the limit is reached.
    pub fn register(&self, def: LoopDef) -> Option<LoopId> {
        let id = def.id.clone();
        let state = LoopState::new(def);
        let mut loops = self.loops.lock();

        if loops.len() >= MAX_ACTIVE_LOOPS as usize {
            tracing::warn!(
                "loop registration rejected: {} active loops (max {})",
                loops.len(),
                MAX_ACTIVE_LOOPS
            );
            return None;
        }

        info!("loop registered: {} ({})", state.def.name, id);
        loops.insert(id.clone(), state);
        Some(id)
    }

    /// Start a loop (transitions Pending -> Running).
    pub fn start(&self, id: &LoopId) -> bool {
        let mut loops = self.loops.lock();
        if let Some(state) = loops.get_mut(id)
            && state.status == LoopStatus::Pending
        {
            state.start();
            let _ = self.event_tx.send(LoopEvent::Started {
                loop_id: id.clone(),
                name: state.def.name.clone(),
            });
            return true;
        }
        false
    }

    /// Signal an out-of-band break for a running loop. The next call to
    /// `record_iteration` will treat the iteration as break-matched and
    /// stop the loop.
    ///
    /// Use this for external break signals (e.g. `signal_loop_success`
    /// tool call) that aren't detectable from iteration output.
    pub fn signal_break(&self, id: &LoopId) -> bool {
        let mut loops = self.loops.lock();
        if let Some(state) = loops.get_mut(id)
            && state.status == LoopStatus::Running
        {
            state.break_signaled = true;
            return true;
        }
        false
    }

    /// Record an iteration result. Returns true if the loop should continue.
    pub fn record_iteration(
        &self,
        id: &LoopId,
        output: String,
        exit_code: Option<i32>,
    ) -> bool {
        let mut loops = self.loops.lock();
        let Some(state) = loops.get_mut(id) else {
            return false;
        };

        if state.status != LoopStatus::Running {
            return false;
        }

        let break_matched = state.break_signaled || state.check_break(&output, exit_code);
        state.break_signaled = false;
        let iteration_idx = state.current_iteration;

        let result = IterationResult {
            index: iteration_idx,
            output: output.clone(),
            exit_code,
            started_at: Utc::now(), // approximate; real timing is caller's job
            finished_at: Utc::now(),
            break_matched,
        };

        let should_continue = state.record_iteration(result);

        let _ = self.event_tx.send(LoopEvent::IterationComplete {
            loop_id: id.clone(),
            name: state.def.name.clone(),
            iteration: iteration_idx,
            output,
            break_matched,
        });

        if !should_continue {
            let _ = self.event_tx.send(LoopEvent::Finished {
                loop_id: id.clone(),
                name: state.def.name.clone(),
                status: state.status,
                total_iterations: state.current_iteration,
                elapsed_secs: state.elapsed_secs(),
            });
        }

        should_continue
    }

    /// Stop a running loop.
    pub fn stop(&self, id: &LoopId) -> bool {
        let mut loops = self.loops.lock();
        if let Some(state) = loops.get_mut(id)
            && state.status == LoopStatus::Running
        {
            state.stop();
            let _ = self.event_tx.send(LoopEvent::Finished {
                loop_id: id.clone(),
                name: state.def.name.clone(),
                status: LoopStatus::Stopped,
                total_iterations: state.current_iteration,
                elapsed_secs: state.elapsed_secs(),
            });
            return true;
        }
        false
    }

    /// Mark a loop as failed.
    pub fn fail(&self, id: &LoopId) -> bool {
        let mut loops = self.loops.lock();
        if let Some(state) = loops.get_mut(id)
            && state.status == LoopStatus::Running
        {
            state.fail();
            let _ = self.event_tx.send(LoopEvent::Finished {
                loop_id: id.clone(),
                name: state.def.name.clone(),
                status: LoopStatus::Failed,
                total_iterations: state.current_iteration,
                elapsed_secs: state.elapsed_secs(),
            });
            return true;
        }
        false
    }

    /// Get a snapshot of a specific loop.
    pub fn get(&self, id: &LoopId) -> Option<LoopState> {
        self.loops.lock().get(id).cloned()
    }

    /// Get all active (non-finished) loops.
    pub fn active(&self) -> Vec<LoopState> {
        self.loops
            .lock()
            .values()
            .filter(|s| matches!(s.status, LoopStatus::Pending | LoopStatus::Running))
            .cloned()
            .collect()
    }

    /// Get all loops (including finished).
    pub fn all(&self) -> Vec<LoopState> {
        self.loops.lock().values().cloned().collect()
    }

    /// Remove a loop by ID.
    pub fn remove(&self, id: &LoopId) -> Option<LoopState> {
        self.loops.lock().remove(id)
    }

    /// Remove all completed/stopped/failed loops.
    pub fn gc(&self) -> usize {
        let mut loops = self.loops.lock();
        let before = loops.len();
        loops.retain(|_, s| matches!(s.status, LoopStatus::Pending | LoopStatus::Running));
        before - loops.len()
    }
}

impl Default for LoopEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::condition::BreakCondition;

    #[test]
    fn register_and_list() {
        let engine = LoopEngine::new();
        let def = LoopDef::fixed("test-loop", 5, json!({"cmd": "echo hi"}));
        let id = engine.register(def).unwrap();

        let all = engine.all();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].def.id, id);
        assert_eq!(all[0].status, LoopStatus::Pending);
    }

    #[test]
    fn start_transitions_to_running() {
        let engine = LoopEngine::new();
        let def = LoopDef::fixed("run-me", 3, json!({}));
        let id = engine.register(def).unwrap();

        assert!(engine.start(&id));
        assert_eq!(engine.get(&id).unwrap().status, LoopStatus::Running);
    }

    #[test]
    fn fixed_loop_through_engine() {
        let engine = LoopEngine::new();
        let mut rx = engine.subscribe();

        let def = LoopDef::fixed("counter", 3, json!({}));
        let id = engine.register(def).unwrap();
        engine.start(&id);

        assert!(engine.record_iteration(&id, "iter 0".into(), None));
        assert!(engine.record_iteration(&id, "iter 1".into(), None));
        assert!(!engine.record_iteration(&id, "iter 2".into(), None));

        let state = engine.get(&id).unwrap();
        assert_eq!(state.status, LoopStatus::Completed);
        assert_eq!(state.current_iteration, 3);

        // Should have: Started + 3*IterationComplete + Finished = 5 events
        let mut event_count = 0;
        while rx.try_recv().is_ok() {
            event_count += 1;
        }
        assert_eq!(event_count, 5);
    }

    #[test]
    fn until_loop_breaks_on_match() {
        let engine = LoopEngine::new();
        let def = LoopDef::until(
            "wait-for-ok",
            BreakCondition::Contains("OK".into()),
            json!({}),
        );
        let id = engine.register(def).unwrap();
        engine.start(&id);

        assert!(engine.record_iteration(&id, "not yet".into(), None));
        assert!(engine.record_iteration(&id, "still waiting".into(), None));
        assert!(!engine.record_iteration(&id, "status: OK".into(), None));

        let state = engine.get(&id).unwrap();
        assert_eq!(state.status, LoopStatus::Completed);
        assert_eq!(state.current_iteration, 3);
    }

    #[test]
    fn stop_running_loop() {
        let engine = LoopEngine::new();
        let def = LoopDef::fixed("stoppable", 100, json!({}));
        let id = engine.register(def).unwrap();
        engine.start(&id);

        engine.record_iteration(&id, "iter 0".into(), None);
        assert!(engine.stop(&id));

        let state = engine.get(&id).unwrap();
        assert_eq!(state.status, LoopStatus::Stopped);
    }

    #[test]
    fn gc_removes_finished() {
        let engine = LoopEngine::new();

        let def1 = LoopDef::fixed("done", 1, json!({}));
        let id1 = engine.register(def1).unwrap();
        engine.start(&id1);
        engine.record_iteration(&id1, "".into(), None);

        let def2 = LoopDef::fixed("still-running", 100, json!({}));
        let id2 = engine.register(def2).unwrap();
        engine.start(&id2);

        assert_eq!(engine.all().len(), 2);
        let removed = engine.gc();
        assert_eq!(removed, 1);
        assert_eq!(engine.all().len(), 1);
        assert_eq!(engine.all()[0].def.name, "still-running");
    }

    #[test]
    fn signal_break_stops_loop() {
        let engine = LoopEngine::new();
        let def = LoopDef::fixed("signaled", 100, json!({}));
        let id = engine.register(def).unwrap();
        engine.start(&id);

        // Run one normal iteration
        assert!(engine.record_iteration(&id, "iter 0".into(), None));

        // Signal break
        assert!(engine.signal_break(&id));

        // Next record_iteration should return false (loop completed)
        assert!(!engine.record_iteration(&id, "iter 1".into(), None));

        let state = engine.get(&id).unwrap();
        assert_eq!(state.status, LoopStatus::Completed);
        assert_eq!(state.current_iteration, 2);
    }

    #[test]
    fn active_filters_finished() {
        let engine = LoopEngine::new();

        let def1 = LoopDef::fixed("finished", 1, json!({}));
        let id1 = engine.register(def1).unwrap();
        engine.start(&id1);
        engine.record_iteration(&id1, "".into(), None);

        let def2 = LoopDef::fixed("active", 10, json!({}));
        engine.register(def2).unwrap();

        assert_eq!(engine.active().len(), 1);
        assert_eq!(engine.active()[0].def.name, "active"); // pending counts as active
    }
}
