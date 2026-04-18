//! Tool backed by a plugin runtime.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;

use crate::plugin::PluginManager;
use crate::plugin::StdioToolCallEvent;
use crate::tools::Tool;
use crate::tools::ToolContext;
use crate::tools::ToolDefinition;
use crate::tools::ToolResult;

const DEFAULT_STDIO_TOOL_TIMEOUT_SECS: u64 = 300;
const DEFAULT_STDIO_CANCEL_GRACE_SECS: u64 = 5;

enum PluginToolBackend {
    Wasm { function_name: String },
    Stdio,
}

/// A tool that delegates execution to a plugin runtime.
pub struct PluginTool {
    definition: ToolDefinition,
    plugin_name: String,
    backend: PluginToolBackend,
    manager: Arc<std::sync::Mutex<PluginManager>>,
}

impl PluginTool {
    pub fn new(
        definition: ToolDefinition,
        plugin_name: String,
        function_name: String,
        manager: Arc<std::sync::Mutex<PluginManager>>,
    ) -> Self {
        Self {
            definition,
            plugin_name,
            backend: PluginToolBackend::Wasm { function_name },
            manager,
        }
    }

    pub fn new_stdio(
        definition: ToolDefinition,
        plugin_name: String,
        manager: Arc<std::sync::Mutex<PluginManager>>,
    ) -> Self {
        Self {
            definition,
            plugin_name,
            backend: PluginToolBackend::Stdio,
            manager,
        }
    }

    fn execute_wasm(&self, ctx: &ToolContext, function_name: &str, params: Value) -> ToolResult {
        // Wrap params in the tool call envelope that plugins expect:
        //   { "tool": "<tool_name>", "args": { ... } }
        let envelope = serde_json::json!({
            "tool": self.definition.name,
            "args": params,
        });
        let input = serde_json::to_string(&envelope).unwrap_or_default();

        ctx.emit_progress(&format!("plugin: {}::{}", self.plugin_name, function_name));

        let manager = match self.manager.lock() {
            Ok(m) => m,
            Err(poisoned) => {
                tracing::warn!("Plugin manager mutex poisoned, recovering");
                poisoned.into_inner()
            }
        };

        match manager.call_plugin(&self.plugin_name, function_name, &input) {
            Ok(output) => {
                ctx.emit_progress(&format!("plugin returned: {} bytes", output.len()));
                // Try to parse as JSON ToolResult first
                if let Ok(result) = serde_json::from_str::<ToolResult>(&output) {
                    return result;
                }
                // Try to parse the plugin's standard response format:
                //   { "tool": "...", "result": "...", "status": "ok" | "error" }
                if let Ok(parsed) = serde_json::from_str::<Value>(&output) {
                    // Process host_calls if present
                    let permissions = manager
                        .get(&self.plugin_name)
                        .map(|info| info.manifest.permissions.clone())
                        .unwrap_or_default();
                    let host_fns = crate::plugin::host::HostFunctions::new();
                    let host_results = host_fns.process_host_calls(&parsed, &permissions);
                    if !host_results.is_empty() {
                        tracing::debug!("Plugin '{}' made {} host call(s)", self.plugin_name, host_results.len());
                    }

                    let status = parsed.get("status").and_then(|s| s.as_str()).unwrap_or("ok");
                    let result = parsed.get("result").and_then(|r| r.as_str()).unwrap_or(&output);
                    if status == "ok" {
                        return ToolResult::text(result.to_string());
                    }
                    return ToolResult::error(result.to_string());
                }
                // Fallback: return raw output as text
                ToolResult::text(output)
            }
            Err(e) => ToolResult::error(format!("Plugin error: {}", e)),
        }
    }

    async fn execute_stdio(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        ctx.emit_progress(&format!("plugin: {}::{}", self.plugin_name, self.definition.name));

        let mut events = match crate::plugin::start_stdio_tool_call(
            &self.manager,
            &self.plugin_name,
            &ctx.call_id,
            &self.definition.name,
            params,
        ) {
            Ok(events) => events,
            Err(error) => return ToolResult::error(format!("Plugin error: {}", error)),
        };

        let tool_timeout = stdio_duration_override(
            "CLANKERS_STDIO_TOOL_TIMEOUT_MS",
            Duration::from_secs(DEFAULT_STDIO_TOOL_TIMEOUT_SECS),
        );
        let cancel_grace = stdio_duration_override(
            "CLANKERS_STDIO_TOOL_CANCEL_TIMEOUT_MS",
            Duration::from_secs(DEFAULT_STDIO_CANCEL_GRACE_SECS),
        );
        let timed_out = tokio::time::sleep(tool_timeout);
        tokio::pin!(timed_out);

        loop {
            tokio::select! {
                () = ctx.signal.cancelled() => {
                    crate::plugin::cancel_stdio_tool_call(&self.manager, &self.plugin_name, &ctx.call_id, "turn cancelled").ok();
                    return self.finish_cancelled_stdio_call(&ctx.call_id, &mut events, cancel_grace).await;
                }
                () = &mut timed_out => {
                    crate::plugin::cancel_stdio_tool_call(&self.manager, &self.plugin_name, &ctx.call_id, "tool call timed out").ok();
                    crate::plugin::abandon_stdio_tool_call(&self.manager, &self.plugin_name, &ctx.call_id).ok();
                    return ToolResult::error(format!(
                        "Plugin tool '{}' timed out waiting for stdio result",
                        self.definition.name,
                    ));
                }
                event = events.recv() => {
                    match event {
                        Some(StdioToolCallEvent::Progress(message)) => ctx.emit_progress(&message),
                        Some(StdioToolCallEvent::Result(content)) => return decode_stdio_result(content),
                        Some(StdioToolCallEvent::Error(message)) => return ToolResult::error(message),
                        Some(StdioToolCallEvent::Cancelled) => return ToolResult::error("Tool call cancelled"),
                        Some(StdioToolCallEvent::Disconnected(reason)) => return ToolResult::error(reason),
                        None => {
                            return ToolResult::error(format!(
                                "Plugin '{}' disconnected during tool call",
                                self.plugin_name,
                            ));
                        }
                    }
                }
            }
        }
    }

    async fn finish_cancelled_stdio_call(
        &self,
        call_id: &str,
        events: &mut tokio::sync::mpsc::UnboundedReceiver<StdioToolCallEvent>,
        cancel_grace: Duration,
    ) -> ToolResult {
        let cancelled = tokio::time::sleep(cancel_grace);
        tokio::pin!(cancelled);

        loop {
            tokio::select! {
                () = &mut cancelled => {
                    crate::plugin::abandon_stdio_tool_call(&self.manager, &self.plugin_name, call_id).ok();
                    return ToolResult::error("Tool call cancelled");
                }
                event = events.recv() => {
                    match event {
                        Some(StdioToolCallEvent::Progress(_)) => {}
                        Some(_) | None => {
                            crate::plugin::abandon_stdio_tool_call(&self.manager, &self.plugin_name, call_id).ok();
                            return ToolResult::error("Tool call cancelled");
                        }
                    }
                }
            }
        }
    }
}

fn decode_stdio_result(content: Value) -> ToolResult {
    if let Ok(result) = serde_json::from_value::<ToolResult>(content.clone()) {
        return result;
    }

    match content {
        Value::String(text) => ToolResult::text(text),
        other => ToolResult::text(serde_json::to_string_pretty(&other).unwrap_or_else(|_| other.to_string())),
    }
}

fn stdio_duration_override(key: &str, default: Duration) -> Duration {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_millis)
        .unwrap_or(default)
}

#[async_trait]
impl Tool for PluginTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    fn source(&self) -> &str {
        &self.plugin_name
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        match &self.backend {
            PluginToolBackend::Wasm { function_name } => self.execute_wasm(ctx, function_name, params),
            PluginToolBackend::Stdio => self.execute_stdio(ctx, params).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use tokio_util::sync::CancellationToken;

    use super::*;

    /// Build a PluginTool backed by the real test plugin WASM.
    fn build_test_tool(tool_name: &str) -> PluginTool {
        let plugins_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
        let mut mgr = PluginManager::new(plugins_dir, None);
        mgr.discover();
        mgr.load_wasm("clankers-test-plugin").expect("load test plugin");
        let manager = Arc::new(Mutex::new(mgr));

        PluginTool::new(
            ToolDefinition {
                name: tool_name.to_string(),
                description: format!("Test tool: {}", tool_name),
                input_schema: serde_json::json!({"type": "object"}),
            },
            "clankers-test-plugin".to_string(),
            "handle_tool_call".to_string(),
            manager,
        )
    }

    fn run(tool: &PluginTool, params: Value) -> ToolResult {
        let rt = tokio::runtime::Runtime::new().expect("should create runtime");
        rt.block_on(tool.execute(&ToolContext::new("call-1".to_string(), CancellationToken::new(), None), params))
    }

    // ── Envelope wrapping ────────────────────────────────────────

    #[test]
    fn execute_echo_wraps_params_in_envelope() {
        let tool = build_test_tool("test_echo");
        let result = run(&tool, serde_json::json!({"text": "hello world"}));
        assert!(!result.is_error, "Should not be error: {:?}", result);
        let text = match &result.content[0] {
            crate::tools::ToolResultContent::Text { text } => text.clone(),
            _ => panic!("Expected text content"),
        };
        assert_eq!(text, "hello world");
    }

    #[test]
    fn execute_reverse_wraps_params_in_envelope() {
        let tool = build_test_tool("test_reverse");
        let result = run(&tool, serde_json::json!({"text": "abcdef"}));
        assert!(!result.is_error);
        let text = match &result.content[0] {
            crate::tools::ToolResultContent::Text { text } => text.clone(),
            _ => panic!("Expected text content"),
        };
        assert_eq!(text, "fedcba");
    }

    #[test]
    fn execute_echo_empty_text() {
        let tool = build_test_tool("test_echo");
        let result = run(&tool, serde_json::json!({"text": ""}));
        assert!(!result.is_error);
        let text = match &result.content[0] {
            crate::tools::ToolResultContent::Text { text } => text.clone(),
            _ => panic!("Expected text content"),
        };
        assert_eq!(text, "");
    }

    #[test]
    fn execute_echo_unicode() {
        let tool = build_test_tool("test_echo");
        let result = run(&tool, serde_json::json!({"text": "🦀 Rust + WASM 🎉"}));
        assert!(!result.is_error);
        let text = match &result.content[0] {
            crate::tools::ToolResultContent::Text { text } => text.clone(),
            _ => panic!("Expected text content"),
        };
        assert_eq!(text, "🦀 Rust + WASM 🎉");
    }

    // ── Unknown tool returns error status ────────────────────────

    #[test]
    fn execute_unknown_tool_returns_error() {
        let tool = build_test_tool("nonexistent_tool");
        let result = run(&tool, serde_json::json!({}));
        // The plugin returns status: "unknown_tool", which is not "ok"
        assert!(result.is_error, "Unknown tool should be an error: {:?}", result);
    }

    // ── Missing text arg still works (empty) ─────────────────────

    #[test]
    fn execute_echo_missing_text_arg() {
        let tool = build_test_tool("test_echo");
        let result = run(&tool, serde_json::json!({}));
        assert!(!result.is_error);
        let text = match &result.content[0] {
            crate::tools::ToolResultContent::Text { text } => text.clone(),
            _ => panic!("Expected text content"),
        };
        assert_eq!(text, "");
    }

    // ── Definition is accessible ─────────────────────────────────

    #[test]
    fn definition_returns_correct_metadata() {
        let tool = build_test_tool("test_echo");
        let def = tool.definition();
        assert_eq!(def.name, "test_echo");
        assert!(def.description.contains("test_echo"));
    }

    // ── Plugin not loaded errors cleanly ─────────────────────────

    #[test]
    fn execute_unloaded_plugin_returns_error() {
        let plugins_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
        let mut mgr = PluginManager::new(plugins_dir, None);
        mgr.discover();
        // Don't load WASM — leave it in Loaded state
        let manager = Arc::new(Mutex::new(mgr));

        let tool = PluginTool::new(
            ToolDefinition {
                name: "test_echo".to_string(),
                description: "test".to_string(),
                input_schema: serde_json::json!({}),
            },
            "clankers-test-plugin".to_string(),
            "handle_tool_call".to_string(),
            manager,
        );

        let result = run(&tool, serde_json::json!({"text": "hi"}));
        assert!(result.is_error, "Unloaded plugin should error: {:?}", result);
        let text = match &result.content[0] {
            crate::tools::ToolResultContent::Text { text } => text.clone(),
            _ => panic!("Expected text content"),
        };
        assert!(text.contains("not loaded"), "Error should mention not loaded: {}", text);
    }

    // ── Multiple sequential calls ────────────────────────────────

    #[test]
    fn execute_multiple_calls_sequentially() {
        let tool = build_test_tool("test_echo");
        for i in 0..10 {
            let input = format!("call-{}", i);
            let result = run(&tool, serde_json::json!({"text": input}));
            assert!(!result.is_error);
            let text = match &result.content[0] {
                crate::tools::ToolResultContent::Text { text } => text.clone(),
                _ => panic!("Expected text content"),
            };
            assert_eq!(text, input);
        }
    }

    // ── clankers-hash plugin tool tests ─────────────────────────────────

    fn build_hash_tool(tool_name: &str) -> PluginTool {
        let plugins_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
        let mut mgr = PluginManager::new(plugins_dir, None);
        mgr.discover();
        mgr.load_wasm("clankers-hash").expect("load hash plugin");
        let manager = Arc::new(Mutex::new(mgr));

        PluginTool::new(
            ToolDefinition {
                name: tool_name.to_string(),
                description: format!("Hash tool: {}", tool_name),
                input_schema: serde_json::json!({"type": "object"}),
            },
            "clankers-hash".to_string(),
            "handle_tool_call".to_string(),
            manager,
        )
    }

    #[test]
    fn execute_hash_text_sha256() {
        let tool = build_hash_tool("hash_text");
        let result = run(&tool, serde_json::json!({"text": "test", "algorithm": "sha256"}));
        assert!(!result.is_error, "Should not be error: {:?}", result);
        let text = match &result.content[0] {
            crate::tools::ToolResultContent::Text { text } => text.clone(),
            _ => panic!("Expected text content"),
        };
        assert_eq!(text, "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08");
    }

    #[test]
    fn execute_hash_text_default_algorithm() {
        let tool = build_hash_tool("hash_text");
        let result = run(&tool, serde_json::json!({"text": "test"}));
        assert!(!result.is_error);
        let text = match &result.content[0] {
            crate::tools::ToolResultContent::Text { text } => text.clone(),
            _ => panic!("Expected text content"),
        };
        // Default is SHA-256
        assert_eq!(text, "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08");
    }

    #[test]
    fn execute_encode_text_base64() {
        let tool = build_hash_tool("encode_text");
        let result = run(&tool, serde_json::json!({"text": "hello", "encoding": "base64", "direction": "encode"}));
        assert!(!result.is_error, "Should not be error: {:?}", result);
        let text = match &result.content[0] {
            crate::tools::ToolResultContent::Text { text } => text.clone(),
            _ => panic!("Expected text content"),
        };
        assert_eq!(text, "aGVsbG8=");
    }

    #[test]
    fn execute_encode_text_hex() {
        let tool = build_hash_tool("encode_text");
        let result = run(&tool, serde_json::json!({"text": "hello", "encoding": "hex", "direction": "encode"}));
        assert!(!result.is_error);
        let text = match &result.content[0] {
            crate::tools::ToolResultContent::Text { text } => text.clone(),
            _ => panic!("Expected text content"),
        };
        assert_eq!(text, "68656c6c6f");
    }

    #[test]
    fn execute_hash_unknown_tool_returns_error() {
        let tool = build_hash_tool("nonexistent_tool");
        let result = run(&tool, serde_json::json!({}));
        assert!(result.is_error, "Unknown tool should be an error: {:?}", result);
    }

    #[test]
    fn execute_hash_multiple_calls() {
        let tool = build_hash_tool("hash_text");
        for i in 0..5 {
            let input = format!("input-{}", i);
            let result = run(&tool, serde_json::json!({"text": input, "algorithm": "sha256"}));
            assert!(!result.is_error, "Call {} should not error", i);
        }
    }
}
