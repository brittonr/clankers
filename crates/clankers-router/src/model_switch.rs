//! Model switch tracking — audit trail for runtime model changes
//!
//! Tracks when and why the active model was changed, providing:
//!
//! - **History** — Ordered log of all model switches with timestamps
//! - **Reason tagging** — Each switch is annotated with why it happened
//!   (user request, rate-limit fallback, role routing, etc.)
//! - **Undo** — Revert to the previous model with `switch_back()`
//! - **Statistics** — How often each model has been used, time spent per model
//!
//! The tracker is embedded in the [`Router`](crate::Router) and also usable standalone.

use std::collections::HashMap;
use std::time::Instant;

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

// ── Switch reason ───────────────────────────────────────────────────────

/// Why a model switch occurred.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ModelSwitchReason {
    /// User explicitly requested the switch (e.g. `/model gpt-4o`).
    UserRequest,
    /// Automatic fallback because the previous model was rate-limited or errored.
    RateLimitFallback,
    /// Role-based routing selected a different model (e.g. `smol` for grep tasks).
    RoleSwitch { role: String },
    /// Multi-model strategy elected this model as the winner.
    MultiModelWinner,
    /// Configuration change (settings file reload, API key added, etc.).
    ConfigChange,
    /// Session resume restored a previously-active model.
    SessionRestore,
    /// Initial model set at startup (no previous model).
    Initial,
}

impl std::fmt::Display for ModelSwitchReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelSwitchReason::UserRequest => write!(f, "user request"),
            ModelSwitchReason::RateLimitFallback => write!(f, "rate-limit fallback"),
            ModelSwitchReason::RoleSwitch { role } => write!(f, "role:{role}"),
            ModelSwitchReason::MultiModelWinner => write!(f, "multi-model winner"),
            ModelSwitchReason::ConfigChange => write!(f, "config change"),
            ModelSwitchReason::SessionRestore => write!(f, "session restore"),
            ModelSwitchReason::Initial => write!(f, "initial"),
        }
    }
}

// ── Switch record ───────────────────────────────────────────────────────

/// A single model switch event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSwitchRecord {
    /// Previous model ID (empty string for the initial switch).
    pub from: String,
    /// New model ID.
    pub to: String,
    /// Why the switch happened.
    pub reason: ModelSwitchReason,
    /// When it happened.
    pub timestamp: DateTime<Utc>,
}

// ── Per-model usage stats ───────────────────────────────────────────────

/// Accumulated usage statistics for a single model.
#[derive(Debug, Clone, Default)]
pub struct ModelUsageStats {
    /// Number of times this model was switched to.
    pub switch_count: u64,
    /// Number of requests completed while this model was active.
    pub request_count: u64,
    /// Total wall-clock milliseconds this model was the active model.
    pub active_ms: u64,
}

// ── Tracker ─────────────────────────────────────────────────────────────

/// Tracks the active model, switch history, and per-model usage stats.
#[derive(Debug)]
pub struct ModelSwitchTracker {
    /// Currently active model ID.
    current: String,
    /// When the current model became active (monotonic, for duration tracking).
    current_since: Instant,
    /// Ordered history of all switches.
    history: Vec<ModelSwitchRecord>,
    /// Per-model accumulated stats.
    stats: HashMap<String, ModelUsageStats>,
    /// Maximum history entries to keep (0 = unlimited).
    max_history: usize,
}

impl ModelSwitchTracker {
    /// Create a new tracker with the given initial model.
    pub fn new(initial_model: impl Into<String>) -> Self {
        let model = initial_model.into();
        let now = Instant::now();

        let initial_record = ModelSwitchRecord {
            from: String::new(),
            to: model.clone(),
            reason: ModelSwitchReason::Initial,
            timestamp: Utc::now(),
        };

        let mut stats = HashMap::new();
        stats.insert(model.clone(), ModelUsageStats {
            switch_count: 1,
            ..Default::default()
        });

        Self {
            current: model,
            current_since: now,
            history: vec![initial_record],
            stats,
            max_history: 1000,
        }
    }

    /// Set the maximum number of history entries to keep.
    /// Older entries are dropped when the limit is exceeded.
    pub fn set_max_history(&mut self, max: usize) {
        self.max_history = max;
        self.trim_history();
    }

    /// Get the currently active model.
    pub fn current_model(&self) -> &str {
        &self.current
    }

    /// Switch to a new model with a reason. Returns the previous model ID.
    ///
    /// If `new_model` is the same as the current model, this is a no-op
    /// and returns `None`.
    pub fn switch(&mut self, new_model: impl Into<String>, reason: ModelSwitchReason) -> Option<String> {
        let new_model = new_model.into();
        if new_model == self.current {
            return None;
        }

        let old = std::mem::replace(&mut self.current, new_model.clone());

        // Accumulate active time for the outgoing model
        let elapsed = self.current_since.elapsed().as_millis() as u64;
        self.stats.entry(old.clone()).or_default().active_ms += elapsed;
        self.current_since = Instant::now();

        // Record the switch
        let record = ModelSwitchRecord {
            from: old.clone(),
            to: new_model.clone(),
            reason,
            timestamp: Utc::now(),
        };
        self.history.push(record);
        self.trim_history();

        // Update stats for the new model
        self.stats.entry(new_model).or_default().switch_count += 1;

        Some(old)
    }

    /// Switch back to the previous model. Returns `None` if there's no
    /// previous model to switch back to.
    pub fn switch_back(&mut self) -> Option<String> {
        // The most recent record's `from` is the model we were on before now.
        let prev = self
            .history
            .last()
            .filter(|r| !r.from.is_empty()) // guard against the initial record
            .map(|r| r.from.clone())?;

        self.switch(prev, ModelSwitchReason::UserRequest)
    }

    /// Record that a request was completed on the current model.
    pub fn record_request(&mut self) {
        self.stats.entry(self.current.clone()).or_default().request_count += 1;
    }

    /// Get the full switch history (oldest first).
    pub fn history(&self) -> &[ModelSwitchRecord] {
        &self.history
    }

    /// Get the last `n` switches.
    pub fn recent_history(&self, n: usize) -> &[ModelSwitchRecord] {
        let start = self.history.len().saturating_sub(n);
        &self.history[start..]
    }

    /// Get per-model usage statistics.
    pub fn stats(&self) -> &HashMap<String, ModelUsageStats> {
        &self.stats
    }

    /// Get stats for a specific model.
    pub fn stats_for(&self, model: &str) -> Option<&ModelUsageStats> {
        self.stats.get(model)
    }

    /// How many times the model has been switched in total.
    pub fn total_switches(&self) -> usize {
        // Subtract 1 for the initial "switch"
        self.history.len().saturating_sub(1)
    }

    /// How long the current model has been active (since last switch).
    pub fn current_active_duration(&self) -> std::time::Duration {
        self.current_since.elapsed()
    }

    /// List all models that have been used, sorted by request count (descending).
    pub fn models_by_usage(&self) -> Vec<(&str, &ModelUsageStats)> {
        let mut entries: Vec<_> = self.stats.iter().map(|(k, v)| (k.as_str(), v)).collect();
        entries.sort_by_key(|e| std::cmp::Reverse(e.1.request_count));
        entries
    }

    /// Trim history to max_history entries.
    fn trim_history(&mut self) {
        if self.max_history > 0 && self.history.len() > self.max_history {
            let drain_count = self.history.len() - self.max_history;
            self.history.drain(..drain_count);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let tracker = ModelSwitchTracker::new("claude-sonnet");
        assert_eq!(tracker.current_model(), "claude-sonnet");
        assert_eq!(tracker.total_switches(), 0);
        assert_eq!(tracker.history().len(), 1); // initial record
        assert_eq!(tracker.history()[0].reason, ModelSwitchReason::Initial);
    }

    #[test]
    fn test_switch_model() {
        let mut tracker = ModelSwitchTracker::new("claude-sonnet");

        let old = tracker.switch("gpt-4o", ModelSwitchReason::UserRequest);
        assert_eq!(old, Some("claude-sonnet".to_string()));
        assert_eq!(tracker.current_model(), "gpt-4o");
        assert_eq!(tracker.total_switches(), 1);
    }

    #[test]
    fn test_switch_same_model_is_noop() {
        let mut tracker = ModelSwitchTracker::new("claude-sonnet");
        let old = tracker.switch("claude-sonnet", ModelSwitchReason::UserRequest);
        assert!(old.is_none());
        assert_eq!(tracker.total_switches(), 0);
    }

    #[test]
    fn test_switch_back() {
        let mut tracker = ModelSwitchTracker::new("claude-sonnet");
        tracker.switch("gpt-4o", ModelSwitchReason::UserRequest);
        tracker.switch("deepseek-chat", ModelSwitchReason::RateLimitFallback);

        let old = tracker.switch_back();
        assert_eq!(old, Some("deepseek-chat".to_string()));
        assert_eq!(tracker.current_model(), "gpt-4o");
    }

    #[test]
    fn test_switch_back_no_history() {
        let mut tracker = ModelSwitchTracker::new("claude-sonnet");
        assert!(tracker.switch_back().is_none());
    }

    #[test]
    fn test_record_request() {
        let mut tracker = ModelSwitchTracker::new("claude-sonnet");
        tracker.record_request();
        tracker.record_request();

        let stats = tracker.stats_for("claude-sonnet").unwrap();
        assert_eq!(stats.request_count, 2);
        assert_eq!(stats.switch_count, 1); // initial
    }

    #[test]
    fn test_stats_across_switches() {
        let mut tracker = ModelSwitchTracker::new("model-a");
        tracker.record_request();
        tracker.record_request();
        tracker.switch("model-b", ModelSwitchReason::UserRequest);
        tracker.record_request();
        tracker.switch("model-a", ModelSwitchReason::UserRequest);
        tracker.record_request();

        let stats_a = tracker.stats_for("model-a").unwrap();
        assert_eq!(stats_a.request_count, 3);
        assert_eq!(stats_a.switch_count, 2); // initial + back

        let stats_b = tracker.stats_for("model-b").unwrap();
        assert_eq!(stats_b.request_count, 1);
        assert_eq!(stats_b.switch_count, 1);
    }

    #[test]
    fn test_recent_history() {
        let mut tracker = ModelSwitchTracker::new("model-a");
        tracker.switch("model-b", ModelSwitchReason::UserRequest);
        tracker.switch("model-c", ModelSwitchReason::RateLimitFallback);
        tracker.switch("model-d", ModelSwitchReason::RoleSwitch {
            role: "smol".into(),
        });

        let recent = tracker.recent_history(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].to, "model-c");
        assert_eq!(recent[1].to, "model-d");
    }

    #[test]
    fn test_max_history_trim() {
        let mut tracker = ModelSwitchTracker::new("model-0");
        tracker.set_max_history(5);

        for i in 1..=10 {
            tracker.switch(format!("model-{i}"), ModelSwitchReason::UserRequest);
        }

        assert!(tracker.history().len() <= 5);
        assert_eq!(tracker.current_model(), "model-10");
    }

    #[test]
    fn test_models_by_usage() {
        let mut tracker = ModelSwitchTracker::new("model-a");
        tracker.record_request();
        tracker.record_request();
        tracker.record_request();
        tracker.switch("model-b", ModelSwitchReason::UserRequest);
        tracker.record_request();

        let by_usage = tracker.models_by_usage();
        assert_eq!(by_usage[0].0, "model-a"); // 3 requests
        assert_eq!(by_usage[1].0, "model-b"); // 1 request
    }

    #[test]
    fn test_reason_display() {
        assert_eq!(ModelSwitchReason::UserRequest.to_string(), "user request");
        assert_eq!(ModelSwitchReason::RateLimitFallback.to_string(), "rate-limit fallback");
        assert_eq!(
            ModelSwitchReason::RoleSwitch {
                role: "smol".into()
            }
            .to_string(),
            "role:smol"
        );
        assert_eq!(ModelSwitchReason::MultiModelWinner.to_string(), "multi-model winner");
    }
}
