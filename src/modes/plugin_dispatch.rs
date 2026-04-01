//! Dispatch agent events to loaded plugins.

use std::sync::Arc;

use crate::agent::events::AgentEvent;
use clankers_protocol::DaemonEvent;

/// Convert a `PluginUiAction` into its corresponding `DaemonEvent`.
pub(crate) fn ui_action_to_daemon_event(action: crate::plugin::ui::PluginUiAction) -> DaemonEvent {
    match action {
        crate::plugin::ui::PluginUiAction::SetWidget { plugin, widget } => DaemonEvent::PluginWidget {
            plugin,
            widget: Some(serde_json::to_value(widget).unwrap_or_default()),
        },
        crate::plugin::ui::PluginUiAction::ClearWidget { plugin } => {
            DaemonEvent::PluginWidget { plugin, widget: None }
        }
        crate::plugin::ui::PluginUiAction::SetStatus { plugin, text, color } => {
            DaemonEvent::PluginStatus { plugin, text: Some(text), color }
        }
        crate::plugin::ui::PluginUiAction::ClearStatus { plugin } => {
            DaemonEvent::PluginStatus { plugin, text: None, color: None }
        }
        crate::plugin::ui::PluginUiAction::Notify { plugin, message, level } => {
            DaemonEvent::PluginNotify { plugin, message, level }
        }
    }
}

/// Result of dispatching events to plugins.
pub(crate) struct PluginDispatchResult {
    /// Messages to surface to the user
    pub messages: Vec<(String, String)>,
    /// UI actions to apply
    pub ui_actions: Vec<crate::plugin::ui::PluginUiAction>,
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
        let is_subscribed = info
            .manifest
            .events
            .iter()
            .any(|e| PluginEvent::parse(e).is_some_and(|pe| pe.matches_event_kind(event_kind)));
        if !is_subscribed {
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
                    let should_display = parsed.get("display").and_then(|d| d.as_bool()).unwrap_or(false);
                    if should_display
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

/// Map a `DaemonEvent` to the plugin event kind string and JSON payload.
/// Returns `None` for events that plugins don't subscribe to.
fn daemon_event_to_plugin_payload(event: &DaemonEvent) -> Option<serde_json::Value> {
    match event {
        DaemonEvent::AgentStart => Some(serde_json::json!({"event": "agent_start", "data": {}})),
        DaemonEvent::AgentEnd => Some(serde_json::json!({"event": "agent_end", "data": {}})),
        DaemonEvent::ToolCall { tool_name, call_id, .. } => {
            Some(serde_json::json!({"event": "tool_call", "data": {"tool": tool_name, "call_id": call_id}}))
        }
        DaemonEvent::ToolDone { call_id, .. } => {
            Some(serde_json::json!({"event": "tool_result", "data": {"call_id": call_id}}))
        }
        DaemonEvent::UserInput { text, .. } => {
            Some(serde_json::json!({"event": "user_input", "data": {"text": text}}))
        }
        DaemonEvent::ModelChanged { from, to, .. } => {
            Some(serde_json::json!({"event": "model_change", "data": {"from": from, "to": to}}))
        }
        DaemonEvent::UsageUpdate { input_tokens, output_tokens, .. } => {
            Some(serde_json::json!({"event": "usage_update", "data": {"input_tokens": input_tokens, "output_tokens": output_tokens}}))
        }
        DaemonEvent::SessionCompaction { .. } => {
            Some(serde_json::json!({"event": "session_compaction", "data": {}}))
        }
        _ => None,
    }
}

/// Dispatch daemon events to subscribed plugins.
///
/// Used by the daemon's event loop where we have `DaemonEvent`s instead
/// of `AgentEvent`s. Maps events to the same plugin protocol.
pub(crate) fn dispatch_daemon_events_to_plugins(
    plugin_manager: &Arc<std::sync::Mutex<crate::plugin::PluginManager>>,
    events: &[DaemonEvent],
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

    for event in events {
        let Some(payload) = daemon_event_to_plugin_payload(event) else {
            continue;
        };
        let event_kind = payload.get("event").and_then(|v| v.as_str()).unwrap_or("");
        let input = serde_json::to_string(&payload).unwrap_or_default();

        for info in mgr.list() {
            if info.state != PluginState::Active {
                continue;
            }
            let is_subscribed = info
                .manifest
                .events
                .iter()
                .any(|e| PluginEvent::parse(e).is_some_and(|pe| pe.matches_event_kind(event_kind)));
            if !is_subscribed {
                continue;
            }

            match mgr.call_plugin(&info.name, "on_event", &input) {
                Ok(output) => {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&output) {
                        let should_display = parsed.get("display").and_then(|d| d.as_bool()).unwrap_or(false);
                        if should_display
                            && let Some(msg) = parsed.get("message").and_then(|m| m.as_str())
                            && !msg.is_empty()
                        {
                            messages.push((info.name.clone(), msg.to_string()));
                        }
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
    }

    PluginDispatchResult { messages, ui_actions }
}
