use clanker_message::ToolResultContent;
use clankers::tools::Tool;
use clankers::tools::ToolContext;
use clankers::tools::soul_personality::SoulPersonalityTool;
use serde_json::json;
use tokio_util::sync::CancellationToken;

fn ctx() -> ToolContext {
    ToolContext::new("soul-personality-integration-test".to_string(), CancellationToken::new(), None)
}

fn text(result: &clankers::tools::ToolResult) -> &str {
    match result.content.first().expect("content") {
        ToolResultContent::Text { text } => text,
        ToolResultContent::Image { .. } => panic!("unexpected image content"),
    }
}

#[test]
fn validates_supported_local_soul_without_preserving_full_path() {
    let source = clankers::soul_personality::parse_soul_source(Some("file:/tmp/private/customer/SOUL.md"));
    let personality = clankers::soul_personality::parse_personality(Some("mentor.v1")).expect("valid personality");
    let validation = clankers::soul_personality::validate(&source, personality.as_ref());

    assert!(validation.supported);
    assert_eq!(validation.source, "soul_personality");
    assert_eq!(validation.soul_kind, "local_file");
    assert_eq!(validation.soul_label, "SOUL.md");
    assert_eq!(validation.personality.as_deref(), Some("mentor.v1"));
    assert!(!validation.soul_label.contains("customer"));
}

#[test]
fn rejects_command_source_first_pass() {
    let source = clankers::soul_personality::parse_soul_source(Some("cmd:cat /tmp/private/SOUL.md"));
    let validation = clankers::soul_personality::validate(&source, None);

    assert!(!validation.supported);
    assert_eq!(validation.soul_kind, "command");
    assert_eq!(validation.soul_label, "command");
    assert_eq!(validation.error_kind, Some("unsupported_source"));
}

#[tokio::test]
async fn soul_personality_tool_returns_safe_details_for_success_and_failure() {
    let tool = SoulPersonalityTool::new();

    let success = tool
        .execute(&ctx(), json!({"action": "validate", "soul": "/tmp/private/SOUL.md", "personality": "concise"}))
        .await;
    assert!(!success.is_error);
    assert!(text(&success).contains("validation succeeded"));
    let details = success.details.expect("success details");
    assert_eq!(details["source"], "soul_personality");
    assert_eq!(details["soul_label"], "SOUL.md");
    assert_eq!(details["personality"], "concise");

    let failure = tool
        .execute(&ctx(), json!({"action": "validate", "soul": "https://token@example.test/SOUL.md\nsecret"}))
        .await;
    assert!(failure.is_error);
    let output = text(&failure);
    assert!(!output.contains("token@example.test"));
    assert!(!output.contains('\n'));
    let details = failure.details.expect("failure details");
    assert_eq!(details["soul_label"], "https");
    assert_eq!(details["supported"], false);
    assert_eq!(details["error_kind"], "unsupported_source");
}
