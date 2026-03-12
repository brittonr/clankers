//! Dispatch agent events to loaded plugins.

use std::sync::Arc;

use crate::agent::events::AgentEvent;

/// Result of dispatching events to plugins.
pub(crate) struct PluginDispatchResult {
    /// Messages to surface to the user
    pub messages: Vec<(String, String)>,
    /// UI actions to apply
    pub ui_actions: Vec<crate::plugin::ui::PluginUIAction>,
}

/// Dispatch an agent event to all subscribed plugins.
/// Returns messages to surface and UI actions to apply.
pub(crate) fn dispatch_event_to_plugins(
    plugin_manager: &Arc<std::sync::Mutex<crate::plugin::PluginManager>>,
    event: &AgentEvent,
) -> PluginDispatchResult {
    use crate::plugin::PluginState;
    use crate::plugin::bridge::PluginEvent;
    use crate::plugin::sandbox;

    let mgr = match plugin_manager.lock() {
        Ok(m) => m,
        Err(poisoned) => {
            tracing::warn!("Plugin manager mutex was poisoned, recovering");
            poisoned.into_inner()
        }
    };

    let mut messages = Vec::new();
    let mut ui_actions = Vec::new();

    for info in mgr.list() {
        if info.state != PluginState::Active {
            continue;
        }
        // Check if this plugin subscribes to this event type
        let event_kind = event.event_kind();
        let subscribed = info
            .manifest
            .events
            .iter()
            .any(|e| PluginEvent::parse(e).is_some_and(|pe| pe.matches_event_kind(event_kind)));
        if !subscribed {
            continue;
        }

        // Build event payload
        let payload = match event {
            AgentEvent::AgentStart => serde_json::json!({"event": "agent_start", "data": {}}),
            AgentEvent::AgentEnd { .. } => serde_json::json!({"event": "agent_end", "data": {}}),
            AgentEvent::ToolCall { tool_name, call_id, .. } => {
                serde_json::json!({"event": "tool_call", "data": {"tool": tool_name, "call_id": call_id}})
            }
            AgentEvent::ToolExecutionEnd { call_id, .. } => {
                serde_json::json!({"event": "tool_result", "data": {"call_id": call_id}})
            }
            AgentEvent::TurnStart { index, .. } => {
                serde_json::json!({"event": "turn_start", "data": {"turn": index}})
            }
            AgentEvent::TurnEnd { index, .. } => {
                serde_json::json!({"event": "turn_end", "data": {"turn": index}})
            }
            AgentEvent::UserInput { text, .. } => {
                serde_json::json!({"event": "user_input", "data": {"text": text}})
            }
            AgentEvent::MessageUpdate { index, .. } => {
                serde_json::json!({"event": "message_update", "data": {"index": index}})
            }
            _ => continue,
        };

        let input = serde_json::to_string(&payload).unwrap_or_default();
        match mgr.call_plugin(&info.name, "on_event", &input) {
            Ok(output) => {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&output) {
                    // Surface messages the plugin explicitly wants shown
                    let wants_display = parsed.get("display").and_then(|d| d.as_bool()).unwrap_or(false);
                    if wants_display
                        && let Some(msg) = parsed.get("message").and_then(|m| m.as_str())
                        && !msg.is_empty()
                    {
                        messages.push((info.name.clone(), msg.to_string()));
                    }

                    // Parse UI actions, then enforce permission
                    let actions = crate::plugin::bridge::parse_ui_actions(&info.name, &parsed);
                    let actions = sandbox::filter_ui_actions(&info.manifest.permissions, actions);
                    ui_actions.extend(actions);
                }
            }
            Err(e) => {
                tracing::debug!("Plugin '{}' event handler error: {}", info.name, e);
            }
        }
    }

    PluginDispatchResult { messages, ui_actions }
}
