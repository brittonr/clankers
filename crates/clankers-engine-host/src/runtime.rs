//! Engine feedback constructors owned by the reusable host runtime.

use clanker_message::Content;
use clankers_engine::EngineCorrelationId;
use clankers_engine::EngineInput;
use clankers_engine::EngineModelResponse;
use clankers_engine::EngineTerminalFailure;

pub const DEFAULT_TOOL_FAILURE_MESSAGE: &str = "tool execution failed";

#[must_use]
pub fn model_completed_input(request_id: EngineCorrelationId, response: EngineModelResponse) -> EngineInput {
    EngineInput::ModelCompleted { request_id, response }
}

#[must_use]
pub fn model_failed_input(request_id: EngineCorrelationId, failure: EngineTerminalFailure) -> EngineInput {
    EngineInput::ModelFailed { request_id, failure }
}

#[must_use]
pub fn retry_ready_input(request_id: EngineCorrelationId) -> EngineInput {
    EngineInput::RetryReady { request_id }
}

#[must_use]
pub fn tool_completed_input(call_id: EngineCorrelationId, result: Vec<Content>) -> EngineInput {
    EngineInput::ToolCompleted { call_id, result }
}

#[must_use]
pub fn tool_failed_input(call_id: EngineCorrelationId, error: String, result: Vec<Content>) -> EngineInput {
    EngineInput::ToolFailed { call_id, error, result }
}

#[must_use]
pub fn tool_feedback_input(call_id: EngineCorrelationId, is_error: bool, result: Vec<Content>) -> EngineInput {
    if is_error {
        let error = first_text_block(&result).unwrap_or_else(|| DEFAULT_TOOL_FAILURE_MESSAGE.to_string());
        return tool_failed_input(call_id, error, result);
    }

    tool_completed_input(call_id, result)
}

#[must_use]
pub fn cancel_turn_input(reason: String) -> EngineInput {
    EngineInput::CancelTurn { reason }
}

fn first_text_block(content: &[Content]) -> Option<String> {
    content.iter().find_map(|block| match block {
        Content::Text { text } => Some(text.clone()),
        Content::Image { .. } | Content::Thinking { .. } | Content::ToolUse { .. } | Content::ToolResult { .. } => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_ID: &str = "correlation";
    const TEST_ERROR: &str = "boom";

    #[test]
    fn tool_feedback_converts_success_and_error_inputs() {
        let success = tool_feedback_input(EngineCorrelationId(TEST_ID.to_string()), false, vec![Content::Text {
            text: "ok".to_string(),
        }]);
        assert!(matches!(success, EngineInput::ToolCompleted { .. }));

        let failed = tool_feedback_input(EngineCorrelationId(TEST_ID.to_string()), true, vec![Content::Text {
            text: TEST_ERROR.to_string(),
        }]);
        assert!(matches!(failed, EngineInput::ToolFailed { error, .. } if error == TEST_ERROR));
    }

    #[test]
    fn tool_feedback_uses_default_error_when_text_missing() {
        let failed = tool_feedback_input(EngineCorrelationId(TEST_ID.to_string()), true, Vec::new());
        assert!(matches!(failed, EngineInput::ToolFailed { error, .. } if error == DEFAULT_TOOL_FAILURE_MESSAGE));
    }
}
