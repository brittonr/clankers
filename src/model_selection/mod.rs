//! Multi-model routing policy
//!
//! Dynamically selects the appropriate model role based on task complexity
//! signals: token count, tool usage, keyword hints, and user preferences.

pub mod config;
pub mod cost_tracker;
pub mod orchestration;
pub mod policy;
pub mod signals;

#[cfg(test)]
mod integration_tests;

pub use config::RoutingPolicyConfig;
pub use cost_tracker::BudgetEvent;
pub use cost_tracker::BudgetStatus;
pub use cost_tracker::CostSummary;
pub use cost_tracker::CostTracker;
pub use cost_tracker::CostTrackerConfig;
pub use policy::ModelSelectionResult;
pub use policy::RoutingPolicy;
pub use policy::SelectionReason;
pub use signals::ComplexitySignals;
pub use signals::ModelRoleHint;
pub use signals::ToolCallSummary;
pub use signals::ToolComplexity;
