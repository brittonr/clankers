use super::*;

// ── clankers-hash plugin tests ──────────────────────────────────────

// ── Hash plugin discovery ───────────────────────────────────────

#[test]
fn discover_finds_hash_plugin() {
    let mgr = manager_with_test_plugin();
    assert!(mgr.get("clankers-hash").is_some(), "Hash plugin should be discovered");
}

#[test]
fn discover_reads_hash_manifest_metadata() {
    let mgr = manager_with_test_plugin();
    let info = mgr.get("clankers-hash").unwrap();
    assert_eq!(info.name, "clankers-hash");
    assert_eq!(info.version, "0.1.0");
    assert_eq!(info.state, PluginState::Loaded);
    assert!(info.manifest.description.contains("hash"));
}

#[test]
fn discover_reads_hash_manifest_tools() {
    let mgr = manager_with_test_plugin();
    let info = mgr.get("clankers-hash").unwrap();
    assert_eq!(info.manifest.tools, vec!["hash_text", "encode_text"]);
}

#[test]
fn discover_reads_hash_manifest_events() {
    let mgr = manager_with_test_plugin();
    let info = mgr.get("clankers-hash").unwrap();
    assert_eq!(info.manifest.events, vec!["agent_start", "agent_end"]);
}

// ── Hash plugin WASM loading ────────────────────────────────────

#[test]
fn load_hash_wasm_transitions_to_active() {
    let mgr = loaded_hash_manager();
    let info = mgr.get("clankers-hash").unwrap();
    assert_eq!(info.state, PluginState::Active);
}

// ── Hash plugin function existence ──────────────────────────────

#[test]
fn hash_plugin_has_expected_functions() {
    let mgr = loaded_hash_manager();
    assert!(mgr.has_function("clankers-hash", "handle_tool_call"));
    assert!(mgr.has_function("clankers-hash", "on_event"));
    assert!(mgr.has_function("clankers-hash", "describe"));
}

// ── hash_text tool calls ────────────────────────────────────────

#[test]
fn hash_sha256_hello_world() {
    let mgr = loaded_hash_manager();
    let input = r#"{"tool":"hash_text","args":{"text":"hello world","algorithm":"sha256"}}"#;
    let result = mgr.call_plugin("clankers-hash", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["tool"], "hash_text");
    assert_eq!(parsed["result"], "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9");
}

#[test]
fn hash_sha256_empty() {
    let mgr = loaded_hash_manager();
    let input = r#"{"tool":"hash_text","args":{"text":"","algorithm":"sha256"}}"#;
    let result = mgr.call_plugin("clankers-hash", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["result"], "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
}

#[test]
fn hash_md5_hello_world() {
    let mgr = loaded_hash_manager();
    let input = r#"{"tool":"hash_text","args":{"text":"hello world","algorithm":"md5"}}"#;
    let result = mgr.call_plugin("clankers-hash", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["result"], "5eb63bbbe01eeed093cb22bb8f5acdc3");
}

#[test]
fn hash_md5_empty() {
    let mgr = loaded_hash_manager();
    let input = r#"{"tool":"hash_text","args":{"text":"","algorithm":"md5"}}"#;
    let result = mgr.call_plugin("clankers-hash", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["result"], "d41d8cd98f00b204e9800998ecf8427e");
}

#[test]
fn hash_default_algorithm_is_sha256() {
    let mgr = loaded_hash_manager();
    let input = r#"{"tool":"hash_text","args":{"text":"hello world"}}"#;
    let result = mgr.call_plugin("clankers-hash", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["status"], "ok");
    // Default should be SHA-256
    assert_eq!(parsed["result"], "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9");
}

// ── encode_text tool calls ──────────────────────────────────────

#[test]
fn encode_base64() {
    let mgr = loaded_hash_manager();
    let input = r#"{"tool":"encode_text","args":{"text":"hello world","encoding":"base64","direction":"encode"}}"#;
    let result = mgr.call_plugin("clankers-hash", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["result"], "aGVsbG8gd29ybGQ=");
}

#[test]
fn decode_base64() {
    let mgr = loaded_hash_manager();
    let input = r#"{"tool":"encode_text","args":{"text":"aGVsbG8gd29ybGQ=","encoding":"base64","direction":"decode"}}"#;
    let result = mgr.call_plugin("clankers-hash", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["result"], "hello world");
}

#[test]
fn encode_hex() {
    let mgr = loaded_hash_manager();
    let input = r#"{"tool":"encode_text","args":{"text":"hello","encoding":"hex","direction":"encode"}}"#;
    let result = mgr.call_plugin("clankers-hash", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["result"], "68656c6c6f");
}

#[test]
fn decode_hex() {
    let mgr = loaded_hash_manager();
    let input = r#"{"tool":"encode_text","args":{"text":"68656c6c6f","encoding":"hex","direction":"decode"}}"#;
    let result = mgr.call_plugin("clankers-hash", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["result"], "hello");
}

#[test]
fn encode_default_is_base64() {
    let mgr = loaded_hash_manager();
    let input = r#"{"tool":"encode_text","args":{"text":"test"}}"#;
    let result = mgr.call_plugin("clankers-hash", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["result"], "dGVzdA=="); // base64("test")
}

#[test]
fn encode_default_direction_is_encode() {
    let mgr = loaded_hash_manager();
    let input = r#"{"tool":"encode_text","args":{"text":"test","encoding":"hex"}}"#;
    let result = mgr.call_plugin("clankers-hash", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["result"], "74657374"); // hex("test")
}

#[test]
fn decode_invalid_base64_returns_error() {
    let mgr = loaded_hash_manager();
    let input =
        r#"{"tool":"encode_text","args":{"text":"!!!not-valid-base64!!!","encoding":"base64","direction":"decode"}}"#;
    let result = mgr.call_plugin("clankers-hash", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    let status = parsed["status"].as_str().unwrap();
    assert!(status.starts_with("error"), "Expected error status, got: {}", status);
}

#[test]
fn decode_invalid_hex_returns_error() {
    let mgr = loaded_hash_manager();
    let input = r#"{"tool":"encode_text","args":{"text":"xyz","encoding":"hex","direction":"decode"}}"#;
    let result = mgr.call_plugin("clankers-hash", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    let status = parsed["status"].as_str().unwrap();
    assert!(status.starts_with("error"), "Expected error status, got: {}", status);
}

// ── Hash plugin event handling ──────────────────────────────────

#[test]
fn hash_on_event_agent_start() {
    let mgr = loaded_hash_manager();
    let input = r#"{"event":"agent_start","data":{}}"#;
    let result = mgr.call_plugin("clankers-hash", "on_event", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["handled"], true);
    assert_eq!(parsed["event"], "agent_start");
}

#[test]
fn hash_on_event_agent_end() {
    let mgr = loaded_hash_manager();
    let input = r#"{"event":"agent_end","data":{}}"#;
    let result = mgr.call_plugin("clankers-hash", "on_event", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["handled"], true);
}

#[test]
fn hash_on_event_unknown() {
    let mgr = loaded_hash_manager();
    let input = r#"{"event":"custom_event","data":{}}"#;
    let result = mgr.call_plugin("clankers-hash", "on_event", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["handled"], false);
}

// ── Hash plugin unknown tool ────────────────────────────────────

#[test]
fn hash_unknown_tool_returns_unknown_status() {
    let mgr = loaded_hash_manager();
    let input = r#"{"tool":"nonexistent","args":{}}"#;
    let result = mgr.call_plugin("clankers-hash", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["status"], "unknown_tool");
}

// ── Hash plugin describe ────────────────────────────────────────

#[test]
fn hash_describe_metadata() {
    let mgr = loaded_hash_manager();
    let result = mgr.call_plugin("clankers-hash", "describe", "null").unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["name"], "clankers-hash");
    assert_eq!(parsed["version"], "0.1.0");
    let tools = parsed["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0]["name"], "hash_text");
    assert_eq!(tools[1]["name"], "encode_text");
}
