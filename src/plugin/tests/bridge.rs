use crate::plugin::bridge::PluginEvent;
use crate::plugin::bridge::{self};
use crate::plugin::ui;

// ── Bridge event parsing tests ───────────────────────────────────

#[test]
fn bridge_parse_known_events() {
    assert_eq!(PluginEvent::parse("tool_call"), Some(PluginEvent::ToolCall));
    assert_eq!(PluginEvent::parse("tool_result"), Some(PluginEvent::ToolResult));
    assert_eq!(PluginEvent::parse("agent_start"), Some(PluginEvent::AgentStart));
    assert_eq!(PluginEvent::parse("agent_end"), Some(PluginEvent::AgentEnd));
    assert_eq!(PluginEvent::parse("turn_start"), Some(PluginEvent::TurnStart));
    assert_eq!(PluginEvent::parse("turn_end"), Some(PluginEvent::TurnEnd));
    assert_eq!(PluginEvent::parse("message_update"), Some(PluginEvent::MessageUpdate));
    assert_eq!(PluginEvent::parse("user_input"), Some(PluginEvent::UserInput));
}

#[test]
fn bridge_parse_unknown_event() {
    assert_eq!(PluginEvent::parse("unknown"), None);
    assert_eq!(PluginEvent::parse(""), None);
}

// ── Bridge UI action parsing ─────────────────────────────────────

#[test]
fn bridge_parse_ui_actions_single() {
    let response = serde_json::json!({
        "handled": true,
        "ui": {
            "action": "set_status",
            "text": "running",
            "color": "green"
        }
    });
    let actions = bridge::parse_ui_actions("my-plugin", &response);
    assert_eq!(actions.len(), 1);
    match &actions[0] {
        ui::PluginUiAction::SetStatus { plugin, text, color } => {
            assert_eq!(plugin, "my-plugin");
            assert_eq!(text, "running");
            assert_eq!(color.as_deref(), Some("green"));
        }
        _ => panic!("Expected SetStatus"),
    }
}

#[test]
fn bridge_parse_ui_actions_array() {
    let response = serde_json::json!({
        "handled": true,
        "ui": [
            {"action": "set_status", "text": "ok", "color": "green"},
            {"action": "notify", "message": "done!", "level": "info"},
            {"action": "set_widget", "widget": {"type": "Text", "content": "hi"}}
        ]
    });
    let actions = bridge::parse_ui_actions("test-plugin", &response);
    assert_eq!(actions.len(), 3);
}

#[test]
fn bridge_parse_ui_actions_none() {
    let response = serde_json::json!({"handled": true});
    let actions = bridge::parse_ui_actions("test", &response);
    assert!(actions.is_empty());
}

#[test]
fn bridge_parse_ui_actions_invalid_ignored() {
    let response = serde_json::json!({
        "ui": [
            {"action": "set_status", "text": "ok"},
            {"action": "bogus_action"},
            {"action": "notify", "message": "hi"}
        ]
    });
    let actions = bridge::parse_ui_actions("test", &response);
    // bogus_action should be silently skipped
    assert_eq!(actions.len(), 2);
}

#[test]
fn bridge_parse_ui_actions_injects_plugin_name() {
    let response = serde_json::json!({
        "ui": {"action": "set_status", "text": "test"}
    });
    let actions = bridge::parse_ui_actions("injected-name", &response);
    match &actions[0] {
        ui::PluginUiAction::SetStatus { plugin, .. } => {
            assert_eq!(plugin, "injected-name");
        }
        _ => panic!("Expected SetStatus"),
    }
}

#[test]
fn bridge_parse_ui_actions_preserves_explicit_plugin_name() {
    let response = serde_json::json!({
        "ui": {"action": "set_status", "plugin": "explicit", "text": "test"}
    });
    let actions = bridge::parse_ui_actions("fallback", &response);
    match &actions[0] {
        ui::PluginUiAction::SetStatus { plugin, .. } => {
            assert_eq!(plugin, "explicit");
        }
        _ => panic!("Expected SetStatus"),
    }
}
