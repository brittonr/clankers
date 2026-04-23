//! Host-facing reusable engine contracts that sit above `clankers-core`
//! and below controller, agent-runtime, and UI/transport shells.

use std::collections::HashMap;

use clankers_core::CoreState;
use clankers_message::AgentMessage;
use clankers_message::Content;
use clankers_message::StopReason;
use clankers_provider::CompletionRequest;
use clankers_provider::ThinkingConfig;
use clanker_router::provider::ToolDefinition;
use serde_json::Value;

pub const ENGINE_CONTRACT_VERSION: u32 = 1;
pub const ENGINE_MODEL_REQUEST_ID: &str = "model-request-1";
pub const ENGINE_SUBMIT_PROMPT_NOTICE: &str = "engine queued initial model request";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineCorrelationId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineTurnPhase {
    Idle,
    WaitingForModel,
    WaitingForTools,
    Finished,
}

#[derive(Debug, Clone)]
pub struct EngineMessage {
    pub role: EngineMessageRole,
    pub content: Vec<Content>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineMessageRole {
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone)]
pub struct EngineModelRequest {
    pub request_id: EngineCorrelationId,
    pub request: CompletionRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineToolCall {
    pub call_id: EngineCorrelationId,
    pub tool_name: String,
    pub input: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineEvent {
    BusyChanged { busy: bool },
    Notice { message: String },
    TurnFinished { stop_reason: StopReason },
}

#[derive(Debug, Clone)]
pub enum EngineEffect {
    RequestModel(EngineModelRequest),
    ExecuteTool(EngineToolCall),
    EmitEvent(EngineEvent),
}

#[derive(Debug, Clone)]
pub enum EngineInput {
    SubmitUserPrompt { prompt: String, attachments: Vec<String> },
    ModelCompleted { request_id: EngineCorrelationId, output: Vec<Content> },
    ModelFailed { request_id: EngineCorrelationId, error: String },
    ToolCompleted { call_id: EngineCorrelationId, result: Vec<Content> },
    ToolFailed { call_id: EngineCorrelationId, error: String },
    CancelTurn { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineRejection {
    Busy,
    CorrelationMismatch,
    InvalidPhase,
}

#[derive(Debug, Clone)]
pub struct EngineState {
    pub contract_version: u32,
    pub core_state: Option<CoreState>,
    pub phase: EngineTurnPhase,
    pub messages: Vec<EngineMessage>,
    pub pending_model_request: Option<EngineCorrelationId>,
    pub pending_tool_calls: Vec<EngineCorrelationId>,
}

#[derive(Debug, Clone)]
pub struct EngineOutcome {
    pub next_state: EngineState,
    pub effects: Vec<EngineEffect>,
    pub rejection: Option<EngineRejection>,
}

#[derive(Debug, Clone)]
pub struct EnginePromptSubmission {
    pub messages: Vec<AgentMessage>,
    pub model: String,
    pub system_prompt: String,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f64>,
    pub thinking: Option<ThinkingConfig>,
    pub tools: Vec<ToolDefinition>,
    pub no_cache: bool,
    pub cache_ttl: Option<String>,
    pub session_id: String,
}

impl EngineState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            contract_version: ENGINE_CONTRACT_VERSION,
            core_state: None,
            phase: EngineTurnPhase::Idle,
            messages: Vec::new(),
            pending_model_request: None,
            pending_tool_calls: Vec::new(),
        }
    }
}

impl Default for EngineState {
    fn default() -> Self {
        Self::new()
    }
}

#[must_use]
pub fn plan_initial_model_request(state: &EngineState, submission: &EnginePromptSubmission) -> EngineOutcome {
    if state.phase != EngineTurnPhase::Idle || state.pending_model_request.is_some() {
        return EngineOutcome {
            next_state: state.clone(),
            effects: Vec::new(),
            rejection: Some(EngineRejection::Busy),
        };
    }

    let request_id = EngineCorrelationId(ENGINE_MODEL_REQUEST_ID.to_string());
    let request = CompletionRequest {
        model: submission.model.clone(),
        messages: submission.messages.clone(),
        system_prompt: Some(submission.system_prompt.clone()),
        max_tokens: submission.max_tokens,
        temperature: submission.temperature,
        tools: submission.tools.clone(),
        thinking: submission.thinking.clone(),
        no_cache: submission.no_cache,
        cache_ttl: submission.cache_ttl.clone(),
        extra_params: build_extra_params(&submission.session_id),
    };

    let next_state = EngineState {
        contract_version: state.contract_version,
        core_state: state.core_state.clone(),
        phase: EngineTurnPhase::WaitingForModel,
        messages: state.messages.clone(),
        pending_model_request: Some(request_id.clone()),
        pending_tool_calls: Vec::new(),
    };

    EngineOutcome {
        next_state,
        effects: vec![
            EngineEffect::EmitEvent(EngineEvent::BusyChanged { busy: true }),
            EngineEffect::EmitEvent(EngineEvent::Notice {
                message: ENGINE_SUBMIT_PROMPT_NOTICE.to_string(),
            }),
            EngineEffect::RequestModel(EngineModelRequest { request_id, request }),
        ],
        rejection: None,
    }
}

fn build_extra_params(session_id: &str) -> HashMap<String, Value> {
    if session_id.is_empty() {
        return HashMap::new();
    }

    HashMap::from([("_session_id".to_string(), Value::String(session_id.to_string()))])
}

#[cfg(test)]
mod tests {
    use clankers_message::MessageId;
    use clankers_message::UserMessage;
    use serde_json::json;

    use super::*;

    fn test_timestamp() -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now()
    }

    const MAX_TOKENS: usize = 100;
    const MESSAGE_COUNT: usize = 1;

    fn submission_with_session(session_id: &str) -> EnginePromptSubmission {
        EnginePromptSubmission {
            messages: vec![AgentMessage::User(UserMessage {
                id: MessageId::new("user-1"),
                content: vec![Content::Text {
                    text: "hello".to_string(),
                }],
                timestamp: test_timestamp(),
            })],
            model: "test-model".to_string(),
            system_prompt: "system".to_string(),
            max_tokens: Some(MAX_TOKENS),
            temperature: None,
            thinking: None,
            tools: vec![ToolDefinition {
                name: "read".to_string(),
                description: "Read a file".to_string(),
                input_schema: json!({"type": "object"}),
            }],
            no_cache: true,
            cache_ttl: None,
            session_id: session_id.to_string(),
        }
    }

    #[test]
    fn plan_initial_model_request_builds_request_effect() {
        let state = EngineState::new();
        let submission = submission_with_session("session-123");

        let outcome = plan_initial_model_request(&state, &submission);

        assert!(outcome.rejection.is_none());
        assert_eq!(outcome.next_state.phase, EngineTurnPhase::WaitingForModel);
        assert_eq!(outcome.next_state.pending_model_request, Some(EngineCorrelationId(ENGINE_MODEL_REQUEST_ID.to_string())));
        assert_eq!(outcome.effects.len(), 3);

        let EngineEffect::RequestModel(model_effect) = &outcome.effects[2] else {
            panic!("expected final effect to be RequestModel");
        };
        assert_eq!(model_effect.request_id, EngineCorrelationId(ENGINE_MODEL_REQUEST_ID.to_string()));
        assert_eq!(model_effect.request.model, "test-model");
        assert_eq!(model_effect.request.messages.len(), MESSAGE_COUNT);
        assert_eq!(model_effect.request.extra_params.get("_session_id"), Some(&json!("session-123")));
    }

    #[test]
    fn plan_initial_model_request_rejects_busy_state() {
        let mut state = EngineState::new();
        state.phase = EngineTurnPhase::WaitingForModel;
        state.pending_model_request = Some(EngineCorrelationId("existing".to_string()));

        let outcome = plan_initial_model_request(&state, &submission_with_session("session-123"));

        assert!(outcome.effects.is_empty());
        assert_eq!(outcome.rejection, Some(EngineRejection::Busy));
        assert_eq!(outcome.next_state.phase, EngineTurnPhase::WaitingForModel);
        assert_eq!(outcome.next_state.pending_model_request, Some(EngineCorrelationId("existing".to_string())));
    }

    #[test]
    fn plan_initial_model_request_skips_session_param_when_empty() {
        let outcome = plan_initial_model_request(&EngineState::new(), &submission_with_session(""));

        let EngineEffect::RequestModel(model_effect) = &outcome.effects[2] else {
            panic!("expected final effect to be RequestModel");
        };
        assert!(model_effect.request.extra_params.is_empty());
    }
}
