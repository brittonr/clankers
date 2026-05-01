//! Tool gateway validation tool.
//!
//! First-pass gateway support is intentionally validation-only: local/session
//! delivery is accepted, Matrix is accepted only when the caller explicitly
//! reports an active bridge context, and remote/webhook/cloud/credential targets
//! return safe unsupported metadata.

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;
use crate::tool_gateway;

pub struct ToolGatewayTool {
    definition: ToolDefinition,
}

impl ToolGatewayTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "tool_gateway".to_string(),
                description: "Inspect and validate first-pass tool gateway delivery policy.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["status", "validate"],
                            "description": "Gateway action to perform"
                        },
                        "toolsets": {
                            "type": "string",
                            "description": "Comma-separated toolsets to validate: core, orchestration, specialty, matrix"
                        },
                        "deliver": {
                            "type": "string",
                            "description": "Delivery target to validate: local, session, matrix, or an unsupported remote target"
                        },
                        "matrix_active": {
                            "type": "boolean",
                            "description": "True only inside an active Matrix bridge delivery context"
                        }
                    },
                    "required": ["action"]
                }),
            },
        }
    }
}

impl Default for ToolGatewayTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ToolGatewayTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, _ctx: &ToolContext, params: Value) -> ToolResult {
        match params.get("action").and_then(|value| value.as_str()) {
            Some("status") => validation_result(tool_gateway::status_summary()),
            Some("validate") => validate_params(&params),
            Some(other) => ToolResult::error(format!("Unknown tool_gateway action: {other}")),
            None => ToolResult::error("Missing required parameter: action"),
        }
    }
}

fn validate_params(params: &Value) -> ToolResult {
    let toolsets = match params.get("toolsets").and_then(|value| value.as_str()) {
        Some(value) => value,
        None => return ToolResult::error("Missing required parameter for validate: toolsets"),
    };
    let toolsets = match tool_gateway::parse_toolsets(toolsets) {
        Ok(toolsets) => toolsets,
        Err(message) => return ToolResult::error(message),
    };
    let target = tool_gateway::parse_delivery_target(params.get("deliver").and_then(|value| value.as_str()));
    let matrix_active = params.get("matrix_active").and_then(|value| value.as_bool()).unwrap_or(false);
    validation_result(tool_gateway::validate(&toolsets, &target, matrix_active))
}

fn validation_result(validation: tool_gateway::GatewayValidation) -> ToolResult {
    let details = serde_json::to_value(&validation).unwrap_or_else(|_| json!({"source": "tool_gateway"}));
    let text = if validation.supported {
        format!(
            "Tool gateway validation succeeded: {} delivery via {} (toolsets: {})",
            validation.delivery_target,
            validation.backend,
            validation.toolsets.join(", ")
        )
    } else {
        format!(
            "Tool gateway validation unsupported: {} ({})",
            validation.delivery_target,
            validation.error_message.as_deref().unwrap_or("unsupported target")
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
        ToolContext::new("tool-gateway-test".to_string(), CancellationToken::new(), None)
    }

    fn text(result: &ToolResult) -> &str {
        match result.content.first().expect("content") {
            ToolResultContent::Text { text } => text,
            ToolResultContent::Image { .. } => panic!("unexpected image content"),
        }
    }

    #[tokio::test]
    async fn validate_returns_safe_details_for_supported_local_delivery() {
        let tool = ToolGatewayTool::new();
        let result = tool
            .execute(&ctx(), json!({"action": "validate", "toolsets": "core,specialty", "deliver": "local"}))
            .await;

        assert!(!result.is_error);
        assert!(text(&result).contains("validation succeeded"));
        let details = result.details.expect("details");
        assert_eq!(details["source"], "tool_gateway");
        assert_eq!(details["delivery_target"], "local");
        assert_eq!(details["supported"], true);
    }

    #[tokio::test]
    async fn validate_rejects_unsupported_remote_delivery_with_safe_details() {
        let tool = ToolGatewayTool::new();
        let result = tool
            .execute(
                &ctx(),
                json!({"action": "validate", "toolsets": "core", "deliver": "https://token@example.test/hook\nsecret"}),
            )
            .await;

        assert!(result.is_error);
        let output = text(&result);
        assert!(!output.contains("token@example.test"));
        assert!(!output.contains('\n'));
        let details = result.details.expect("details");
        assert_eq!(details["delivery_target"], "https");
        assert_eq!(details["supported"], false);
        assert_eq!(details["error_kind"], "unsupported_target");
    }
}
