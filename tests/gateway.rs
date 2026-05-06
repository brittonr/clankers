use clanker_message::ToolResultContent;
use clankers::tools::Tool;
use clankers::tools::ToolContext;
use clankers::tools::tool_gateway::ToolGatewayTool;
use serde_json::json;
use tokio_util::sync::CancellationToken;

#[test]
fn gateway_validates_local_delivery_and_rejects_remote_targets() {
    let toolsets = clankers::tool_gateway::parse_toolsets("core,specialty").expect("parse toolsets");

    let local = clankers::tool_gateway::validate(
        &toolsets,
        &clankers::tool_gateway::parse_delivery_target(Some("local")),
        false,
    );
    assert!(local.supported);
    assert_eq!(local.backend, "local");
    assert_eq!(local.delivery_target, "local");
    assert_eq!(local.toolsets, vec!["core".to_string(), "specialty".to_string()]);

    let remote = clankers::tool_gateway::validate(
        &toolsets,
        &clankers::tool_gateway::parse_delivery_target(Some("https://token@example.test/hook\nsecret")),
        false,
    );
    assert!(!remote.supported);
    assert_eq!(remote.status, "unsupported");
    assert_eq!(remote.delivery_target, "https");
    assert_eq!(remote.error_kind, Some("unsupported_target"));
    let message = remote.error_message.expect("error message");
    assert!(!message.contains("token@example.test"));
    assert!(!message.contains('\n'));
}

#[tokio::test]
async fn gateway_tool_returns_replay_safe_success_and_failure_details() {
    let tool = ToolGatewayTool::new();
    let ctx = ToolContext::new("gateway-integration".to_string(), CancellationToken::new(), None);

    let success = tool.execute(&ctx, json!({"action": "validate", "toolsets": "core", "deliver": "session"})).await;
    assert!(!success.is_error);
    assert!(text(&success).contains("validation succeeded"));
    let success_details = success.details.expect("success details");
    assert_eq!(success_details["source"], "tool_gateway");
    assert_eq!(success_details["delivery_target"], "session");
    assert_eq!(success_details["supported"], true);

    let failure = tool
        .execute(&ctx, json!({"action": "validate", "toolsets": "core", "deliver": "webhook://secret-host/path"}))
        .await;
    assert!(failure.is_error);
    assert!(!text(&failure).contains("secret-host"));
    let failure_details = failure.details.expect("failure details");
    assert_eq!(failure_details["delivery_target"], "webhook");
    assert_eq!(failure_details["supported"], false);
    assert_eq!(failure_details["error_kind"], "unsupported_target");
}

#[test]
fn gateway_delivery_receipts_are_platform_safe() {
    let receipt = clankers::tool_gateway::local_delivery_receipt(
        clankers::tool_gateway::ArtifactKind::ScheduledOutput,
        Some(std::path::Path::new("/tmp/token/schedule-result.json")),
        &clankers::tool_gateway::parse_delivery_target(Some("local")),
    );
    assert_eq!(receipt.status, "success");
    assert_eq!(receipt.artifact_type, "scheduled_output");
    assert_eq!(receipt.safe_path.as_deref(), Some("schedule-result.json"));
    assert!(!serde_json::to_string(&receipt).expect("serialize").contains("token"));

    let unsupported = clankers::tool_gateway::local_delivery_receipt(
        clankers::tool_gateway::ArtifactKind::File,
        Some(std::path::Path::new("/tmp/secret.txt")),
        &clankers::tool_gateway::parse_delivery_target(Some("webhook://secret-host/path")),
    );
    assert_eq!(unsupported.status, "unsupported");
    assert_eq!(unsupported.target_kind, "webhook");
    assert!(unsupported.safe_path.is_none());
    assert!(!serde_json::to_string(&unsupported).expect("serialize").contains("secret-host"));
}

#[tokio::test]
async fn gateway_tool_deliver_receipt_returns_safe_details() {
    let tool = ToolGatewayTool::new();
    let ctx = ToolContext::new("gateway-delivery".to_string(), CancellationToken::new(), None);
    let result = tool
        .execute(
            &ctx,
            json!({"action": "deliver_receipt", "artifact_type": "media", "path": "/tmp/secret/out.mp3", "deliver": "session"}),
        )
        .await;

    assert!(!result.is_error);
    let details = result.details.expect("delivery details");
    assert_eq!(details["artifact_type"], "media");
    assert_eq!(details["safe_path"], "out.mp3");
    assert!(!serde_json::to_string(&details).expect("serialize").contains("secret"));
}

fn text(result: &clankers::tools::ToolResult) -> &str {
    match result.content.first().expect("tool result content") {
        ToolResultContent::Text { text } => text,
        ToolResultContent::Image { .. } => panic!("unexpected image content"),
    }
}
