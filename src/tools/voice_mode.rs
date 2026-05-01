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
                            "enum": ["status", "validate"],
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
                        }
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
    let matrix_active = params.get("matrix_active").and_then(|value| value.as_bool()).unwrap_or(false);
    validation_result(voice_mode::validate(&source, reply, matrix_active))
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
}
