//! Dispatch agent events to loaded plugins.

use std::sync::Arc;

use clankers_protocol::DaemonEvent;

use crate::agent::events::AgentEvent;

/// Convert a `PluginUiAction` into its corresponding `DaemonEvent`.
pub(crate) fn ui_action_to_daemon_event(action: crate::plugin::ui::PluginUiAction) -> DaemonEvent {
    match action {
        crate::plugin::ui::PluginUiAction::SetWidget { plugin, widget } => DaemonEvent::PluginWidget {
            plugin,
            widget: Some(serde_json::to_value(widget).unwrap_or_default()),
        },
        crate::plugin::ui::PluginUiAction::ClearWidget { plugin } => DaemonEvent::PluginWidget { plugin, widget: None },
        crate::plugin::ui::PluginUiAction::SetStatus { plugin, text, color } => DaemonEvent::PluginStatus {
            plugin,
            text: Some(text),
            color,
        },
        crate::plugin::ui::PluginUiAction::ClearStatus { plugin } => DaemonEvent::PluginStatus {
            plugin,
            text: None,
            color: None,
        },
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
    use crate::plugin::sandbox;

    let host = crate::plugin::PluginHostFacade::new(Arc::clone(plugin_manager));
    let mut messages = Vec::new();
    let mut ui_actions = Vec::new();
    let event_kind = event.event_kind();

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
        _ => return PluginDispatchResult { messages, ui_actions },
    };

    let input = serde_json::to_string(&payload).unwrap_or_default();
    for info in host.event_subscribers(event_kind) {
        match host.call_plugin(&info.name, "on_event", &input) {
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
        DaemonEvent::UserInput { text, .. } => Some(serde_json::json!({"event": "user_input", "data": {"text": text}})),
        DaemonEvent::ModelChanged { from, to, .. } => {
            Some(serde_json::json!({"event": "model_change", "data": {"from": from, "to": to}}))
        }
        DaemonEvent::UsageUpdate {
            input_tokens,
            output_tokens,
            ..
        } => Some(
            serde_json::json!({"event": "usage_update", "data": {"input_tokens": input_tokens, "output_tokens": output_tokens}}),
        ),
        DaemonEvent::SessionCompaction { .. } => Some(serde_json::json!({"event": "session_compaction", "data": {}})),
        DaemonEvent::ScheduleFire {
            schedule_id,
            schedule_name,
            payload,
            fire_count,
        } => Some(serde_json::json!({
            "event": "schedule_fire",
            "data": {
                "schedule_id": schedule_id,
                "schedule_name": schedule_name,
                "payload": payload,
                "fire_count": fire_count,
            }
        })),
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
    use crate::plugin::sandbox;

    let host = crate::plugin::PluginHostFacade::new(Arc::clone(plugin_manager));
    let mut messages = Vec::new();
    let mut ui_actions = Vec::new();

    for event in events {
        let Some(payload) = daemon_event_to_plugin_payload(event) else {
            continue;
        };
        let event_kind = payload.get("event").and_then(|v| v.as_str()).unwrap_or("");
        let input = serde_json::to_string(&payload).unwrap_or_default();

        for info in host.event_subscribers(event_kind) {
            match host.call_plugin(&info.name, "on_event", &input) {
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

#[cfg(test)]
mod tests {
    use clankers_protocol::DaemonEvent;
    use serde_json::json;

    use super::*;

    // ── daemon_event_to_plugin_payload mapping ──────────────────

    #[test]
    fn agent_start_maps_correctly() {
        let payload = daemon_event_to_plugin_payload(&DaemonEvent::AgentStart).unwrap();
        assert_eq!(payload["event"], "agent_start");
    }

    #[test]
    fn agent_end_maps_correctly() {
        let payload = daemon_event_to_plugin_payload(&DaemonEvent::AgentEnd).unwrap();
        assert_eq!(payload["event"], "agent_end");
    }

    #[test]
    fn tool_call_maps_fields() {
        let event = DaemonEvent::ToolCall {
            tool_name: "bash".into(),
            call_id: "call-42".into(),
            input: json!({"command": "ls"}),
        };
        let payload = daemon_event_to_plugin_payload(&event).unwrap();
        assert_eq!(payload["event"], "tool_call");
        assert_eq!(payload["data"]["tool"], "bash");
        assert_eq!(payload["data"]["call_id"], "call-42");
    }

    #[test]
    fn tool_done_maps_call_id() {
        let event = DaemonEvent::ToolDone {
            call_id: "call-99".into(),
            text: String::new(),
            images: vec![],
            is_error: false,
        };
        let payload = daemon_event_to_plugin_payload(&event).unwrap();
        assert_eq!(payload["event"], "tool_result");
        assert_eq!(payload["data"]["call_id"], "call-99");
    }

    #[test]
    fn user_input_maps_text() {
        let event = DaemonEvent::UserInput {
            text: "hello".into(),
            agent_msg_count: 5,
        };
        let payload = daemon_event_to_plugin_payload(&event).unwrap();
        assert_eq!(payload["event"], "user_input");
        assert_eq!(payload["data"]["text"], "hello");
    }

    #[test]
    fn model_changed_maps_from_to() {
        let event = DaemonEvent::ModelChanged {
            from: "gpt-4".into(),
            to: "claude-3".into(),
            reason: "user".into(),
        };
        let payload = daemon_event_to_plugin_payload(&event).unwrap();
        assert_eq!(payload["event"], "model_change");
        assert_eq!(payload["data"]["from"], "gpt-4");
        assert_eq!(payload["data"]["to"], "claude-3");
    }

    #[test]
    fn usage_update_maps_tokens() {
        let event = DaemonEvent::UsageUpdate {
            input_tokens: 100,
            output_tokens: 200,
            cache_read: 0,
            model: "test".into(),
        };
        let payload = daemon_event_to_plugin_payload(&event).unwrap();
        assert_eq!(payload["event"], "usage_update");
        assert_eq!(payload["data"]["input_tokens"], 100);
        assert_eq!(payload["data"]["output_tokens"], 200);
    }

    #[test]
    fn session_compaction_maps() {
        let event = DaemonEvent::SessionCompaction {
            compacted_count: 10,
            tokens_saved: 500,
        };
        let payload = daemon_event_to_plugin_payload(&event).unwrap();
        assert_eq!(payload["event"], "session_compaction");
    }

    // ── Events that should NOT map to plugin payloads ──────────

    #[test]
    fn text_delta_not_dispatched() {
        let event = DaemonEvent::TextDelta { text: "hi".into() };
        assert!(daemon_event_to_plugin_payload(&event).is_none());
    }

    #[test]
    fn content_block_start_not_dispatched() {
        let event = DaemonEvent::ContentBlockStart { is_thinking: false };
        assert!(daemon_event_to_plugin_payload(&event).is_none());
    }

    #[test]
    fn system_message_not_dispatched() {
        let event = DaemonEvent::SystemMessage {
            text: "info".into(),
            is_error: false,
        };
        assert!(daemon_event_to_plugin_payload(&event).is_none());
    }

    #[test]
    fn prompt_done_not_dispatched() {
        let event = DaemonEvent::PromptDone { error: None };
        assert!(daemon_event_to_plugin_payload(&event).is_none());
    }

    // ── ui_action_to_daemon_event mapping ──────────────────────

    #[test]
    fn set_widget_maps_to_plugin_widget() {
        use clankers_tui_types::Widget;

        use crate::plugin::ui::PluginUiAction;
        let action = PluginUiAction::SetWidget {
            plugin: "test".into(),
            widget: Widget::Text {
                content: "hello".into(),
                bold: false,
                color: None,
            },
        };
        let event = ui_action_to_daemon_event(action);
        match event {
            DaemonEvent::PluginWidget { plugin, widget } => {
                assert_eq!(plugin, "test");
                assert!(widget.is_some());
            }
            other => panic!("expected PluginWidget, got {other:?}"),
        }
    }

    #[test]
    fn clear_widget_maps_to_none() {
        use crate::plugin::ui::PluginUiAction;
        let action = PluginUiAction::ClearWidget { plugin: "test".into() };
        let event = ui_action_to_daemon_event(action);
        match event {
            DaemonEvent::PluginWidget { widget, .. } => assert!(widget.is_none()),
            other => panic!("expected PluginWidget, got {other:?}"),
        }
    }

    #[test]
    fn set_status_maps_to_plugin_status() {
        use crate::plugin::ui::PluginUiAction;
        let action = PluginUiAction::SetStatus {
            plugin: "test".into(),
            text: "busy".into(),
            color: Some("yellow".into()),
        };
        let event = ui_action_to_daemon_event(action);
        match event {
            DaemonEvent::PluginStatus { plugin, text, color } => {
                assert_eq!(plugin, "test");
                assert_eq!(text.unwrap(), "busy");
                assert_eq!(color.unwrap(), "yellow");
            }
            other => panic!("expected PluginStatus, got {other:?}"),
        }
    }

    #[test]
    fn clear_status_maps_to_none_text() {
        use crate::plugin::ui::PluginUiAction;
        let action = PluginUiAction::ClearStatus { plugin: "test".into() };
        let event = ui_action_to_daemon_event(action);
        match event {
            DaemonEvent::PluginStatus { text, color, .. } => {
                assert!(text.is_none());
                assert!(color.is_none());
            }
            other => panic!("expected PluginStatus, got {other:?}"),
        }
    }

    #[test]
    fn notify_maps_to_plugin_notify() {
        use crate::plugin::ui::PluginUiAction;
        let action = PluginUiAction::Notify {
            plugin: "test".into(),
            message: "done!".into(),
            level: "info".into(),
        };
        let event = ui_action_to_daemon_event(action);
        match event {
            DaemonEvent::PluginNotify { plugin, message, level } => {
                assert_eq!(plugin, "test");
                assert_eq!(message, "done!");
                assert_eq!(level, "info");
            }
            other => panic!("expected PluginNotify, got {other:?}"),
        }
    }
}
