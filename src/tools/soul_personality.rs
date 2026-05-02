//! SOUL/personality validation tool.

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;
use crate::soul_personality;

pub struct SoulPersonalityTool {
    definition: ToolDefinition,
}

impl SoulPersonalityTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "soul_personality".to_string(),
                description: "Inspect and validate first-pass SOUL/personality policy.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "action": {"type": "string", "enum": ["status", "validate"]},
                        "soul": {"type": "string", "description": "SOUL source: local path, file:<PATH>, discover, http(s)://*, cloud:*, or command marker"},
                        "personality": {"type": "string", "description": "Safe personality preset name"}
                    },
                    "required": ["action"]
                }),
            },
        }
    }
}

impl Default for SoulPersonalityTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for SoulPersonalityTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, _ctx: &ToolContext, params: Value) -> ToolResult {
        match params.get("action").and_then(|value| value.as_str()) {
            Some("status") => validation_result(soul_personality::status_summary()),
            Some("validate") => validate_params(&params),
            Some(other) => ToolResult::error(format!("Unknown soul_personality action: {other}")),
            None => ToolResult::error("Missing required parameter: action"),
        }
    }
}

fn validate_params(params: &Value) -> ToolResult {
    let source = soul_personality::parse_soul_source(params.get("soul").and_then(|value| value.as_str()));
    let personality =
        match soul_personality::parse_personality(params.get("personality").and_then(|value| value.as_str())) {
            Ok(value) => value,
            Err(message) => return ToolResult::error(message),
        };
    validation_result(soul_personality::validate(&source, personality.as_ref()))
}

fn validation_result(validation: soul_personality::SoulValidation) -> ToolResult {
    let details = serde_json::to_value(&validation).unwrap_or_else(|_| json!({"source": "soul_personality"}));
    let personality = validation.personality.as_deref().unwrap_or("none");
    let text = if validation.supported {
        format!(
            "SOUL/personality validation succeeded: {} source via {} (personality: {})",
            validation.soul_label, validation.backend, personality
        )
    } else {
        format!(
            "SOUL/personality validation unsupported: {} ({})",
            validation.soul_label,
            validation.error_message.as_deref().unwrap_or("unsupported source")
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
        ToolContext::new("soul-personality-test".to_string(), CancellationToken::new(), None)
    }

    fn text(result: &ToolResult) -> &str {
        match result.content.first().expect("content") {
            ToolResultContent::Text { text } => text,
            ToolResultContent::Image { .. } => panic!("unexpected image content"),
        }
    }

    #[tokio::test]
    async fn validate_returns_safe_details_for_supported_local_source() {
        let tool = SoulPersonalityTool::new();
        let result = tool
            .execute(&ctx(), json!({"action": "validate", "soul": "/tmp/private/SOUL.md", "personality": "mentor"}))
            .await;

        assert!(!result.is_error);
        assert!(text(&result).contains("validation succeeded"));
        let details = result.details.expect("details");
        assert_eq!(details["source"], "soul_personality");
        assert_eq!(details["soul_label"], "SOUL.md");
        assert_eq!(details["personality"], "mentor");
    }

    #[tokio::test]
    async fn validate_rejects_remote_source_with_safe_details() {
        let tool = SoulPersonalityTool::new();
        let result = tool
            .execute(&ctx(), json!({"action": "validate", "soul": "https://token@example.test/SOUL.md\nsecret"}))
            .await;

        assert!(result.is_error);
        let output = text(&result);
        assert!(!output.contains("token@example.test"));
        assert!(!output.contains('\n'));
        let details = result.details.expect("details");
        assert_eq!(details["soul_label"], "https");
        assert_eq!(details["supported"], false);
        assert_eq!(details["error_kind"], "unsupported_source");
    }
}
