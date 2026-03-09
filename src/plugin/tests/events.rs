use super::*;

// ── Event handling ───────────────────────────────────────────────

#[test]
fn call_on_event_agent_start() {
    let mgr = loaded_manager();
    let input = r#"{"event":"agent_start","data":{}}"#;
    let result = mgr.call_plugin("clankers-test-plugin", "on_event", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["event"], "agent_start");
    assert_eq!(parsed["handled"], true);
    assert_eq!(parsed["message"], "Test plugin initialized");
}

#[test]
fn call_on_event_agent_end() {
    let mgr = loaded_manager();
    let input = r#"{"event":"agent_end","data":{}}"#;
    let result = mgr.call_plugin("clankers-test-plugin", "on_event", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["handled"], true);
    assert_eq!(parsed["message"], "Test plugin shutting down");
}

#[test]
fn call_on_event_tool_call_with_data() {
    let mgr = loaded_manager();
    let input = r#"{"event":"tool_call","data":{"tool":"grep"}}"#;
    let result = mgr.call_plugin("clankers-test-plugin", "on_event", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["handled"], true);
    assert_eq!(parsed["message"], "Observed tool call: grep");
}

#[test]
fn call_on_event_unknown() {
    let mgr = loaded_manager();
    let input = r#"{"event":"custom_event","data":{}}"#;
    let result = mgr.call_plugin("clankers-test-plugin", "on_event", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["handled"], false);
    assert_eq!(parsed["message"], "Unhandled event: custom_event");
}
