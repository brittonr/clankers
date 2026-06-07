//! Token usage tracking and cost calculation

use clanker_message::Usage;
use tokio::sync::broadcast;

use crate::events::AgentEvent;
use crate::routing::AgentCostRecorder;
use crate::routing::record_turn_cost;

/// Update usage tracking and emit events.
pub(super) fn update_usage_tracking(
    cumulative_usage: &mut Usage,
    turn_usage: &Usage,
    active_model: &str,
    cost_recorder: Option<&dyn AgentCostRecorder>,
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

    let _ = record_turn_cost(cost_recorder, active_model, turn_usage);
}
