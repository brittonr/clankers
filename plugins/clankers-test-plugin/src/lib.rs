//! clankers test plugin — WASM module for testing the plugin system.
//!
//! Exports several Extism guest functions that exercise different aspects
//! of the plugin protocol: string I/O, JSON processing, event handling,
//! tool registration, and error paths.

use clanker_plugin_sdk::prelude::*;

// ── Simple string functions ───────────────────────────────────────────

/// Identity echo — returns whatever input is given.
#[plugin_fn]
pub fn echo(input: String) -> FnResult<String> {
    Ok(input)
}

/// Greet the user by name.
#[plugin_fn]
pub fn greet(name: String) -> FnResult<String> {
    Ok(format!("Hello, {}! 👋", name))
}

/// Reverse the input string.
#[plugin_fn]
pub fn reverse(input: String) -> FnResult<String> {
    Ok(input.chars().rev().collect())
}

/// Return the byte-length of the input.
#[plugin_fn]
pub fn length(input: String) -> FnResult<String> {
    Ok(input.len().to_string())
}

// ── JSON tool dispatch ───────────────────────────────────────────────

fn handle_echo(args: &Value) -> Result<String, String> {
    Ok(args.get_str_or("text", "").to_string())
}

fn handle_reverse(args: &Value) -> Result<String, String> {
    let text = args.get_str_or("text", "");
    Ok(text.chars().rev().collect())
}

fn dispatch_test_tool_call(input: &str) -> FnResult<String> {
    dispatch_tools(input, &[
        ("test_echo", handle_echo),
        ("test_reverse", handle_reverse),
    ])
}

/// Process a tool call (JSON in, JSON out).
#[plugin_fn]
pub fn handle_tool_call(input: String) -> FnResult<String> {
    dispatch_test_tool_call(&input)
}

// ── Event handling ───────────────────────────────────────────────────

fn dispatch_test_event(input: &str) -> FnResult<String> {
    dispatch_events(input, &[
        ("agent_start", |_| "Test plugin initialized".to_string()),
        ("agent_end", |_| "Test plugin shutting down".to_string()),
        ("tool_call", |data| {
            let tool_name = data.get_str("tool").unwrap_or("unknown");
            format!("Observed tool call: {tool_name}")
        }),
    ])
}

/// Handle a plugin lifecycle event.
#[plugin_fn]
pub fn on_event(input: String) -> FnResult<String> {
    dispatch_test_event(&input)
}

// ── Plugin metadata ──────────────────────────────────────────────────

/// Return plugin metadata as JSON.
#[plugin_fn]
pub fn describe(Json(_): Json<()>) -> FnResult<Json<PluginMeta>> {
    Ok(Json(PluginMeta::new("clankers-test-plugin", "0.1.0", &[
        ("test_echo", "Echoes input text back"),
        ("test_reverse", "Reverses input text"),
    ], &["test"])))
}

// ── Utility functions ────────────────────────────────────────────────

/// Count words in the input.
#[plugin_fn]
pub fn count_words(input: String) -> FnResult<String> {
    let count = input.split_whitespace().count();
    Ok(count.to_string())
}

/// Uppercase the input.
#[plugin_fn]
pub fn uppercase(input: String) -> FnResult<String> {
    Ok(input.to_uppercase())
}

// ── Error path testing ───────────────────────────────────────────────

/// Always returns an error.
#[plugin_fn]
pub fn fail(_input: String) -> FnResult<String> {
    Err(Error::msg("intentional test failure").into())
}

/// Expects valid JSON; returns an error on malformed input.
#[plugin_fn]
pub fn parse_json(input: String) -> FnResult<String> {
    let value: Value = clanker_plugin_sdk::serde_json::from_str(&input)
        .map_err(|e| Error::msg(format!("JSON parse error: {e}")))?;
    Ok(clanker_plugin_sdk::serde_json::to_string_pretty(&value)
        .map_err(|e| Error::msg(format!("JSON serialize error: {e}")))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_dispatch_handles_agent_start() {
        let input = clanker_plugin_sdk::serde_json::json!({
            "event": "agent_start",
            "data": {},
        })
        .to_string();
        let output = dispatch_test_event(&input).expect("agent_start event should dispatch");
        let result: Value = clanker_plugin_sdk::serde_json::from_str(&output).expect("event result should be JSON");

        assert_eq!(result["event"], "agent_start");
        assert_eq!(result["handled"], true);
        assert_eq!(result["message"], "Test plugin initialized");
    }

    #[test]
    fn tool_dispatch_reverses_text() {
        let input = clanker_plugin_sdk::serde_json::json!({
            "tool": "test_reverse",
            "args": { "text": "abc" },
        })
        .to_string();
        let output = dispatch_test_tool_call(&input).expect("test_reverse should dispatch");
        let result: Value = clanker_plugin_sdk::serde_json::from_str(&output).expect("tool result should be JSON");

        assert_eq!(result["tool"], "test_reverse");
        assert_eq!(result["status"], "ok");
        assert_eq!(result["result"], "cba");
    }
}
