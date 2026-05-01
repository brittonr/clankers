use clanker_message::ToolResultContent;
use clankers::tools::Tool;
use clankers::tools::ToolContext;
use clankers::tools::voice_mode::VoiceModeTool;
use serde_json::json;
use tokio_util::sync::CancellationToken;

fn ctx() -> ToolContext {
    ToolContext::new("voice-integration-test".to_string(), CancellationToken::new(), None)
}

fn text(result: &clankers::tools::ToolResult) -> &str {
    match result.content.first().expect("content") {
        ToolResultContent::Text { text } => text,
        ToolResultContent::Image { .. } => panic!("unexpected image content"),
    }
}

#[test]
fn validates_supported_file_input_without_preserving_path() {
    let source = clankers::voice_mode::parse_input_source("/tmp/private/customer-call.wav");
    let validation = clankers::voice_mode::validate(&source, clankers::voice_mode::VoiceReplyMode::Text, false);

    assert!(validation.supported);
    assert_eq!(validation.input_kind, "file");
    assert_eq!(validation.input_label, "file:wav");
    assert!(!validation.input_label.contains("customer-call"));
}

#[test]
fn rejects_microphone_input_first_pass() {
    let source = clankers::voice_mode::parse_input_source("microphone");
    let validation = clankers::voice_mode::validate(&source, clankers::voice_mode::VoiceReplyMode::Text, false);

    assert!(!validation.supported);
    assert_eq!(validation.error_kind, Some("unsupported_input"));
    assert_eq!(validation.input_label, "microphone");
}

#[tokio::test]
async fn voice_mode_tool_returns_safe_details_for_success_and_failure() {
    let tool = VoiceModeTool::new();

    let success = tool
        .execute(&ctx(), json!({"action": "validate", "input": "/tmp/private/audio.mp3", "reply": "none"}))
        .await;
    assert!(!success.is_error);
    assert!(text(&success).contains("validation succeeded"));
    let details = success.details.expect("success details");
    assert_eq!(details["source"], "voice_mode");
    assert_eq!(details["input_label"], "file:mp3");
    assert_eq!(details["reply_mode"], "none");

    let failure = tool
        .execute(
            &ctx(),
            json!({"action": "validate", "input": "https://token@example.test/audio.wav\nsecret", "reply": "tts"}),
        )
        .await;
    assert!(failure.is_error);
    let output = text(&failure);
    assert!(!output.contains("token@example.test"));
    assert!(!output.contains('\n'));
    let details = failure.details.expect("failure details");
    assert_eq!(details["input_label"], "https");
    assert_eq!(details["supported"], false);
    assert_eq!(details["error_kind"], "unsupported_input");
}
