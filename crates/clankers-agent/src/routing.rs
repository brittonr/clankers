//! Agent-owned routing and cost port contracts.
//!
//! Concrete routing/cost implementations live at app edges; the reusable agent
//! only owns the neutral inputs and outputs it needs for turn selection and
//! usage accounting.

use clanker_message::BudgetEvent;
use clanker_message::Usage;

/// Recent tool usage signal passed to routing implementations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRoutingToolUse {
    pub tool_name: String,
}

/// Signals the agent can provide to a routing implementation.
#[derive(Debug, Clone)]
pub struct AgentRoutingSignals {
    pub token_count: usize,
    pub recent_tools: Vec<AgentRoutingToolUse>,
    pub current_cost: f64,
    pub prompt_text: Option<String>,
}

/// Model-selection result returned to the agent.
#[derive(Debug, Clone)]
pub struct AgentModelSelection {
    pub role: String,
    pub reason: String,
    pub orchestration: Option<AgentOrchestrationPlan>,
}

/// Neutral orchestration plan used by the agent runtime.
#[derive(Debug, Clone)]
pub struct AgentOrchestrationPlan {
    pub pattern: String,
    pub phases: Vec<AgentOrchestrationPhase>,
}

/// One phase of an orchestrated turn.
#[derive(Debug, Clone)]
pub struct AgentOrchestrationPhase {
    pub role: String,
    pub label: String,
    pub system_suffix: String,
}

/// Routing policy contract injected by app-edge adapters.
pub trait AgentRoutingPolicy: Send + Sync {
    fn select_model(&self, signals: &AgentRoutingSignals) -> AgentModelSelection;
}

/// Cost recorder contract injected by app-edge adapters.
pub trait AgentCostRecorder: Send + Sync {
    fn record_usage(&self, model_id: &str, input_tokens: u64, output_tokens: u64) -> (f64, Vec<BudgetEvent>);
    fn total_cost(&self) -> f64;
}

/// Cost recorder that ignores all usage; used when cost tracking is disabled.
#[derive(Debug, Default)]
pub struct NoopCostRecorder;

impl AgentCostRecorder for NoopCostRecorder {
    fn record_usage(&self, _model_id: &str, _input_tokens: u64, _output_tokens: u64) -> (f64, Vec<BudgetEvent>) {
        (0.0, Vec::new())
    }

    fn total_cost(&self) -> f64 {
        0.0
    }
}

/// Record one turn with an optional cost recorder.
pub(crate) fn record_turn_cost(
    recorder: Option<&dyn AgentCostRecorder>,
    active_model: &str,
    turn_usage: &Usage,
) -> Option<f64> {
    let recorder = recorder?;
    let (total_cost, budget_events) = recorder.record_usage(
        active_model,
        turn_usage.input_tokens as u64,
        turn_usage.output_tokens as u64,
    );

    for event in budget_events {
        match event {
            BudgetEvent::Warning { threshold, current } => {
                tracing::warn!("Budget warning: ${:.2} spent (soft limit: ${:.2})", current, threshold);
            }
            BudgetEvent::Exceeded { limit, current } => {
                tracing::warn!("Budget exceeded: ${:.2} spent (hard limit: ${:.2})", current, limit);
            }
            BudgetEvent::Milestone { milestone, total: _ } => {
                tracing::info!("Cost milestone: ${:.2}", milestone);
            }
        }
    }

    tracing::debug!(
        "Turn cost recorded: model={}, in={}, out={}, total=${:.4}",
        active_model,
        turn_usage.input_tokens,
        turn_usage.output_tokens,
        total_cost,
    );

    Some(total_cost)
}
