//! Multi-model routing policy
//!
//! Dynamically selects the appropriate model role based on task complexity
//! signals: token count, tool usage, keyword hints, and user preferences.
#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", feature(register_tool), register_tool(tigerstyle))]
#![cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        tigerstyle::assertion_density,
        tigerstyle::multi_lock_ordering,
        tigerstyle::explicit_defaults,
        tigerstyle::raw_arithmetic_overflow,
        reason = "model selection preserves public cost/routing policy shapes; focused tests cover routing behavior during Tigerstyle drain"
    )
)]

pub mod config;
pub mod cost_tracker;
pub mod orchestration;
pub mod policy;
pub mod signals;

pub use config::RoutingPolicyConfig;
pub use cost_tracker::BudgetEvent;
pub use cost_tracker::BudgetStatus;
pub use cost_tracker::CostMicros;
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
