//! Voice/STT validation tool.
//!
//! First-pass voice support is validation-only. The tool validates local file
//! input intent and reply-mode policy without recording microphone input,
//! reading audio bytes, or contacting transcription providers.

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;
use crate::voice_mode;

pub struct VoiceModeTool {
    definition: ToolDefinition,
}

impl VoiceModeTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "voice_mode".to_string(),
                description: "Inspect and validate first-pass voice/STT policy.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["status", "validate", "start_capture", "stop_capture", "submit_transcript"],
                            "description": "Voice/STT action to perform"
                        },
                        "input": {
                            "type": "string",
                            "description": "Input source to validate: local path, file:<PATH>, microphone, matrix, remote:*, http(s)://*, or cloud:*"
                        },
                        "reply": {
                            "type": "string",
                            "enum": ["text", "tts", "none"],
                            "description": "Reply mode to validate"
                        },
                        "matrix_active": {
                            "type": "boolean",
                            "description": "True only inside an active Matrix voice bridge context"
                        },
                        "enabled": {"type": "boolean", "description": "Explicitly enable live capture for start_capture"},
                        "auto_submit": {"type": "boolean", "description": "Route accepted transcript into the session prompt path automatically"},
                        "transcript": {"type": "string", "description": "Accepted STT transcript for submit_transcript"}
                    },
                    "required": ["action"]
                }),
            },
        }
    }
}

impl Default for VoiceModeTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for VoiceModeTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, _ctx: &ToolContext, params: Value) -> ToolResult {
        match params.get("action").and_then(|value| value.as_str()) {
            Some("status") => validation_result(voice_mode::status_summary()),
            Some("validate") => validate_params(&params),
            Some("start_capture") => capture_params(&params, true),
            Some("stop_capture") => capture_params(&params, false),
            Some("submit_transcript") => transcript_params(&params),
            Some(other) => ToolResult::error(format!("Unknown voice_mode action: {other}")),
            None => ToolResult::error("Missing required parameter: action"),
        }
    }
}

fn validate_params(params: &Value) -> ToolResult {
    let input = match params.get("input").and_then(|value| value.as_str()) {
        Some(value) => value,
        None => return ToolResult::error("Missing required parameter for validate: input"),
    };
    let reply = match voice_mode::parse_reply_mode(params.get("reply").and_then(|value| value.as_str())) {
        Ok(mode) => mode,
        Err(message) => return ToolResult::error(message),
    };
    let source = voice_mode::parse_input_source(input);
    let is_matrix_active = params.get("matrix_active").and_then(|value| value.as_bool()).unwrap_or(false);
    validation_result(voice_mode::validate(&source, reply, is_matrix_active))
}

fn capture_params(params: &Value, start: bool) -> ToolResult {
    let input = params.get("input").and_then(|value| value.as_str()).unwrap_or("microphone");
    let reply = match voice_mode::parse_reply_mode(params.get("reply").and_then(|value| value.as_str())) {
        Ok(mode) => mode,
        Err(message) => return ToolResult::error(message),
    };
    let policy = voice_mode::VoiceCapturePolicy {
        enabled: params.get("enabled").and_then(|value| value.as_bool()).unwrap_or(false),
        provider: voice_mode::SttProviderPolicy::LocalFake,
        retain_audio: false,
        auto_submit: params.get("auto_submit").and_then(|value| value.as_bool()).unwrap_or(false),
    };
    let request = voice_mode::VoiceCaptureRequest {
        session_id: params.get("session_id").and_then(|value| value.as_str()).map(ToOwned::to_owned),
        source: voice_mode::parse_input_source(input),
        reply_mode: reply,
    };
    let receipt = if start {
        voice_mode::start_capture(&policy, request)
    } else {
        voice_mode::stop_capture(&policy, request)
    };
    capture_result(receipt)
}

fn transcript_params(params: &Value) -> ToolResult {
    let transcript = match params.get("transcript").and_then(|value| value.as_str()) {
        Some(value) => value,
        None => return ToolResult::error("Missing required parameter for submit_transcript: transcript"),
    };
    let reply = match voice_mode::parse_reply_mode(params.get("reply").and_then(|value| value.as_str())) {
        Ok(mode) => mode,
        Err(message) => return ToolResult::error(message),
    };
    let should_auto_submit = params.get("auto_submit").and_then(|value| value.as_bool()).unwrap_or(false);
    match voice_mode::session_prompt_from_transcript(transcript, reply, should_auto_submit) {
        Ok(prompt) => prompt_result(prompt),
        Err(message) => ToolResult::error(message),
    }
}

fn validation_result(validation: voice_mode::VoiceValidation) -> ToolResult {
    let details = serde_json::to_value(&validation).unwrap_or_else(|_| json!({"source": "voice_mode"}));
    let text = if validation.supported {
        format!(
            "Voice mode validation succeeded: {} input via {} (reply: {})",
            validation.input_label, validation.backend, validation.reply_mode
        )
    } else {
        format!(
            "Voice mode validation unsupported: {} ({})",
            validation.input_label,
            validation.error_message.as_deref().unwrap_or("unsupported input")
        )
    };
    let result = if validation.supported {
        ToolResult::text(text)
    } else {
        ToolResult::error(text)
    };
    result.with_details(details)
}

fn capture_result(receipt: voice_mode::VoiceCaptureReceipt) -> ToolResult {
    let details = serde_json::to_value(&receipt).unwrap_or_else(|_| json!({"source": "voice_mode"}));
    let text = format!("Voice capture {}: {} input via {}", receipt.status, receipt.input_label, receipt.backend);
    let result = if receipt.error_kind.is_some() {
        ToolResult::error(text)
    } else {
        ToolResult::text(text)
    };
    result.with_details(details)
}

fn prompt_result(prompt: voice_mode::VoiceSessionPrompt) -> ToolResult {
    let details = json!({
        "source": prompt.source,
        "action": prompt.action,
        "status": prompt.status,
        "reply_mode": prompt.reply_mode,
        "auto_submit": prompt.auto_submit,
        "transcript_chars": prompt.transcript_chars,
        "transcript_digest": prompt.transcript_digest,
    });
    ToolResult::text(format!(
        "Voice transcript {} for session prompt ({} chars, reply: {})",
        prompt.status, prompt.transcript_chars, prompt.reply_mode
    ))
    .with_details(details)
}

#[cfg(test)]
mod tests {
    use clanker_message::ToolResultContent;
    use serde_json::json;
    use tokio_util::sync::CancellationToken;

    use super::*;

    fn ctx() -> ToolContext {
        ToolContext::new("voice-mode-test".to_string(), CancellationToken::new(), None)
    }

    fn text(result: &ToolResult) -> &str {
        match result.content.first().expect("content") {
            ToolResultContent::Text { text } => text,
            ToolResultContent::Image { .. } => panic!("unexpected image content"),
        }
    }

    #[tokio::test]
    async fn validate_returns_safe_details_for_supported_file_input() {
        let tool = VoiceModeTool::new();
        let result = tool
            .execute(&ctx(), json!({"action": "validate", "input": "/tmp/private/audio.wav", "reply": "text"}))
            .await;

        assert!(!result.is_error);
        assert!(text(&result).contains("validation succeeded"));
        let details = result.details.expect("details");
        assert_eq!(details["source"], "voice_mode");
        assert_eq!(details["input_label"], "file:wav");
        assert_eq!(details["supported"], true);
    }

    #[tokio::test]
    async fn validate_rejects_remote_input_with_safe_details() {
        let tool = VoiceModeTool::new();
        let result = tool
            .execute(
                &ctx(),
                json!({"action": "validate", "input": "https://token@example.test/audio.wav\nsecret", "reply": "tts"}),
            )
            .await;

        assert!(result.is_error);
        let output = text(&result);
        assert!(!output.contains("token@example.test"));
        assert!(!output.contains('\n'));
        let details = result.details.expect("details");
        assert_eq!(details["input_label"], "https");
        assert_eq!(details["supported"], false);
        assert_eq!(details["error_kind"], "unsupported_input");
    }

    #[tokio::test]
    async fn live_capture_actions_return_normalized_receipts() {
        let tool = VoiceModeTool::new();
        let denied = tool.execute(&ctx(), json!({"action": "start_capture", "input": "microphone"})).await;
        assert!(denied.is_error);
        let details = denied.details.expect("denied details");
        assert_eq!(details["status"], "unsupported");
        assert_eq!(details["error_kind"], "voice_disabled");

        let active = tool
            .execute(
                &ctx(),
                json!({"action": "start_capture", "input": "microphone", "enabled": true, "auto_submit": true}),
            )
            .await;
        assert!(!active.is_error);
        let details = active.details.expect("active details");
        assert_eq!(details["status"], "active");
        assert_eq!(details["capture_active"], true);
        assert_eq!(details["auto_submit"], true);
        assert!(details.get("transcript").is_none());

        let stopped = tool.execute(&ctx(), json!({"action": "stop_capture", "input": "microphone"})).await;
        assert!(!stopped.is_error);
        assert_eq!(stopped.details.expect("stop details")["provider_request"], "closed");
    }

    #[tokio::test]
    async fn transcript_action_prepares_session_prompt_without_replay_transcript() {
        let tool = VoiceModeTool::new();
        let result = tool
            .execute(&ctx(), json!({"action": "submit_transcript", "transcript": "hello from voice", "reply": "text"}))
            .await;
        assert!(!result.is_error);
        let details = result.details.expect("prompt details");
        assert_eq!(details["status"], "prepared");
        assert_eq!(details["transcript_chars"], 16);
        assert!(details["transcript_digest"].as_str().unwrap().starts_with("fnv64:"));
        assert!(details.get("transcript").is_none());
    }
}
