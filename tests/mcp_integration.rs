use std::sync::Arc;

use async_trait::async_trait;
use clankers::modes::common::ToolEnv;
use clankers::modes::common::ToolSet;
use clankers::modes::common::ToolTier;
use clankers::tools::ToolResultContent;
use clankers::tools::mcp::McpRegisteredTool;
use clankers::tools::mcp::McpRuntime;
use clankers::tools::mcp::McpRuntimeRegistry;
use serde_json::Value;
use serde_json::json;
use tokio::sync::Mutex;

struct IntegrationMcpRegistry {
    calls: Mutex<Vec<(String, String, Value)>>,
    fail: bool,
}

#[async_trait]
impl McpRuntime for IntegrationMcpRegistry {
    async fn call_tool(&self, server: &str, tool: &str, args: Value) -> Result<Value, String> {
        self.calls.lock().await.push((server.to_string(), tool.to_string(), args));
        if self.fail {
            Err("configured failure".to_string())
        } else {
            Ok(json!({"content": [{"type": "text", "text": "from mcp"}]}))
        }
    }
}

impl McpRuntimeRegistry for IntegrationMcpRegistry {
    fn registered_tools(&self, server: &str) -> Vec<McpRegisteredTool> {
        if server == "filesystem" {
            vec![McpRegisteredTool::new(
                "read_file",
                "Read file via MCP",
                json!({"type": "object"}),
            )]
        } else {
            Vec::new()
        }
    }
}

#[tokio::test]
async fn configured_mcp_tool_is_available_and_executes() {
    let mut settings = clankers::config::settings::Settings::default();
    settings.mcp = serde_json::from_value(json!({
        "servers": {
            "filesystem": {"transport": "stdio", "command": "fake-mcp", "toolPrefix": "fs"}
        }
    }))
    .expect("settings should parse");
    let registry = Arc::new(IntegrationMcpRegistry {
        calls: Mutex::new(Vec::new()),
        fail: false,
    });
    let env = ToolEnv {
        settings: Some(settings),
        mcp_registry: Some(registry.clone()),
        ..Default::default()
    };

    let tiered = clankers::modes::common::build_all_tiered_tools(&env, None);
    let tool_set = ToolSet::new(tiered, [ToolTier::Specialty]);
    let tools = tool_set.active_tools();
    let tool = tools
        .iter()
        .find(|tool| tool.definition().name == "fs_read_file")
        .expect("MCP tool should be published");
    let ctx = clankers::tools::ToolContext::new("call-1".to_string(), tokio_util::sync::CancellationToken::new(), None);

    let result = tool.execute(&ctx, json!({"path": "README.md"})).await;

    assert!(!result.is_error);
    assert_eq!(registry.calls.lock().await[0].1, "read_file");
    assert_eq!(
        result.details.as_ref().and_then(|details| details.get("visible_tool")).and_then(Value::as_str),
        Some("fs_read_file")
    );
    match &result.content[0] {
        ToolResultContent::Text { text } => assert_eq!(text, "from mcp"),
        ToolResultContent::Image { .. } => panic!("expected text result"),
    }
}

#[tokio::test]
async fn configured_mcp_tool_failure_is_actionable() {
    let mut settings = clankers::config::settings::Settings::default();
    settings.mcp = serde_json::from_value(json!({
        "servers": {
            "filesystem": {"transport": "stdio", "command": "fake-mcp", "toolPrefix": "fs"}
        }
    }))
    .expect("settings should parse");
    let registry = Arc::new(IntegrationMcpRegistry {
        calls: Mutex::new(Vec::new()),
        fail: true,
    });
    let env = ToolEnv {
        settings: Some(settings),
        mcp_registry: Some(registry),
        ..Default::default()
    };

    let tiered = clankers::modes::common::build_all_tiered_tools(&env, None);
    let tool_set = ToolSet::new(tiered, [ToolTier::Specialty]);
    let tools = tool_set.active_tools();
    let tool = tools
        .iter()
        .find(|tool| tool.definition().name == "fs_read_file")
        .expect("MCP tool should be published");
    let ctx = clankers::tools::ToolContext::new("call-2".to_string(), tokio_util::sync::CancellationToken::new(), None);

    let result = tool.execute(&ctx, json!({"path": "README.md"})).await;

    assert!(result.is_error);
    assert_eq!(
        result.details.as_ref().and_then(|details| details.get("source")).and_then(Value::as_str),
        Some("mcp")
    );
    match &result.content[0] {
        ToolResultContent::Text { text } => assert!(text.contains("MCP tool error (filesystem::read_file)")),
        ToolResultContent::Image { .. } => panic!("expected text error"),
    }
}
