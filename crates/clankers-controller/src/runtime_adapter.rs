//! Runtime/session service adapter contracts for controller shell tests.
//!
//! These contracts let the controller exercise command lifecycle and semantic
//! event projection without constructing providers, databases, TUI state,
//! sockets, or desktop session storage. Production daemon mode can continue to
//! own an `Agent`; controller fixtures and future runtime-backed shells can use
//! this narrower adapter seam.

use clanker_message::SemanticEvent;
use clanker_message::SemanticEventMetadata;
use clanker_message::SemanticStopReason;
use clankers_core::CoreThinkingLevel;

/// Prompt request passed from the controller to a runtime/session adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePromptRequest {
    pub session_id: String,
    pub model: String,
    pub text: String,
    pub image_count: u32,
}

/// Prompt completion state returned by a runtime/session adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimePromptCompletion {
    Succeeded,
    Cancelled,
    Failed { message: String },
}

/// Prompt result returned by a runtime/session adapter.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimePromptResult {
    pub completion: RuntimePromptCompletion,
    pub semantic_events: Vec<SemanticEvent>,
}

impl RuntimePromptResult {
    #[must_use]
    pub fn succeeded(semantic_events: Vec<SemanticEvent>) -> Self {
        Self {
            completion: RuntimePromptCompletion::Succeeded,
            semantic_events,
        }
    }

    #[must_use]
    pub fn failed(message: impl Into<String>, semantic_events: Vec<SemanticEvent>) -> Self {
        Self {
            completion: RuntimePromptCompletion::Failed {
                message: message.into(),
            },
            semantic_events,
        }
    }
}

/// Control requests passed from the controller to a runtime/session adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeControlRequest {
    Abort,
    ResetCancel,
    SetThinkingLevel { level: CoreThinkingLevel },
    SetDisabledTools { tools: Vec<String> },
}

/// Control result returned by a runtime/session adapter.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RuntimeControlResult {
    pub semantic_events: Vec<SemanticEvent>,
}

/// Controller-facing runtime/session service seam.
pub trait ControllerRuntimeAdapter {
    fn submit_prompt(&mut self, request: RuntimePromptRequest) -> RuntimePromptResult;
    fn apply_control(&mut self, request: RuntimeControlRequest) -> RuntimeControlResult;
}

/// Deterministic adapter for controller fixtures.
#[derive(Debug, Default)]
pub struct FakeRuntimeAdapter {
    prompt_results: Vec<RuntimePromptResult>,
    pub prompts: Vec<RuntimePromptRequest>,
    pub controls: Vec<RuntimeControlRequest>,
}

impl FakeRuntimeAdapter {
    #[must_use]
    pub fn new(prompt_results: Vec<RuntimePromptResult>) -> Self {
        Self {
            prompt_results,
            prompts: Vec::new(),
            controls: Vec::new(),
        }
    }
}

impl ControllerRuntimeAdapter for FakeRuntimeAdapter {
    fn submit_prompt(&mut self, request: RuntimePromptRequest) -> RuntimePromptResult {
        self.prompts.push(request);
        if self.prompt_results.is_empty() {
            return RuntimePromptResult::succeeded(vec![SemanticEvent::Completed {
                stop_reason: SemanticStopReason::Complete,
                metadata: SemanticEventMetadata::empty().with("source", "fake_runtime_adapter"),
            }]);
        }
        self.prompt_results.remove(0)
    }

    fn apply_control(&mut self, request: RuntimeControlRequest) -> RuntimeControlResult {
        self.controls.push(request);
        RuntimeControlResult::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_runtime_adapter_records_prompts_and_controls_without_desktop_services() {
        let mut adapter = FakeRuntimeAdapter::new(vec![RuntimePromptResult::succeeded(vec![
            SemanticEvent::AssistantDelta {
                text: "ok".to_string(),
                metadata: SemanticEventMetadata::empty().with("source", "fixture"),
            },
        ])]);

        let result = adapter.submit_prompt(RuntimePromptRequest {
            session_id: "session-adapter".to_string(),
            model: "model-adapter".to_string(),
            text: "hello".to_string(),
            image_count: 0,
        });
        adapter.apply_control(RuntimeControlRequest::SetThinkingLevel {
            level: CoreThinkingLevel::High,
        });

        assert_eq!(adapter.prompts.len(), 1);
        assert_eq!(adapter.controls.len(), 1);
        assert!(matches!(result.completion, RuntimePromptCompletion::Succeeded));
        assert_eq!(result.semantic_events[0].kind(), "assistant_delta");
    }
}
