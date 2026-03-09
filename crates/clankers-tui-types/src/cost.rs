//! Cost tracking display types.

/// Current budget status.
#[derive(Debug, Clone, PartialEq)]
pub enum BudgetStatus {
    /// No budget configured.
    NoBudget,
    /// Under all limits.
    Ok { remaining: f64 },
    /// Over soft limit, under hard limit.
    Warning {
        over_soft_by: f64,
        hard_limit_remaining: f64,
    },
    /// Over hard limit.
    Exceeded { over_hard_by: f64 },
}

/// Aggregate cost summary for display.
#[derive(Debug, Clone)]
pub struct CostSummary {
    /// Total cost across all models (USD).
    pub total_cost: f64,
    /// Per-model breakdown.
    pub by_model: Vec<ModelCostBreakdown>,
    /// Current budget status.
    pub budget_status: BudgetStatus,
    /// Most expensive model this session.
    pub most_expensive: Option<String>,
}

/// Cost breakdown for a single model.
#[derive(Debug, Clone)]
pub struct ModelCostBreakdown {
    pub model_id: String,
    pub display_name: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
    /// Percentage of total cost.
    pub percentage: f32,
}

/// Trait for accessing cost data from the TUI without depending on `CostTracker`.
pub trait CostProvider: Send + Sync {
    /// Full cost summary with per-model breakdown.
    fn summary(&self) -> CostSummary;
    /// Current budget status (for status bar badge).
    fn budget_status(&self) -> BudgetStatus;
    /// Total cost in USD across all models.
    fn total_cost(&self) -> f64;
}
