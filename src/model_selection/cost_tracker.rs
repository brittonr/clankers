//! Per-model cost tracking and budget enforcement
//!
//! Records token usage for each model, calculates cost from pricing tables,
//! and enforces soft/hard budget thresholds.

use std::collections::HashMap;
use std::path::Path;
use std::sync::RwLock;

use clankers_tui_types::CostProvider;
use serde::Deserialize;
use serde::Serialize;

// ── Pricing ─────────────────────────────────────────────────────────────────

/// Per-model pricing (cost per million tokens)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    /// Cost per million input tokens (USD)
    pub input_per_mtok: f64,
    /// Cost per million output tokens (USD)
    pub output_per_mtok: f64,
    /// Human-readable model name for display
    pub display_name: String,
}

/// Try loading a user-override `pricing.json` from `config_dir`.
fn try_load_user_pricing(config_dir: Option<&Path>) -> Option<HashMap<String, ModelPricing>> {
    let dir = config_dir?;
    let path = dir.join("pricing.json");
    if !path.exists() {
        return None;
    }
    match std::fs::read_to_string(&path)
        .ok()
        .and_then(|data| serde_json::from_str::<HashMap<String, ModelPricing>>(&data).ok())
    {
        Some(parsed) => {
            tracing::info!("Loaded custom pricing from {}", path.display());
            Some(parsed)
        }
        None => {
            tracing::warn!("Invalid pricing.json at {}, ignoring", path.display());
            None
        }
    }
}

/// Derive pricing from the provider's model registry.
///
/// This is the preferred way to build a pricing table — it reads
/// `input_cost_per_mtok` / `output_cost_per_mtok` from each
/// [`clankers_router::Model`], so prices stay in sync with the
/// model catalog automatically.  Models without pricing data are
/// skipped (tracked at $0).
///
/// An optional `config_dir` is checked first for a user-override
/// `pricing.json`; if present, that takes priority.
pub fn pricing_from_models(
    models: &[clankers_router::Model],
    config_dir: Option<&Path>,
) -> HashMap<String, ModelPricing> {
    if let Some(user) = try_load_user_pricing(config_dir) {
        return user;
    }

    models
        .iter()
        .filter_map(|m| {
            let input = m.input_cost_per_mtok?;
            let output = m.output_cost_per_mtok?;
            Some((m.id.clone(), ModelPricing {
                input_per_mtok: input,
                output_per_mtok: output,
                display_name: m.name.clone(),
            }))
        })
        .collect()
}

/// Load pricing from a user-override file only (no hardcoded defaults).
///
/// Prefer [`pricing_from_models`] when a provider is available.
/// This function is kept for test/headless contexts where no provider exists.
pub fn load_pricing(config_dir: Option<&Path>) -> HashMap<String, ModelPricing> {
    try_load_user_pricing(config_dir).unwrap_or_default()
}

// ── Configuration ───────────────────────────────────────────────────────────

/// Budget configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CostTrackerConfig {
    /// Soft budget limit — warn but don't enforce (USD)
    #[serde(default)]
    pub soft_limit: Option<f64>,
    /// Hard budget limit — downgrade to cheaper models (USD)
    #[serde(default)]
    pub hard_limit: Option<f64>,
    /// Warn at regular cost intervals (e.g., every $1.00)
    #[serde(default)]
    pub warning_interval: Option<f64>,
}

// ── Per-model usage ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
struct ModelUsage {
    input_tokens: u64,
    output_tokens: u64,
    total_turns: u64,
    cost_usd: f64,
}

// ── Budget status ───────────────────────────────────────────────────────────

// Cost display types re-exported from clankers-tui-types (canonical definitions).
pub use clankers_tui_types::BudgetStatus;
pub use clankers_tui_types::CostSummary;
pub use clankers_tui_types::ModelCostBreakdown;

// ── Budget event (returned, not sent directly) ──────────────────────────────

/// Budget events emitted after recording usage
#[derive(Debug, Clone, PartialEq)]
pub enum BudgetEvent {
    /// Soft budget threshold reached
    Warning { threshold: f64, current: f64 },
    /// Hard budget limit exceeded
    Exceeded { limit: f64, current: f64 },
    /// Cost milestone hit (e.g., every $1)
    Milestone { milestone: f64, total: f64 },
}

// ── CostTracker ─────────────────────────────────────────────────────────────

/// Tracks per-model token usage and cost, enforces budget thresholds.
///
/// Thread-safe via `RwLock`. Designed to be wrapped in `Arc` and shared
/// between the agent, routing policy, and TUI.
pub struct CostTracker {
    usage: RwLock<HashMap<String, ModelUsage>>,
    pricing: HashMap<String, ModelPricing>,
    config: CostTrackerConfig,
    /// Previous total before last record_usage — for milestone detection
    prev_total: RwLock<f64>,
}

impl std::fmt::Debug for CostTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CostTracker")
            .field("config", &self.config)
            .field("pricing_models", &self.pricing.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl CostTracker {
    /// Create a new cost tracker with the given pricing and config.
    pub fn new(pricing: HashMap<String, ModelPricing>, config: CostTrackerConfig) -> Self {
        Self {
            usage: RwLock::new(HashMap::new()),
            pricing,
            config,
            prev_total: RwLock::new(0.0),
        }
    }

    /// Create with no pricing data and no budget limits.
    ///
    /// Models will be tracked at $0 cost. In production, prefer creating
    /// via [`pricing_from_models`] to get real pricing from the model registry.
    pub fn with_defaults() -> Self {
        Self::new(HashMap::new(), CostTrackerConfig::default())
    }

    /// Record token usage from an API response.
    ///
    /// Returns any budget events triggered (warnings, exceeded, milestones).
    /// Unknown models are tracked at zero cost (no pricing entry found).
    pub fn record_usage(&self, model_id: &str, input_tokens: u64, output_tokens: u64) -> (f64, Vec<BudgetEvent>) {
        let (input_cost, output_cost) = if let Some(pricing) = self.pricing.get(model_id) {
            (
                (input_tokens as f64 / 1_000_000.0) * pricing.input_per_mtok,
                (output_tokens as f64 / 1_000_000.0) * pricing.output_per_mtok,
            )
        } else {
            // Try prefix matching (e.g., "claude-sonnet-4-5-20250514" matches "claude-sonnet-4-5")
            let fallback = self.pricing.iter().find(|(k, _)| model_id.starts_with(k.as_str()));
            if let Some((_, pricing)) = fallback {
                (
                    (input_tokens as f64 / 1_000_000.0) * pricing.input_per_mtok,
                    (output_tokens as f64 / 1_000_000.0) * pricing.output_per_mtok,
                )
            } else {
                tracing::debug!("No pricing for model '{}', tracking at $0", model_id);
                (0.0, 0.0)
            }
        };
        let turn_cost = input_cost + output_cost;

        let total_cost = {
            let mut usage = self.usage.write().expect("usage lock not poisoned");
            let entry = usage.entry(model_id.to_string()).or_default();
            entry.input_tokens += input_tokens;
            entry.output_tokens += output_tokens;
            entry.total_turns += 1;
            entry.cost_usd += turn_cost;
            usage.values().map(|u| u.cost_usd).sum::<f64>()
        };

        let events = self.check_thresholds(total_cost);
        (total_cost, events)
    }

    /// Get total cost across all models.
    pub fn total_cost(&self) -> f64 {
        let usage = self.usage.read().expect("usage lock not poisoned");
        usage.values().map(|u| u.cost_usd).sum()
    }

    /// Get current budget status.
    pub fn budget_status(&self) -> BudgetStatus {
        self.compute_budget_status(self.total_cost())
    }

    /// Generate a full cost summary.
    pub fn summary(&self) -> CostSummary {
        let usage = self.usage.read().expect("usage lock not poisoned");
        let total_cost: f64 = usage.values().map(|u| u.cost_usd).sum();

        let by_model: Vec<ModelCostBreakdown> = usage
            .iter()
            .map(|(model_id, u)| {
                let display_name =
                    self.pricing.get(model_id).map(|p| p.display_name.clone()).unwrap_or_else(|| model_id.clone());
                let percentage = if total_cost > 0.0 {
                    (u.cost_usd / total_cost * 100.0) as f32
                } else {
                    0.0
                };
                ModelCostBreakdown {
                    model_id: model_id.clone(),
                    display_name,
                    input_tokens: u.input_tokens,
                    output_tokens: u.output_tokens,
                    cost_usd: u.cost_usd,
                    percentage,
                }
            })
            .collect();

        let most_expensive = by_model
            .iter()
            .max_by(|a, b| a.cost_usd.partial_cmp(&b.cost_usd).unwrap_or(std::cmp::Ordering::Equal))
            .map(|m| m.model_id.clone());

        CostSummary {
            total_cost,
            by_model,
            budget_status: self.compute_budget_status(total_cost),
            most_expensive,
        }
    }

    /// Format a one-line cost string for the status bar.
    pub fn status_line(&self, current_model: &str) -> String {
        let usage = self.usage.read().expect("usage lock not poisoned");
        let total_cost: f64 = usage.values().map(|u| u.cost_usd).sum();
        let total_in: u64 = usage.values().map(|u| u.input_tokens).sum();
        let total_out: u64 = usage.values().map(|u| u.output_tokens).sum();

        let model_short = self.pricing.get(current_model).map(|p| p.display_name.as_str()).unwrap_or(current_model);

        let budget_part = match (&self.config.soft_limit, &self.config.hard_limit) {
            (_, Some(hard)) => format!(" | Budget: ${:.2} / ${:.2}", total_cost, hard),
            (Some(soft), None) => format!(" | Budget: ${:.2} / ${:.2}", total_cost, soft),
            _ => String::new(),
        };

        format!(
            "[{}] {}k in / {}k out | ${:.3}{}",
            model_short,
            total_in / 1000,
            total_out / 1000,
            total_cost,
            budget_part,
        )
    }

    // ── Internal ────────────────────────────────────────────────────────────

    fn check_thresholds(&self, total: f64) -> Vec<BudgetEvent> {
        let mut events = Vec::new();
        let prev = {
            let mut prev = self.prev_total.write().expect("prev_total lock not poisoned");
            let old = *prev;
            *prev = total;
            old
        };

        // Hard limit
        if let Some(hard) = self.config.hard_limit
            && total >= hard
            && prev < hard
        {
            events.push(BudgetEvent::Exceeded {
                limit: hard,
                current: total,
            });
        }

        // Soft limit
        if let Some(soft) = self.config.soft_limit
            && total >= soft
            && prev < soft
        {
            events.push(BudgetEvent::Warning {
                threshold: soft,
                current: total,
            });
        }

        // Milestone intervals — emit one event per crossed milestone
        if let Some(interval) = self.config.warning_interval
            && interval > 0.0
        {
            let prev_milestone = (prev / interval).floor() as u64;
            let curr_milestone = (total / interval).floor() as u64;
            for m in (prev_milestone + 1)..=curr_milestone {
                events.push(BudgetEvent::Milestone {
                    milestone: m as f64 * interval,
                    total,
                });
            }
        }

        events
    }

    fn compute_budget_status(&self, total: f64) -> BudgetStatus {
        match (self.config.soft_limit, self.config.hard_limit) {
            (None, None) => BudgetStatus::NoBudget,
            (Some(soft), None) => {
                if total < soft {
                    BudgetStatus::Ok {
                        remaining: soft - total,
                    }
                } else {
                    BudgetStatus::Warning {
                        over_soft_by: total - soft,
                        hard_limit_remaining: f64::INFINITY,
                    }
                }
            }
            (Some(soft), Some(hard)) => {
                if total < soft {
                    BudgetStatus::Ok {
                        remaining: soft - total,
                    }
                } else if total < hard {
                    BudgetStatus::Warning {
                        over_soft_by: total - soft,
                        hard_limit_remaining: hard - total,
                    }
                } else {
                    BudgetStatus::Exceeded {
                        over_hard_by: total - hard,
                    }
                }
            }
            (None, Some(hard)) => {
                if total < hard {
                    BudgetStatus::Ok {
                        remaining: hard - total,
                    }
                } else {
                    BudgetStatus::Exceeded {
                        over_hard_by: total - hard,
                    }
                }
            }
        }
    }
}

impl CostProvider for CostTracker {
    fn summary(&self) -> CostSummary {
        self.summary()
    }

    fn budget_status(&self) -> BudgetStatus {
        self.budget_status()
    }

    fn total_cost(&self) -> f64 {
        self.total_cost()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Test pricing table — covers the models used in unit tests.
    fn test_pricing() -> HashMap<String, ModelPricing> {
        [
            ("claude-opus-4", 15.0, 75.0, "Claude Opus 4"),
            ("claude-sonnet-4-5", 3.0, 15.0, "Claude Sonnet 4.5"),
            ("claude-sonnet-4", 3.0, 15.0, "Claude Sonnet 4"),
            ("claude-haiku-4", 1.0, 5.0, "Claude Haiku 4"),
        ]
        .into_iter()
        .map(|(id, input, output, name)| {
            (id.to_string(), ModelPricing {
                input_per_mtok: input,
                output_per_mtok: output,
                display_name: name.to_string(),
            })
        })
        .collect()
    }

    fn test_tracker() -> CostTracker {
        CostTracker::new(test_pricing(), CostTrackerConfig::default())
    }

    fn tracker_with_budget(soft: Option<f64>, hard: Option<f64>) -> CostTracker {
        CostTracker::new(test_pricing(), CostTrackerConfig {
            soft_limit: soft,
            hard_limit: hard,
            warning_interval: None,
        })
    }

    #[test]
    fn test_pricing_from_models() {
        let models = vec![
            clankers_router::Model {
                id: "test-model".to_string(),
                name: "Test Model".to_string(),
                provider: "test".to_string(),
                max_input_tokens: 100_000,
                max_output_tokens: 4_096,
                supports_thinking: false,
                supports_images: false,
                supports_tools: true,
                input_cost_per_mtok: Some(5.0),
                output_cost_per_mtok: Some(25.0),
            },
            clankers_router::Model {
                id: "free-model".to_string(),
                name: "Free Model".to_string(),
                provider: "test".to_string(),
                max_input_tokens: 100_000,
                max_output_tokens: 4_096,
                supports_thinking: false,
                supports_images: false,
                supports_tools: true,
                input_cost_per_mtok: None,
                output_cost_per_mtok: None,
            },
        ];
        let pricing = pricing_from_models(&models, None);
        assert_eq!(pricing.len(), 1, "model without pricing should be skipped");
        assert!(pricing.contains_key("test-model"));
        assert_eq!(pricing["test-model"].input_per_mtok, 5.0);
        assert_eq!(pricing["test-model"].display_name, "Test Model");
    }

    #[test]
    fn test_record_usage_basic() {
        let tracker = test_tracker();
        let (total, events) = tracker.record_usage("claude-sonnet-4-5", 1_000_000, 500_000);

        // 1M input * $3/MTok + 500K output * $15/MTok = $3 + $7.50 = $10.50
        assert!((total - 10.5).abs() < 0.01);
        assert!(events.is_empty()); // no budget limits set
    }

    #[test]
    fn test_record_usage_accumulates() {
        let tracker = test_tracker();
        tracker.record_usage("claude-sonnet-4-5", 100_000, 50_000);
        tracker.record_usage("claude-sonnet-4-5", 200_000, 100_000);

        let summary = tracker.summary();
        assert_eq!(summary.by_model.len(), 1);
        assert_eq!(summary.by_model[0].input_tokens, 300_000);
        assert_eq!(summary.by_model[0].output_tokens, 150_000);
    }

    #[test]
    fn test_record_usage_multiple_models() {
        let tracker = test_tracker();
        tracker.record_usage("claude-sonnet-4-5", 100_000, 50_000);
        tracker.record_usage("claude-haiku-4", 200_000, 100_000);

        let summary = tracker.summary();
        assert_eq!(summary.by_model.len(), 2);
        assert!(summary.total_cost > 0.0);
    }

    #[test]
    fn test_unknown_model_tracked_at_zero() {
        let tracker = test_tracker();
        let (total, _) = tracker.record_usage("gpt-4o", 1_000_000, 500_000);
        assert_eq!(total, 0.0);
    }

    #[test]
    fn test_cost_calculation_accuracy() {
        let tracker = test_tracker();

        // Opus: 10K input * $15/MTok + 5K output * $75/MTok
        // = $0.15 + $0.375 = $0.525
        let (total, _) = tracker.record_usage("claude-opus-4", 10_000, 5_000);
        assert!((total - 0.525).abs() < 0.001);

        // Haiku: 10K input * $1/MTok + 5K output * $5/MTok
        // = $0.01 + $0.025 = $0.035
        let (total, _) = tracker.record_usage("claude-haiku-4", 10_000, 5_000);
        // cumulative: $0.525 + $0.035 = $0.56
        assert!((total - 0.56).abs() < 0.001);
    }

    #[test]
    fn test_budget_status_no_budget() {
        let tracker = test_tracker();
        assert_eq!(tracker.budget_status(), BudgetStatus::NoBudget);
    }

    #[test]
    fn test_budget_status_ok() {
        let tracker = tracker_with_budget(Some(10.0), Some(20.0));
        tracker.record_usage("claude-haiku-4", 10_000, 5_000); // ~$0.035
        assert!(matches!(tracker.budget_status(), BudgetStatus::Ok { .. }));
    }

    #[test]
    fn test_budget_warning_event() {
        let tracker = tracker_with_budget(Some(0.01), None);
        // Record enough to exceed $0.01 soft limit
        let (_, events) = tracker.record_usage("claude-sonnet-4-5", 100_000, 50_000);
        // $0.30 + $0.75 = $1.05 — well over $0.01
        assert!(events.iter().any(|e| matches!(e, BudgetEvent::Warning { .. })));
    }

    #[test]
    fn test_budget_exceeded_event() {
        let tracker = tracker_with_budget(None, Some(0.01));
        let (_, events) = tracker.record_usage("claude-sonnet-4-5", 100_000, 50_000);
        assert!(events.iter().any(|e| matches!(e, BudgetEvent::Exceeded { .. })));
    }

    #[test]
    fn test_budget_milestone_event() {
        let tracker = CostTracker::new(test_pricing(), CostTrackerConfig {
            soft_limit: None,
            hard_limit: None,
            warning_interval: Some(1.0), // every $1
        });
        // Record usage that crosses $1 milestone
        // Sonnet: 1M input = $3.0
        let (_, events) = tracker.record_usage("claude-sonnet-4-5", 1_000_000, 0);
        // Should have milestones at $1, $2, $3
        let milestones: Vec<_> = events
            .iter()
            .filter_map(|e| {
                if let BudgetEvent::Milestone { milestone, .. } = e {
                    Some(*milestone)
                } else {
                    None
                }
            })
            .collect();
        assert!(milestones.contains(&1.0));
        assert!(milestones.contains(&2.0));
        assert!(milestones.contains(&3.0));
    }

    #[test]
    fn test_budget_warning_fires_once() {
        let tracker = tracker_with_budget(Some(0.01), None);
        let (_, events1) = tracker.record_usage("claude-sonnet-4-5", 100_000, 0);
        let (_, events2) = tracker.record_usage("claude-sonnet-4-5", 100_000, 0);

        // First call crosses the threshold → warning
        assert!(events1.iter().any(|e| matches!(e, BudgetEvent::Warning { .. })));
        // Second call is already over → no duplicate warning
        assert!(!events2.iter().any(|e| matches!(e, BudgetEvent::Warning { .. })));
    }

    #[test]
    fn test_summary() {
        let tracker = tracker_with_budget(Some(5.0), Some(10.0));
        tracker.record_usage("claude-sonnet-4-5", 100_000, 50_000);
        tracker.record_usage("claude-opus-4", 10_000, 5_000);

        let summary = tracker.summary();
        assert_eq!(summary.by_model.len(), 2);
        assert!(summary.total_cost > 0.0);
        assert!(summary.most_expensive.is_some());

        // Percentages should sum to ~100%
        let pct_sum: f32 = summary.by_model.iter().map(|m| m.percentage).sum();
        assert!((pct_sum - 100.0).abs() < 0.1);
    }

    #[test]
    fn test_status_line() {
        let tracker = test_tracker();
        tracker.record_usage("claude-sonnet-4-5", 100_000, 50_000);
        let line = tracker.status_line("claude-sonnet-4-5");
        assert!(line.contains("Sonnet"));
        assert!(line.contains("$"));
    }

    #[test]
    fn test_budget_status_transitions() {
        let tracker = tracker_with_budget(Some(1.0), Some(5.0));

        // Under soft
        tracker.record_usage("claude-haiku-4", 10_000, 5_000); // ~$0.035
        assert!(matches!(tracker.budget_status(), BudgetStatus::Ok { .. }));

        // Over soft, under hard — sonnet 1M input = $3
        tracker.record_usage("claude-sonnet-4-5", 1_000_000, 0);
        assert!(matches!(tracker.budget_status(), BudgetStatus::Warning { .. }));

        // Over hard — opus 1M input = $15
        tracker.record_usage("claude-opus-4", 1_000_000, 0);
        assert!(matches!(tracker.budget_status(), BudgetStatus::Exceeded { .. }));
    }

    #[test]
    fn test_prefix_matching_for_dated_models() {
        let tracker = test_tracker();
        // Dated model ID should match a prefix in pricing
        let (total, _) = tracker.record_usage("claude-sonnet-4-5-20250514", 1_000_000, 0);
        assert!(total > 0.0, "Prefix-matched model should have non-zero cost");
    }

    #[test]
    fn test_config_serialization() {
        let config = CostTrackerConfig {
            soft_limit: Some(5.0),
            hard_limit: Some(10.0),
            warning_interval: Some(1.0),
        };
        let json = serde_json::to_string(&config).expect("config should serialize to JSON");
        let decoded: CostTrackerConfig = serde_json::from_str(&json).expect("JSON should deserialize to config");
        assert_eq!(decoded.soft_limit, config.soft_limit);
        assert_eq!(decoded.hard_limit, config.hard_limit);
        assert_eq!(decoded.warning_interval, config.warning_interval);
    }

    #[test]
    fn test_load_pricing_no_config_returns_empty() {
        let pricing = load_pricing(None);
        assert!(pricing.is_empty(), "load_pricing with no config dir should return empty map");
    }
}
