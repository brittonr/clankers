//! Tool gateway validation tool.
//!
//! Gateway support is intentionally validation/receipt-first: local/session
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
                description: "Inspect and validate tool gateway delivery policy and safe receipts.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["status", "validate", "deliver", "deliver_receipt", "delivery_status", "retry"],
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
                        },
                        "artifact_type": {
                            "type": "string",
                            "description": "Artifact type for deliver_receipt: file, media, or scheduled-output"
                        },
                        "path": {
                            "type": "string",
                            "description": "Optional artifact path; only the basename is recorded in receipts"
                        },
                        "outbox": {
                            "type": "string",
                            "description": "Optional outbox path for deliver/status/retry actions"
                        },
                        "attempt_id": {
                            "type": "string",
                            "description": "Safe delivery attempt id for retry"
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
            Some("deliver") => deliver_params(&params, true),
            Some("deliver_receipt") => deliver_params(&params, false),
            Some("delivery_status") => delivery_status_params(&params),
            Some("retry") => retry_params(&params),
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

fn deliver_params(params: &Value, record: bool) -> ToolResult {
    let artifact_type = match params.get("artifact_type").and_then(|value| value.as_str()) {
        Some("file") => tool_gateway::ArtifactKind::File,
        Some("media") => tool_gateway::ArtifactKind::Media,
        Some("scheduled-output" | "scheduled_output" | "scheduled") => tool_gateway::ArtifactKind::ScheduledOutput,
        Some(other) => return ToolResult::error(format!("unknown artifact type '{other}'")),
        None => return ToolResult::error("Missing required parameter for deliver: artifact_type"),
    };
    let target = tool_gateway::parse_delivery_target(params.get("deliver").and_then(|value| value.as_str()));
    let path = params.get("path").and_then(|value| value.as_str()).map(std::path::Path::new);
    let matrix_active = params.get("matrix_active").and_then(|value| value.as_bool()).unwrap_or(false);
    let context = if matrix_active {
        tool_gateway::DeliveryContext::matrix("active_matrix_session")
    } else {
        tool_gateway::DeliveryContext::local()
    };
    let attempt = tool_gateway::deliver_artifact(artifact_type, path, &target, &context);
    if record {
        if let Some(outbox) = params.get("outbox").and_then(|value| value.as_str()) {
            match tool_gateway::record_attempt(std::path::Path::new(outbox), attempt) {
                Ok(attempt) => return attempt_result(attempt),
                Err(message) => return ToolResult::error(message),
            }
        }
    }
    if record {
        attempt_result(attempt)
    } else {
        delivery_result(attempt.receipt)
    }
}

fn delivery_status_params(params: &Value) -> ToolResult {
    let Some(outbox) = params.get("outbox").and_then(|value| value.as_str()) else {
        return ToolResult::error("Missing required parameter for delivery_status: outbox");
    };
    match tool_gateway::read_outbox(std::path::Path::new(outbox)) {
        Ok(outbox) => ToolResult::text(format!("Tool gateway delivery status: {} attempts", outbox.attempts.len()))
            .with_details(serde_json::to_value(outbox).unwrap_or_else(|_| json!({"source": "tool_gateway"}))),
        Err(message) => ToolResult::error(message),
    }
}

fn retry_params(params: &Value) -> ToolResult {
    let Some(outbox) = params.get("outbox").and_then(|value| value.as_str()) else {
        return ToolResult::error("Missing required parameter for retry: outbox");
    };
    let Some(attempt_id) = params.get("attempt_id").and_then(|value| value.as_str()) else {
        return ToolResult::error("Missing required parameter for retry: attempt_id");
    };
    let context = tool_gateway::DeliveryContext::local();
    match tool_gateway::retry_attempt(std::path::Path::new(outbox), attempt_id, &context) {
        Ok(attempt) => attempt_result(attempt),
        Err(message) => ToolResult::error(message),
    }
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

fn attempt_result(attempt: tool_gateway::DeliveryAttempt) -> ToolResult {
    let details = serde_json::to_value(&attempt).unwrap_or_else(|_| json!({"source": "tool_gateway"}));
    let text = format!(
        "Tool gateway delivery attempt: {} via {} ({})",
        attempt.artifact_type, attempt.receipt.backend, attempt.target_kind
    );
    let result = if attempt.status == "success" {
        ToolResult::text(text)
    } else {
        ToolResult::error(text)
    };
    result.with_details(details)
}

fn delivery_result(receipt: tool_gateway::PlatformDeliveryReceipt) -> ToolResult {
    let details = serde_json::to_value(&receipt).unwrap_or_else(|_| json!({"source": "tool_gateway"}));
    let text = if receipt.status == "success" {
        format!(
            "Tool gateway delivery receipt: {} via {} ({})",
            receipt.artifact_type, receipt.backend, receipt.target_kind
        )
    } else {
        format!(
            "Tool gateway delivery unsupported: {} ({})",
            receipt.target_kind,
            receipt.error_message.as_deref().unwrap_or("unsupported target")
        )
    };
    let result = if receipt.status == "success" {
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

    #[tokio::test]
    async fn deliver_receipt_returns_safe_artifact_metadata() {
        let tool = ToolGatewayTool::new();
        let result = tool
            .execute(
                &ctx(),
                json!({"action": "deliver_receipt", "artifact_type": "media", "path": "/tmp/secret/out.mp3", "deliver": "session"}),
            )
            .await;

        assert!(!result.is_error);
        let details = result.details.expect("details");
        assert_eq!(details["artifact_type"], "media");
        assert_eq!(details["safe_path"], "out.mp3");
        assert!(!serde_json::to_string(&details).expect("serialize").contains("secret"));
    }
}
