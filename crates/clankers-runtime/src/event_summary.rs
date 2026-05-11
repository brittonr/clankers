use serde_json::Value;
use serde_json::json;

use crate::SessionEvent;

/// Deterministic fixture adapter that mirrors the event order used by headless prompt tests.
#[must_use]
pub fn headless_prompt_parity_fixture(prompt: &str) -> Vec<&'static str> {
    let _ = prompt;
    vec!["prompt_accepted", "assistant_delta", "cost_updated", "completed"]
}

/// Serialize a safe event summary for host parity tests and docs examples.
#[must_use]
pub fn safe_event_summary(event: &SessionEvent) -> Value {
    match event {
        SessionEvent::PromptAccepted { metadata, .. } => {
            json!({"type":"prompt_accepted", "metadata_fields": metadata.fields.len()})
        }
        SessionEvent::AssistantDelta { text, metadata, .. } => {
            json!({"type":"assistant_delta", "text_chars": text.chars().count(), "metadata_fields": metadata.fields.len()})
        }
        SessionEvent::ThinkingDelta { text, metadata, .. } => {
            json!({"type":"thinking_delta", "text_chars": text.chars().count(), "metadata_fields": metadata.fields.len()})
        }
        SessionEvent::CostUpdated {
            input_tokens,
            output_tokens,
            ..
        } => json!({"type":"cost_updated", "input_tokens": input_tokens, "output_tokens": output_tokens}),
        SessionEvent::Completed { stop_reason, .. } => {
            json!({"type":"completed", "stop_reason": format!("{stop_reason:?}")})
        }
        SessionEvent::ToolStarted { tool_name, .. } => json!({"type":"tool_started", "tool_name": tool_name}),
        SessionEvent::ToolFinished { status, .. } => json!({"type":"tool_finished", "status": format!("{status:?}")}),
        SessionEvent::ConfirmationRequested { request, .. } => {
            json!({"type":"confirmation_requested", "action": format!("{:?}", request.action)})
        }
        SessionEvent::Error { error_class, .. } => json!({"type":"error", "class": format!("{error_class:?}")}),
        SessionEvent::Shutdown { .. } => json!({"type":"shutdown"}),
    }
}
