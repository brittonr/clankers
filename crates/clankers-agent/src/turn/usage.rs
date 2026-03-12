//! Token usage tracking and cost calculation

use clankers_model_selection::cost_tracker::CostTracker;
use clankers_provider::Usage;
use tokio::sync::broadcast;

use crate::events::AgentEvent;

/// Update usage tracking and emit events
pub(super) fn update_usage_tracking(
    cumulative_usage: &mut Usage,
    turn_usage: &Usage,
    active_model: &str,
    cost_tracker: Option<&std::sync::Arc<CostTracker>>,
    event_tx: &broadcast::Sender<AgentEvent>,
) {
    cumulative_usage.input_tokens += turn_usage.input_tokens;
    cumulative_usage.output_tokens += turn_usage.output_tokens;
    cumulative_usage.cache_creation_input_tokens += turn_usage.cache_creation_input_tokens;
    cumulative_usage.cache_read_input_tokens += turn_usage.cache_read_input_tokens;

    let _ = event_tx.send(AgentEvent::UsageUpdate {
        turn_usage: turn_usage.clone(),
        cumulative_usage: cumulative_usage.clone(),
    });

    if let Some(tracker) = cost_tracker {
        record_cost(tracker, active_model, turn_usage);
    }
}

/// Record cost and emit budget events
fn record_cost(tracker: &CostTracker, model: &str, usage: &Usage) {
    let (total_cost, budget_events) =
        tracker.record_usage(model, usage.input_tokens as u64, usage.output_tokens as u64);

    for event in budget_events {
        match event {
            clankers_model_selection::cost_tracker::BudgetEvent::Warning { threshold, current } => {
                tracing::warn!("Budget warning: ${:.2} spent (soft limit: ${:.2})", current, threshold,);
            }
            clankers_model_selection::cost_tracker::BudgetEvent::Exceeded { limit, current } => {
                tracing::warn!("Budget exceeded: ${:.2} spent (hard limit: ${:.2})", current, limit,);
            }
            clankers_model_selection::cost_tracker::BudgetEvent::Milestone { milestone, total: _ } => {
                tracing::info!("Cost milestone: ${:.2}", milestone);
            }
        }
    }

    tracing::debug!(
        "Turn cost recorded: model={}, in={}, out={}, total=${:.4}",
        model,
        usage.input_tokens,
        usage.output_tokens,
        total_cost,
    );
}
