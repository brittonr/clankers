//! Model Context Protocol tool adapter.
//!
//! This module owns the clankers-facing adapter layer: it turns MCP tool
//! metadata into normal clankers `Tool` implementations and maps MCP tool
//! call results back into `ToolResult`. Transport-specific stdio/HTTP clients
//! can implement `McpRuntime` without changing tool publication semantics.

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use clankers_config::McpServerConfig;
use serde_json::Value;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;

#[derive(Debug, Clone, PartialEq)]
pub struct McpRegisteredTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

impl McpRegisteredTool {
    pub fn new(name: impl Into<String>, description: impl Into<String>, input_schema: Value) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
        }
    }
}

#[async_trait]
pub trait McpRuntime: Send + Sync {
    async fn call_tool(&self, server: &str, tool: &str, args: Value) -> Result<Value, String>;
}

pub struct McpTool {
    definition: ToolDefinition,
    server_name: String,
    mcp_tool_name: String,
    runtime: Arc<dyn McpRuntime>,
}

impl McpTool {
    pub fn new(
        server_name: impl Into<String>,
        mcp_tool_name: impl Into<String>,
        definition: ToolDefinition,
        runtime: Arc<dyn McpRuntime>,
    ) -> Self {
        Self {
            definition,
            server_name: server_name.into(),
            mcp_tool_name: mcp_tool_name.into(),
            runtime,
        }
    }
}

#[async_trait]
impl Tool for McpTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    fn source(&self) -> &str {
        &self.server_name
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        ctx.emit_progress(&format!("mcp: {}::{}", self.server_name, self.mcp_tool_name));
        match self.runtime.call_tool(&self.server_name, &self.mcp_tool_name, params).await {
            Ok(result) => mcp_result_to_tool_result(result),
            Err(error) => {
                ToolResult::error(format!("MCP tool error ({}::{}): {error}", self.server_name, self.mcp_tool_name))
            }
        }
    }
}

pub fn build_tools_for_server(
    server_name: &str,
    config: &McpServerConfig,
    registered_tools: &[McpRegisteredTool],
    seen_names: &mut HashSet<String>,
    runtime: Arc<dyn McpRuntime>,
) -> Vec<Arc<dyn Tool>> {
    if !config.enabled {
        return Vec::new();
    }

    let mut tools: Vec<Arc<dyn Tool>> = Vec::new();
    for registered in registered_tools {
        if !config.publishes_tool(&registered.name) {
            continue;
        }

        let visible_name = config.published_tool_name(server_name, &registered.name);
        if !seen_names.insert(visible_name.clone()) {
            tracing::warn!(
                server = server_name,
                mcp_tool = registered.name,
                visible_tool = visible_name,
                "skipping MCP tool because a tool with the same visible name is already registered"
            );
            continue;
        }

        let definition = ToolDefinition {
            name: visible_name,
            description: format!(
                "MCP tool '{}' from server '{}': {}",
                registered.name, server_name, registered.description
            ),
            input_schema: registered.input_schema.clone(),
        };
        tools.push(Arc::new(McpTool::new(
            server_name.to_string(),
            registered.name.clone(),
            definition,
            Arc::clone(&runtime),
        )));
    }
    tools
}

fn mcp_result_to_tool_result(result: Value) -> ToolResult {
    if result.get("isError").and_then(Value::as_bool).unwrap_or(false) {
        return ToolResult::error(extract_mcp_text(&result).unwrap_or_else(|| result.to_string()));
    }

    if let Some(text) = extract_mcp_text(&result) {
        ToolResult::text(text)
    } else {
        ToolResult::text(serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string()))
    }
}

fn extract_mcp_text(result: &Value) -> Option<String> {
    let content = result.get("content")?.as_array()?;
    let mut parts = Vec::new();
    for item in content {
        if item.get("type").and_then(Value::as_str) == Some("text")
            && let Some(text) = item.get("text").and_then(Value::as_str)
        {
            parts.push(text.to_string());
        }
    }
    if parts.is_empty() { None } else { Some(parts.join("\n")) }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tokio::sync::Mutex;

    use super::*;
    use crate::tools::ToolResultContent;

    struct FakeRuntime {
        calls: Mutex<Vec<(String, String, Value)>>,
        result: Value,
    }

    #[async_trait]
    impl McpRuntime for FakeRuntime {
        async fn call_tool(&self, server: &str, tool: &str, args: Value) -> Result<Value, String> {
            self.calls.lock().await.push((server.to_string(), tool.to_string(), args));
            Ok(self.result.clone())
        }
    }

    fn stdio_config() -> McpServerConfig {
        serde_json::from_value(json!({
            "transport": "stdio",
            "command": "fake-mcp",
            "includeTools": ["read_file", "write_file"],
            "excludeTools": ["write_file"],
            "toolPrefix": "fs"
        }))
        .unwrap()
    }

    #[test]
    fn build_tools_applies_filters_prefixes_and_collisions() {
        let config = stdio_config();
        let registered = vec![
            McpRegisteredTool::new("read_file", "Read a file", json!({"type":"object"})),
            McpRegisteredTool::new("write_file", "Write a file", json!({"type":"object"})),
            McpRegisteredTool::new("delete_file", "Delete a file", json!({"type":"object"})),
        ];
        let runtime = Arc::new(FakeRuntime {
            calls: Mutex::new(Vec::new()),
            result: json!({"content": []}),
        });
        let mut seen = HashSet::from(["fs_delete_file".to_string()]);

        let tools = build_tools_for_server("filesystem", &config, &registered, &mut seen, runtime);

        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].definition().name, "fs_read_file");
        assert!(seen.contains("fs_read_file"));
        assert!(!seen.contains("fs_write_file"));
    }

    #[test]
    fn disabled_server_publishes_no_tools() {
        let mut config = stdio_config();
        config.enabled = false;
        let runtime = Arc::new(FakeRuntime {
            calls: Mutex::new(Vec::new()),
            result: json!({"content": []}),
        });
        let mut seen = HashSet::new();
        let tools = build_tools_for_server(
            "filesystem",
            &config,
            &[McpRegisteredTool::new("read_file", "Read", json!({}))],
            &mut seen,
            runtime,
        );
        assert!(tools.is_empty());
    }

    #[tokio::test]
    async fn mcp_tool_executes_original_tool_name() {
        let runtime = Arc::new(FakeRuntime {
            calls: Mutex::new(Vec::new()),
            result: json!({"content": [{"type": "text", "text": "ok"}]}),
        });
        let definition = ToolDefinition {
            name: "fs_read_file".to_string(),
            description: "Read".to_string(),
            input_schema: json!({"type":"object"}),
        };
        let tool = McpTool::new("filesystem", "read_file", definition, runtime.clone());
        let ctx = ToolContext::new("call-1".to_string(), tokio_util::sync::CancellationToken::new(), None);

        let result = tool.execute(&ctx, json!({"path": "README.md"})).await;

        assert!(!result.is_error);
        assert_eq!(runtime.calls.lock().await[0].0, "filesystem");
        assert_eq!(runtime.calls.lock().await[0].1, "read_file");
        match &result.content[0] {
            ToolResultContent::Text { text } => assert_eq!(text, "ok"),
            ToolResultContent::Image { .. } => panic!("expected text"),
        }
    }

    #[test]
    fn mcp_error_result_maps_to_tool_error() {
        let result = mcp_result_to_tool_result(json!({
            "isError": true,
            "content": [{"type": "text", "text": "denied"}]
        }));
        assert!(result.is_error);
        match &result.content[0] {
            ToolResultContent::Text { text } => assert_eq!(text, "denied"),
            ToolResultContent::Image { .. } => panic!("expected text"),
        }
    }
}
