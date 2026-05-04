//! Model Context Protocol tool adapter.
//!
//! This module owns the clankers-facing adapter layer: it turns MCP tool
//! metadata into normal clankers `Tool` implementations and maps MCP tool
//! call results back into `ToolResult`. Transport-specific stdio/HTTP clients
//! can implement `McpRuntime` without changing tool publication semantics.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use async_trait::async_trait;
use clankers_config::McpServerConfig;
use serde_json::Value;
use tokio::time::timeout;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpRuntimeState {
    Healthy,
    Unavailable,
}

impl McpRuntimeState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Unavailable => "unavailable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpRuntimeStatus {
    pub state: McpRuntimeState,
    pub message: Option<String>,
}

impl McpRuntimeStatus {
    pub fn healthy() -> Self {
        Self {
            state: McpRuntimeState::Healthy,
            message: None,
        }
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            state: McpRuntimeState::Unavailable,
            message: Some(message.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpCallReceipt {
    pub server: String,
    pub visible_tool: String,
    pub mcp_tool: String,
    pub status: String,
    pub duration_ms: u128,
    pub error_class: Option<String>,
}

impl McpCallReceipt {
    fn new(
        server: &str,
        visible_tool: &str,
        mcp_tool: &str,
        status: impl Into<String>,
        duration_ms: u128,
        error_class: Option<&str>,
    ) -> Self {
        Self {
            server: server.to_string(),
            visible_tool: visible_tool.to_string(),
            mcp_tool: mcp_tool.to_string(),
            status: status.into(),
            duration_ms,
            error_class: error_class.map(str::to_string),
        }
    }

    fn to_details(&self, runtime_state: McpRuntimeState) -> Value {
        serde_json::json!({
            "source": "mcp",
            "server": self.server,
            "mcp_tool": self.mcp_tool,
            "visible_tool": self.visible_tool,
            "runtime_state": runtime_state.as_str(),
            "receipt": {
                "server": self.server,
                "visible_tool": self.visible_tool,
                "mcp_tool": self.mcp_tool,
                "status": self.status,
                "duration_ms": self.duration_ms,
                "error_class": self.error_class,
            }
        })
    }
}

#[async_trait]
pub trait McpRuntime: Send + Sync {
    async fn call_tool(&self, server: &str, tool: &str, args: Value) -> Result<Value, String>;
}

pub trait McpRuntimeRegistry: McpRuntime {
    fn registered_tools(&self, server: &str) -> Vec<McpRegisteredTool>;

    fn runtime_status(&self, _server: &str) -> McpRuntimeStatus {
        McpRuntimeStatus::healthy()
    }
}

pub struct McpTool {
    definition: ToolDefinition,
    server_name: String,
    mcp_tool_name: String,
    timeout_ms: Option<u64>,
    runtime: Arc<dyn McpRuntimeRegistry>,
}

impl McpTool {
    pub fn new(
        server_name: impl Into<String>,
        mcp_tool_name: impl Into<String>,
        definition: ToolDefinition,
        timeout_ms: Option<u64>,
        runtime: Arc<dyn McpRuntimeRegistry>,
    ) -> Self {
        Self {
            definition,
            server_name: server_name.into(),
            mcp_tool_name: mcp_tool_name.into(),
            timeout_ms,
            runtime,
        }
    }

    fn receipt(&self, status: impl Into<String>, started_at: Instant, error_class: Option<&str>) -> McpCallReceipt {
        McpCallReceipt::new(
            &self.server_name,
            &self.definition.name,
            &self.mcp_tool_name,
            status,
            started_at.elapsed().as_millis(),
            error_class,
        )
    }

    fn schema_is_current(&self) -> bool {
        self.runtime
            .registered_tools(&self.server_name)
            .into_iter()
            .find(|registered| registered.name == self.mcp_tool_name)
            .is_some_and(|registered| registered.input_schema == self.definition.input_schema)
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
        let started_at = Instant::now();
        let runtime_status = self.runtime.runtime_status(&self.server_name);
        if runtime_status.state != McpRuntimeState::Healthy {
            let details = self.receipt("runtime_unavailable", started_at, Some("runtime_unavailable"));
            let message = runtime_status.message.unwrap_or_else(|| "runtime unavailable".to_string());
            return ToolResult::error(format!("MCP runtime unavailable ({}): {message}", self.server_name))
                .with_details(details.to_details(runtime_status.state));
        }
        if !self.schema_is_current() {
            let details = self.receipt("schema_drift", started_at, Some("schema_drift"));
            return ToolResult::error(format!(
                "MCP schema drift ({}::{}): published tool schema changed; refresh the tool catalog before retrying",
                self.server_name, self.mcp_tool_name
            ))
            .with_details(details.to_details(runtime_status.state));
        }

        let call = self.runtime.call_tool(&self.server_name, &self.mcp_tool_name, params);
        let outcome = if let Some(timeout_ms) = self.timeout_ms {
            tokio::select! {
                () = ctx.signal.cancelled() => Err("cancelled".to_string()),
                result = timeout(Duration::from_millis(timeout_ms), call) => {
                    result.unwrap_or_else(|_| Err("timeout".to_string()))
                }
            }
        } else {
            tokio::select! {
                () = ctx.signal.cancelled() => Err("cancelled".to_string()),
                result = call => result,
            }
        };

        match outcome {
            Ok(result) => {
                let details = self.receipt("ok", started_at, None);
                mcp_result_to_tool_result(result).with_details(details.to_details(runtime_status.state))
            }
            Err(error) => {
                let error_class = classify_mcp_error(&error);
                let details = self.receipt(error_class, started_at, Some(error_class));
                ToolResult::error(format!("MCP tool error ({}::{}): {error}", self.server_name, self.mcp_tool_name))
                    .with_details(details.to_details(runtime_status.state))
            }
        }
    }
}

pub fn build_tools_for_server(
    server_name: &str,
    config: &McpServerConfig,
    registered_tools: &[McpRegisteredTool],
    seen_names: &mut HashSet<String>,
    runtime: Arc<dyn McpRuntimeRegistry>,
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
            config.timeout_ms,
            Arc::clone(&runtime),
        )));
    }
    tools
}

pub fn build_tools_from_settings(
    settings: &clankers_config::McpSettings,
    seen_names: &mut HashSet<String>,
    registry: Arc<dyn McpRuntimeRegistry>,
) -> Vec<Arc<dyn Tool>> {
    let mut tools = Vec::new();
    for (server_name, config) in &settings.servers {
        if let Err(error) = config.validate() {
            tracing::warn!(server = server_name, error = %error, "skipping invalid MCP server configuration");
            continue;
        }
        let registered = registry.registered_tools(server_name);
        tracing::info!(
            server = server_name,
            registered_tools = registered.len(),
            transport = ?config.transport,
            "discovered MCP server tools"
        );
        tools.extend(build_tools_for_server(server_name, config, &registered, seen_names, Arc::clone(&registry)));
    }
    tools
}

fn classify_mcp_error(error: &str) -> &'static str {
    match error {
        "cancelled" => "cancelled",
        "timeout" => "timeout",
        _ => "runtime_error",
    }
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
    use tokio::time::sleep;

    use super::*;
    use crate::tools::ToolResultContent;

    struct FakeRuntime {
        calls: Mutex<Vec<(String, String, Value)>>,
        result: Value,
        tools: Vec<McpRegisteredTool>,
        status: McpRuntimeStatus,
    }

    impl FakeRuntime {
        fn healthy(result: Value) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                result,
                tools: vec![McpRegisteredTool::new(
                    "read_file",
                    "Read a file",
                    json!({"type":"object"}),
                )],
                status: McpRuntimeStatus::healthy(),
            }
        }
    }

    #[async_trait]
    impl McpRuntime for FakeRuntime {
        async fn call_tool(&self, server: &str, tool: &str, args: Value) -> Result<Value, String> {
            self.calls.lock().await.push((server.to_string(), tool.to_string(), args));
            Ok(self.result.clone())
        }
    }

    impl McpRuntimeRegistry for FakeRuntime {
        fn registered_tools(&self, _server: &str) -> Vec<McpRegisteredTool> {
            self.tools.clone()
        }

        fn runtime_status(&self, _server: &str) -> McpRuntimeStatus {
            self.status.clone()
        }
    }

    struct SlowRuntime;

    #[async_trait]
    impl McpRuntime for SlowRuntime {
        async fn call_tool(&self, _server: &str, _tool: &str, _args: Value) -> Result<Value, String> {
            sleep(Duration::from_millis(50)).await;
            Ok(json!({"content": [{"type": "text", "text": "late"}]}))
        }
    }

    impl McpRuntimeRegistry for SlowRuntime {
        fn registered_tools(&self, _server: &str) -> Vec<McpRegisteredTool> {
            vec![McpRegisteredTool::new(
                "read_file",
                "Read a file",
                json!({"type":"object"}),
            )]
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
        let runtime = Arc::new(FakeRuntime::healthy(json!({"content": []})));
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
        let runtime = Arc::new(FakeRuntime::healthy(json!({"content": []})));
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
        let runtime = Arc::new(FakeRuntime::healthy(json!({"content": [{"type": "text", "text": "ok"}]})));
        let definition = ToolDefinition {
            name: "fs_read_file".to_string(),
            description: "Read".to_string(),
            input_schema: json!({"type":"object"}),
        };
        let tool = McpTool::new("filesystem", "read_file", definition, None, runtime.clone());
        let ctx = ToolContext::new("call-1".to_string(), tokio_util::sync::CancellationToken::new(), None);

        let result = tool.execute(&ctx, json!({"path": "README.md"})).await;

        assert!(!result.is_error);
        assert_eq!(
            result.details.as_ref().and_then(|details| details.get("source")).and_then(Value::as_str),
            Some("mcp")
        );
        assert_eq!(
            result.details.as_ref().and_then(|details| details.get("server")).and_then(Value::as_str),
            Some("filesystem")
        );
        assert_eq!(runtime.calls.lock().await[0].0, "filesystem");
        assert_eq!(runtime.calls.lock().await[0].1, "read_file");
        match &result.content[0] {
            ToolResultContent::Text { text } => assert_eq!(text, "ok"),
            ToolResultContent::Image { .. } => panic!("expected text"),
        }
    }

    #[tokio::test]
    async fn mcp_receipt_excludes_arguments_and_secrets() {
        let runtime = Arc::new(FakeRuntime::healthy(json!({"content": [{"type": "text", "text": "ok"}]})));
        let definition = ToolDefinition {
            name: "fs_read_file".to_string(),
            description: "Read".to_string(),
            input_schema: json!({"type":"object"}),
        };
        let tool = McpTool::new("filesystem", "read_file", definition, None, runtime.clone());
        let ctx = ToolContext::new("call-secret".to_string(), tokio_util::sync::CancellationToken::new(), None);

        let result = tool.execute(&ctx, json!({"token": "***", "path": "README.md"})).await;

        let details = result.details.as_ref().expect("receipt details");
        assert_eq!(details["receipt"]["status"], "ok");
        assert_eq!(details["receipt"]["server"], "filesystem");
        assert_eq!(details["receipt"]["mcp_tool"], "read_file");
        let details_text = details.to_string();
        assert!(!details_text.contains("s3cr3t-value"));
        assert!(!details_text.contains("README.md"));
    }

    #[tokio::test]
    async fn unavailable_runtime_is_isolated_before_call() {
        let runtime = Arc::new(FakeRuntime {
            status: McpRuntimeStatus::unavailable("spawn failed"),
            ..FakeRuntime::healthy(json!({"content": []}))
        });
        let definition = ToolDefinition {
            name: "fs_read_file".to_string(),
            description: "Read".to_string(),
            input_schema: json!({"type":"object"}),
        };
        let tool = McpTool::new("filesystem", "read_file", definition, None, runtime.clone());
        let ctx = ToolContext::new("call-unavailable".to_string(), tokio_util::sync::CancellationToken::new(), None);

        let result = tool.execute(&ctx, json!({"path": "README.md"})).await;

        assert!(result.is_error);
        assert!(runtime.calls.lock().await.is_empty());
        let details = result.details.as_ref().expect("receipt details");
        assert_eq!(details["runtime_state"], "unavailable");
        assert_eq!(details["receipt"]["status"], "runtime_unavailable");
        assert_eq!(details["receipt"]["error_class"], "runtime_unavailable");
    }

    #[tokio::test]
    async fn schema_drift_rejects_before_calling_runtime() {
        let runtime = Arc::new(FakeRuntime {
            tools: vec![McpRegisteredTool::new(
                "read_file",
                "Read a file",
                json!({"type":"object", "required": ["path"]}),
            )],
            ..FakeRuntime::healthy(json!({"content": []}))
        });
        let definition = ToolDefinition {
            name: "fs_read_file".to_string(),
            description: "Read".to_string(),
            input_schema: json!({"type":"object"}),
        };
        let tool = McpTool::new("filesystem", "read_file", definition, None, runtime.clone());
        let ctx = ToolContext::new("call-drift".to_string(), tokio_util::sync::CancellationToken::new(), None);

        let result = tool.execute(&ctx, json!({"path": "README.md"})).await;

        assert!(result.is_error);
        assert!(runtime.calls.lock().await.is_empty());
        let details = result.details.as_ref().expect("receipt details");
        assert_eq!(details["receipt"]["status"], "schema_drift");
        assert_eq!(details["receipt"]["error_class"], "schema_drift");
    }

    #[tokio::test]
    async fn timeout_is_reported_as_safe_receipt_error() {
        let runtime = Arc::new(SlowRuntime);
        let definition = ToolDefinition {
            name: "fs_read_file".to_string(),
            description: "Read".to_string(),
            input_schema: json!({"type":"object"}),
        };
        let tool = McpTool::new("filesystem", "read_file", definition, Some(1), runtime);
        let ctx = ToolContext::new("call-timeout".to_string(), tokio_util::sync::CancellationToken::new(), None);

        let result = tool.execute(&ctx, json!({"path": "README.md"})).await;

        assert!(result.is_error);
        let details = result.details.as_ref().expect("receipt details");
        assert_eq!(details["receipt"]["status"], "timeout");
        assert_eq!(details["receipt"]["error_class"], "timeout");
        assert!(!details.to_string().contains("README.md"));
    }

    #[tokio::test]
    async fn cancellation_is_reported_as_safe_receipt_error() {
        let runtime = Arc::new(SlowRuntime);
        let definition = ToolDefinition {
            name: "fs_read_file".to_string(),
            description: "Read".to_string(),
            input_schema: json!({"type":"object"}),
        };
        let tool = McpTool::new("filesystem", "read_file", definition, None, runtime);
        let signal = tokio_util::sync::CancellationToken::new();
        signal.cancel();
        let ctx = ToolContext::new("call-cancel".to_string(), signal, None);

        let result = tool.execute(&ctx, json!({"path": "README.md"})).await;

        assert!(result.is_error);
        let details = result.details.as_ref().expect("receipt details");
        assert_eq!(details["receipt"]["status"], "cancelled");
        assert_eq!(details["receipt"]["error_class"], "cancelled");
        assert!(!details.to_string().contains("README.md"));
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
