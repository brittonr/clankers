//! Token usage tracking and cost calculation

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

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

    event_tx
        .send(AgentEvent::UsageUpdate {
            turn_usage: turn_usage.clone(),
            cumulative_usage: cumulative_usage.clone(),
        })
        .ok();

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
