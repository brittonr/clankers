use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use clankers::modes::common::ToolEnv;
use clankers::modes::common::ToolSet;
use clankers::modes::common::ToolTier;
use clankers::tools::ToolResultContent;
use clankers::tools::mcp::McpRegisteredTool;
use clankers::tools::mcp::McpRuntime;
use clankers::tools::mcp::McpRuntimeRegistry;
use clankers_controller::client::ClientAdapter;
use clankers_protocol::DaemonEvent;
use clankers_protocol::SessionCommand;
use clankers_protocol::frame;
use clankers_protocol::types::Handshake;
use clankers_protocol::types::PROTOCOL_VERSION;
use serde_json::Value;
use serde_json::json;
use tokio::net::UnixListener;
use tokio::net::UnixStream;
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

#[tokio::test]
async fn session_control_bridge_round_trips_command_and_event_evidence_over_socket() {
    let temp = tempfile::tempdir().expect("tempdir");
    let socket_path = temp.path().join("session.sock");
    let listener = UnixListener::bind(&socket_path).expect("bind session socket");

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept mcp client");
        let (mut reader, mut writer) = tokio::io::split(stream);

        let handshake: Handshake = frame::read_frame(&mut reader).await.expect("handshake");
        assert_eq!(handshake.protocol_version, PROTOCOL_VERSION);
        assert_eq!(handshake.client_name, "clankers-mcp-session-control-test");
        assert_eq!(handshake.session_id.as_deref(), Some("sess-integration"));

        frame::write_frame(&mut writer, &DaemonEvent::HistoryBlock {
            block: json!({"role": "user", "content": "raw session text must not leak"}),
        })
        .await
        .expect("history block event");
        frame::write_frame(&mut writer, &DaemonEvent::HistoryEnd).await.expect("history end event");

        let command: SessionCommand = frame::read_frame(&mut reader).await.expect("session command");
        assert!(matches!(command, SessionCommand::ReplayHistory));
    });

    let stream = UnixStream::connect(&socket_path).await.expect("connect session socket");
    let mut client =
        ClientAdapter::connect(stream, "clankers-mcp-session-control-test", None, Some("sess-integration".to_string()))
            .await
            .expect("client adapter");
    tokio::time::sleep(Duration::from_millis(10)).await;

    let response = clankers::commands::mcp::handle_json_line_for_client(
        r#"{"id":42,"method":"tools/call","params":{"name":"session_history","arguments":{}}}"#,
        Some("sess-integration"),
        &mut client,
    )
    .expect("mcp response");
    let value: Value = serde_json::from_str(&response).expect("response json");

    assert_eq!(value["result"]["receipt"]["status"], "accepted");
    assert_eq!(value["result"]["receipt"]["command"], json!("ReplayHistory"));
    assert_eq!(value["result"]["receipt"]["evidence"]["event_count"], 2);
    assert_eq!(value["result"]["receipt"]["evidence"]["events"][0]["type"], "HistoryBlock");
    assert_eq!(value["result"]["receipt"]["evidence"]["events"][1]["type"], "HistoryEnd");
    assert!(response.contains("block_bytes"));
    assert!(!response.contains("raw session text must not leak"));

    server.await.expect("server task");
}

#[tokio::test]
async fn session_control_bridge_handles_status_and_protocol_errors_over_socket() {
    let temp = tempfile::tempdir().expect("tempdir");
    let socket_path = temp.path().join("session-status.sock");
    let listener = UnixListener::bind(&socket_path).expect("bind session socket");

    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept mcp client");
        let (mut reader, _writer) = tokio::io::split(stream);
        let handshake: Handshake = frame::read_frame(&mut reader).await.expect("handshake");
        assert_eq!(handshake.client_name, "clankers-mcp-session-control-test");

        let command =
            tokio::time::timeout(Duration::from_millis(30), frame::read_frame::<_, SessionCommand>(&mut reader)).await;
        assert!(command.is_err(), "read-only and invalid MCP requests must not submit session commands");
    });

    let stream = UnixStream::connect(&socket_path).await.expect("connect session socket");
    let mut client =
        ClientAdapter::connect(stream, "clankers-mcp-session-control-test", None, Some("sess-status".to_string()))
            .await
            .expect("client adapter");

    let status_response = clankers::commands::mcp::handle_json_line_for_client(
        r#"{"id":1,"method":"tools/call","params":{"name":"session_status","arguments":{}}}"#,
        Some("sess-status"),
        &mut client,
    )
    .expect("status response");
    let status: Value = serde_json::from_str(&status_response).expect("status json");
    assert_eq!(status["result"]["receipt"]["status"], "ok");
    assert_eq!(status["result"]["receipt"]["read_only"]["action"], "session_status");

    let error_response = clankers::commands::mcp::handle_json_line_for_client(
        r#"{"id":2,"method":"private/mutate_app","params":{}}"#,
        Some("sess-status"),
        &mut client,
    )
    .expect("error response");
    let error: Value = serde_json::from_str(&error_response).expect("error json");
    assert_eq!(error["error"]["code"], -32601);
    assert_eq!(error["error"]["data"]["source"], "mcp_session_control");

    server.await.expect("server task");
}
