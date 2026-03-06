//! Multi-model routing policy
//!
//! Dynamically selects the appropriate model role based on task complexity
//! signals: token count, tool usage, keyword hints, and user preferences.

pub mod config;
pub mod cost_tracker;
pub mod orchestration;
pub mod policy;
pub mod signals;

pub use config::RoutingPolicyConfig;
pub use cost_tracker::{BudgetEvent, BudgetStatus, CostSummary, CostTracker, CostTrackerConfig};
pub use policy::{ModelSelectionResult, RoutingPolicy, SelectionReason};
pub use signals::{ComplexitySignals, ModelRoleHint, ToolCallSummary, ToolComplexity};
