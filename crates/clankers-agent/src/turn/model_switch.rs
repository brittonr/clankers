//! Model switching logic

use tokio::sync::broadcast;

use crate::error::Result;
use crate::events::AgentEvent;
use crate::tool::ModelSwitchSlot;

/// Check for pending model switch from tools
pub(super) fn check_model_switch(
    active_model: &mut String,
    model_switch_slot: Option<&ModelSwitchSlot>,
    event_tx: &broadcast::Sender<AgentEvent>,
) -> Result<()> {
    if let Some(slot) = model_switch_slot
        && let Some(new_model) = slot.lock().take()
    {
        tracing::info!("Agent-requested model switch: {} → {}", active_model, new_model);
        let _ = event_tx.send(AgentEvent::ModelChange {
            from: active_model.clone(),
            to: new_model.clone(),
            reason: "agent_request".to_string(),
        });
        *active_model = new_model;
    }
    Ok(())
}
