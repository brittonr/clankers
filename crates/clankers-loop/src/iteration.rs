//! Loop definition, state tracking, and iteration results.

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

use crate::condition::BreakCondition;

/// Tiger Style: fixed limits for loop engine resources.
/// Maximum number of concurrently tracked loops.
pub const MAX_ACTIVE_LOOPS: u32 = 64;
/// Maximum iterations any single loop can run (hard safety valve).
pub const MAX_ITERATIONS_HARD_LIMIT: u32 = 10_000;

// Tiger Style: compile-time constant assertions
const _: () = assert!(MAX_ACTIVE_LOOPS > 0);
const _: () = assert!(MAX_ITERATIONS_HARD_LIMIT > 0);
const _: () = assert!(MAX_ACTIVE_LOOPS <= 256);
const _: () = assert!(MAX_ITERATIONS_HARD_LIMIT <= 100_000);

/// Unique loop identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LoopId(pub String);

impl LoopId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }
}

impl std::fmt::Display for LoopId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What kind of iteration strategy to use.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoopKind {
    /// Run exactly N iterations.
    Fixed { count: u32 },
    /// Run until the break condition matches.
    Until { condition: BreakCondition },
    /// Run at intervals until the break condition matches or timeout.
    Poll {
        interval_secs: u64,
        condition: BreakCondition,
        timeout_secs: Option<u64>,
    },
}

/// Current loop status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoopStatus {
    /// Not yet started.
    Pending,
    /// Actively iterating.
    Running,
    /// Break condition was satisfied.
    Completed,
    /// Hit max_iterations, timeout, or explicit stop.
    Stopped,
    /// An iteration failed and the loop aborted.
    Failed,
}

/// Result of a single iteration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationResult {
    /// Zero-based iteration index.
    pub index: u32,
    /// Output text from this iteration.
    pub output: String,
    /// Exit code (for command-based iterations).
    pub exit_code: Option<i32>,
    /// When this iteration started.
    pub started_at: DateTime<Utc>,
    /// When this iteration finished.
    pub finished_at: DateTime<Utc>,
    /// Whether the break condition was satisfied after this iteration.
    pub break_matched: bool,
}

/// Full loop definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopDef {
    pub id: LoopId,
    pub name: String,
    pub kind: LoopKind,
    /// What to execute each iteration. Interpreted by the consumer.
    /// Typically `{"command": "cargo test"}` or `{"prompt": "check if X is ready"}`.
    pub action: serde_json::Value,
    /// Hard cap on iterations (safety valve). Default: 100.
    pub max_iterations: u32,
    /// Created timestamp.
    pub created_at: DateTime<Utc>,
}

impl LoopDef {
    pub fn fixed(name: impl Into<String>, count: u32, action: serde_json::Value) -> Self {
        Self {
            id: LoopId::generate(),
            name: name.into(),
            kind: LoopKind::Fixed { count },
            action,
            max_iterations: count,
            created_at: Utc::now(),
        }
    }

    pub fn until(
        name: impl Into<String>,
        condition: BreakCondition,
        action: serde_json::Value,
    ) -> Self {
        Self {
            id: LoopId::generate(),
            name: name.into(),
            kind: LoopKind::Until { condition },
            action,
            max_iterations: 100,
            created_at: Utc::now(),
        }
    }

    pub fn poll(
        name: impl Into<String>,
        interval_secs: u64,
        condition: BreakCondition,
        timeout_secs: Option<u64>,
        action: serde_json::Value,
    ) -> Self {
        Self {
            id: LoopId::generate(),
            name: name.into(),
            kind: LoopKind::Poll {
                interval_secs,
                condition,
                timeout_secs,
            },
            action,
            max_iterations: 1000,
            created_at: Utc::now(),
        }
    }

    /// Set maximum iterations.
    ///
    /// # Tiger Style
    ///
    /// Capped at `MAX_ITERATIONS_HARD_LIMIT` to prevent unbounded execution.
    pub fn with_max_iterations(mut self, max: u32) -> Self {
        self.max_iterations = max.min(MAX_ITERATIONS_HARD_LIMIT);
        self
    }
}

/// Runtime state of an active loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopState {
    pub def: LoopDef,
    pub status: LoopStatus,
    pub current_iteration: u32,
    pub results: Vec<IterationResult>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    /// Out-of-band break signal. Set by `LoopEngine::signal_break()`,
    /// consumed by `record_iteration()`.
    #[serde(skip)]
    pub(crate) break_signaled: bool,
}

impl LoopState {
    pub fn new(def: LoopDef) -> Self {
        Self {
            def,
            status: LoopStatus::Pending,
            current_iteration: 0,
            results: Vec::new(),
            started_at: None,
            finished_at: None,
            break_signaled: false,
        }
    }

    /// Start the loop.
    ///
    /// # Tiger Style
    ///
    /// Asserts the loop is in `Pending` state before transitioning.
    /// Starting a non-pending loop is a programmer error.
    pub fn start(&mut self) {
        assert_eq!(self.status, LoopStatus::Pending, "can only start a pending loop");
        assert!(self.started_at.is_none(), "started_at must be None before start");
        self.status = LoopStatus::Running;
        self.started_at = Some(Utc::now());
    }

    /// Record an iteration result. Returns true if the loop should continue.
    ///
    /// # Tiger Style
    ///
    /// Asserts the loop is running and iteration index is consistent.
    /// Bounded by `max_iterations` (safety valve).
    pub fn record_iteration(&mut self, result: IterationResult) -> bool {
        assert_eq!(self.status, LoopStatus::Running, "can only record in running state");
        assert_eq!(
            result.index, self.current_iteration,
            "iteration index must match current_iteration"
        );

        let broke = result.break_matched;
        self.results.push(result);
        self.current_iteration += 1;

        if broke {
            self.status = LoopStatus::Completed;
            self.finished_at = Some(Utc::now());
            return false;
        }

        // Check fixed count (before max_iterations so it yields Completed, not Stopped)
        if let LoopKind::Fixed { count } = &self.def.kind
            && self.current_iteration >= *count
        {
            self.status = LoopStatus::Completed;
            self.finished_at = Some(Utc::now());
            return false;
        }

        // Check max iterations (safety valve)
        if self.current_iteration >= self.def.max_iterations {
            self.status = LoopStatus::Stopped;
            self.finished_at = Some(Utc::now());
            return false;
        }

        // Check poll timeout
        if let LoopKind::Poll { timeout_secs: Some(timeout), .. } = &self.def.kind
            && let Some(started) = self.started_at
        {
            let elapsed = (Utc::now() - started).num_seconds();
            if elapsed >= *timeout as i64 {
                self.status = LoopStatus::Stopped;
                self.finished_at = Some(Utc::now());
                return false;
            }
        }

        true
    }

    /// Check the break condition against output.
    pub fn check_break(&self, output: &str, exit_code: Option<i32>) -> bool {
        match &self.def.kind {
            LoopKind::Fixed { .. } => false, // fixed loops run to count, no condition
            LoopKind::Until { condition } => condition.check(output, exit_code),
            LoopKind::Poll { condition, .. } => condition.check(output, exit_code),
        }
    }

    /// Mark the loop as failed.
    ///
    /// # Tiger Style
    ///
    /// Asserts the loop is running — failing a non-running loop is a programmer error.
    pub fn fail(&mut self) {
        assert_eq!(self.status, LoopStatus::Running, "can only fail a running loop");
        assert!(self.finished_at.is_none());
        self.status = LoopStatus::Failed;
        self.finished_at = Some(Utc::now());
    }

    /// Mark the loop as explicitly stopped.
    ///
    /// # Tiger Style
    ///
    /// Asserts the loop is running — stopping a non-running loop is a programmer error.
    pub fn stop(&mut self) {
        assert_eq!(self.status, LoopStatus::Running, "can only stop a running loop");
        assert!(self.finished_at.is_none());
        self.status = LoopStatus::Stopped;
        self.finished_at = Some(Utc::now());
    }

    /// Total elapsed time since the loop started.
    pub fn elapsed_secs(&self) -> i64 {
        self.started_at
            .map(|s| (self.finished_at.unwrap_or_else(Utc::now) - s).num_seconds())
            .unwrap_or(0)
    }

    /// Summary string for display.
    pub fn summary(&self) -> String {
        format!(
            "{}: {} ({}/{} iterations, {:.0}s)",
            self.def.name,
            match self.status {
                LoopStatus::Pending => "pending",
                LoopStatus::Running => "running",
                LoopStatus::Completed => "completed",
                LoopStatus::Stopped => "stopped",
                LoopStatus::Failed => "failed",
            },
            self.current_iteration,
            self.def.max_iterations,
            self.elapsed_secs(),
        )
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn fixed_loop_runs_to_count() {
        let def = LoopDef::fixed("test", 3, json!({"cmd": "echo hi"}));
        let mut state = LoopState::new(def);
        state.start();

        for i in 0..3 {
            let result = IterationResult {
                index: i,
                output: format!("iteration {i}"),
                exit_code: Some(0),
                started_at: Utc::now(),
                finished_at: Utc::now(),
                break_matched: false,
            };
            let should_continue = state.record_iteration(result);
            if i < 2 {
                assert!(should_continue, "iteration {i} should continue");
            } else {
                assert!(!should_continue, "iteration {i} should stop");
            }
        }

        assert_eq!(state.status, LoopStatus::Completed);
        assert_eq!(state.current_iteration, 3);
    }

    #[test]
    fn until_loop_breaks_on_condition() {
        let def = LoopDef::until(
            "wait-for-pass",
            BreakCondition::Contains("PASS".into()),
            json!({"cmd": "cargo test"}),
        );
        let mut state = LoopState::new(def);
        state.start();

        // First iteration: FAIL
        let broke = state.check_break("tests FAIL", Some(1));
        assert!(!broke);
        state.record_iteration(IterationResult {
            index: 0,
            output: "tests FAIL".into(),
            exit_code: Some(1),
            started_at: Utc::now(),
            finished_at: Utc::now(),
            break_matched: false,
        });

        // Second iteration: PASS
        let broke = state.check_break("all tests PASS", Some(0));
        assert!(broke);
        let cont = state.record_iteration(IterationResult {
            index: 1,
            output: "all tests PASS".into(),
            exit_code: Some(0),
            started_at: Utc::now(),
            finished_at: Utc::now(),
            break_matched: true,
        });
        assert!(!cont);
        assert_eq!(state.status, LoopStatus::Completed);
    }

    #[test]
    fn max_iterations_stops_loop() {
        let def = LoopDef::until(
            "never-matches",
            BreakCondition::Contains("NEVER".into()),
            json!({}),
        )
        .with_max_iterations(5);

        let mut state = LoopState::new(def);
        state.start();

        for i in 0..5 {
            let cont = state.record_iteration(IterationResult {
                index: i,
                output: "nope".into(),
                exit_code: None,
                started_at: Utc::now(),
                finished_at: Utc::now(),
                break_matched: false,
            });
            if i < 4 {
                assert!(cont);
            } else {
                assert!(!cont);
            }
        }

        assert_eq!(state.status, LoopStatus::Stopped);
    }

    #[test]
    fn loop_state_serializes() {
        let def = LoopDef::fixed("ser-test", 10, json!({"cmd": "ls"}));
        let state = LoopState::new(def);
        let json = serde_json::to_string(&state).unwrap();
        let parsed: LoopState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.def.name, "ser-test");
        assert_eq!(parsed.status, LoopStatus::Pending);
    }

    #[test]
    fn summary_includes_status() {
        let def = LoopDef::fixed("summarize", 5, json!({}));
        let state = LoopState::new(def);
        let summary = state.summary();
        assert!(summary.contains("summarize"));
        assert!(summary.contains("pending"));
    }

    #[test]
    fn explicit_stop() {
        let def = LoopDef::fixed("stop-test", 10, json!({}));
        let mut state = LoopState::new(def);
        state.start();
        state.stop();
        assert_eq!(state.status, LoopStatus::Stopped);
        assert!(state.finished_at.is_some());
    }

    #[test]
    fn fail_marks_failed() {
        let def = LoopDef::fixed("fail-test", 10, json!({}));
        let mut state = LoopState::new(def);
        state.start();
        state.fail();
        assert_eq!(state.status, LoopStatus::Failed);
    }
}
