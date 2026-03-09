use super::*;

// ── String function calls ────────────────────────────────────────

#[test]
fn call_echo() {
    let mgr = loaded_manager();
    let result = mgr.call_plugin("clankers-test-plugin", "echo", "hello world").unwrap();
    assert_eq!(result, "hello world");
}

#[test]
fn call_echo_empty() {
    let mgr = loaded_manager();
    let result = mgr.call_plugin("clankers-test-plugin", "echo", "").unwrap();
    assert_eq!(result, "");
}

#[test]
fn call_echo_unicode() {
    let mgr = loaded_manager();
    let result = mgr.call_plugin("clankers-test-plugin", "echo", "🦀 Rust + WASM 🎉").unwrap();
    assert_eq!(result, "🦀 Rust + WASM 🎉");
}

#[test]
fn call_echo_large_input() {
    let mgr = loaded_manager();
    let large = "x".repeat(100_000);
    let result = mgr.call_plugin("clankers-test-plugin", "echo", &large).unwrap();
    assert_eq!(result.len(), 100_000);
}

#[test]
fn call_greet() {
    let mgr = loaded_manager();
    let result = mgr.call_plugin("clankers-test-plugin", "greet", "clankers").unwrap();
    assert_eq!(result, "Hello, clankers! 👋");
}

#[test]
fn call_reverse() {
    let mgr = loaded_manager();
    let result = mgr.call_plugin("clankers-test-plugin", "reverse", "abcdef").unwrap();
    assert_eq!(result, "fedcba");
}

#[test]
fn call_reverse_unicode() {
    let mgr = loaded_manager();
    let result = mgr.call_plugin("clankers-test-plugin", "reverse", "🅰🅱🅲").unwrap();
    assert_eq!(result, "🅲🅱🅰");
}

#[test]
fn call_length() {
    let mgr = loaded_manager();
    let result = mgr.call_plugin("clankers-test-plugin", "length", "hello").unwrap();
    assert_eq!(result, "5");
}

#[test]
fn call_count_words() {
    let mgr = loaded_manager();
    let result = mgr.call_plugin("clankers-test-plugin", "count_words", "the quick brown fox").unwrap();
    assert_eq!(result, "4");
}

#[test]
fn call_count_words_empty() {
    let mgr = loaded_manager();
    let result = mgr.call_plugin("clankers-test-plugin", "count_words", "").unwrap();
    assert_eq!(result, "0");
}

#[test]
fn call_uppercase() {
    let mgr = loaded_manager();
    let result = mgr.call_plugin("clankers-test-plugin", "uppercase", "hello world").unwrap();
    assert_eq!(result, "HELLO WORLD");
}

// ── JSON function calls ──────────────────────────────────────────

#[test]
fn call_handle_tool_call_echo() {
    let mgr = loaded_manager();
    let input = r#"{"tool":"test_echo","args":{"text":"hello"}}"#;
    let result = mgr.call_plugin("clankers-test-plugin", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["tool"], "test_echo");
    assert_eq!(parsed["result"], "hello");
    assert_eq!(parsed["status"], "ok");
}

#[test]
fn call_handle_tool_call_reverse() {
    let mgr = loaded_manager();
    let input = r#"{"tool":"test_reverse","args":{"text":"abcd"}}"#;
    let result = mgr.call_plugin("clankers-test-plugin", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["tool"], "test_reverse");
    assert_eq!(parsed["result"], "dcba");
    assert_eq!(parsed["status"], "ok");
}

#[test]
fn call_handle_tool_call_unknown_tool() {
    let mgr = loaded_manager();
    let input = r#"{"tool":"unknown_tool","args":{}}"#;
    let result = mgr.call_plugin("clankers-test-plugin", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["status"], "unknown_tool");
}

#[test]
fn call_handle_tool_call_invalid_json_errors() {
    let mgr = loaded_manager();
    let result = mgr.call_plugin("clankers-test-plugin", "handle_tool_call", "not json");
    assert!(result.is_err());
}

// ── Plugin metadata (describe) ───────────────────────────────────

#[test]
fn call_describe() {
    let mgr = loaded_manager();
    let result = mgr.call_plugin("clankers-test-plugin", "describe", "null").unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["name"], "clankers-test-plugin");
    assert_eq!(parsed["version"], "0.1.0");
    let tools = parsed["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0]["name"], "test_echo");
    assert_eq!(tools[1]["name"], "test_reverse");
    let commands = parsed["commands"].as_array().unwrap();
    assert_eq!(commands, &["test"]);
}

// ── Error path ───────────────────────────────────────────────────

#[test]
fn call_fail_returns_error() {
    let mgr = loaded_manager();
    let result = mgr.call_plugin("clankers-test-plugin", "fail", "");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("intentional test failure"));
}

#[test]
fn call_parse_json_valid() {
    let mgr = loaded_manager();
    let result = mgr.call_plugin("clankers-test-plugin", "parse_json", r#"{"a":1}"#).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["a"], 1);
}

#[test]
fn call_parse_json_invalid() {
    let mgr = loaded_manager();
    let result = mgr.call_plugin("clankers-test-plugin", "parse_json", "not json{");
    assert!(result.is_err());
}

#[test]
fn call_nonexistent_function_errors() {
    let mgr = loaded_manager();
    let result = mgr.call_plugin("clankers-test-plugin", "no_such_fn", "");
    assert!(result.is_err());
}

#[test]
fn call_unloaded_plugin_errors() {
    let mgr = manager_with_test_plugin();
    let result = mgr.call_plugin("clankers-test-plugin", "echo", "test");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("not loaded"));
}
