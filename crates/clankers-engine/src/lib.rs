//! Host-facing reusable engine contracts that sit above `clankers-core`
//! and below controller, agent-runtime, and UI/transport shells.

use clanker_router::provider::ToolDefinition;
use clankers_core::CoreState;
use clanker_message::AgentMessage;
use clanker_message::Content;
use clanker_message::StopReason;
use clankers_provider::ThinkingConfig;
use serde_json::Value;

pub const ENGINE_CONTRACT_VERSION: u32 = 1;
pub const ENGINE_MODEL_REQUEST_PREFIX: &str = "model-request";
pub const ENGINE_INITIAL_MODEL_REQUEST_SEQUENCE: u32 = 1;
pub const ENGINE_CORRELATION_SEQUENCE_STEP: u32 = 1;
pub const ENGINE_SUBMIT_PROMPT_NOTICE: &str = "engine queued initial model request";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
pub struct EngineRequestTemplate {
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

#[derive(Debug, Clone)]
pub struct EngineModelRequest {
    pub request_id: EngineCorrelationId,
    pub model: String,
    pub messages: Vec<EngineMessage>,
    pub system_prompt: String,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f64>,
    pub thinking: Option<ThinkingConfig>,
    pub tools: Vec<ToolDefinition>,
    pub no_cache: bool,
    pub cache_ttl: Option<String>,
    pub session_id: String,
}

#[derive(Debug, Clone)]
pub struct EngineModelResponse {
    pub output: Vec<Content>,
    pub stop_reason: StopReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineToolCall {
    pub call_id: EngineCorrelationId,
    pub tool_name: String,
    pub input: Value,
}

#[derive(Debug, Clone)]
pub struct EngineBufferedToolResult {
    pub call_id: EngineCorrelationId,
    pub content: Vec<Content>,
    pub is_error: bool,
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
    SubmitUserPrompt { submission: EnginePromptSubmission },
    ModelCompleted {
        request_id: EngineCorrelationId,
        response: EngineModelResponse,
    },
    ModelFailed {
        request_id: EngineCorrelationId,
        error: String,
    },
    ToolCompleted {
        call_id: EngineCorrelationId,
        result: Vec<Content>,
    },
    ToolFailed {
        call_id: EngineCorrelationId,
        error: String,
        result: Vec<Content>,
    },
    CancelTurn {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineRejection {
    Busy,
    CorrelationMismatch,
    InvalidPhase,
    MissingToolCall,
}

#[derive(Debug, Clone)]
pub struct EngineState {
    pub contract_version: u32,
    pub core_state: Option<CoreState>,
    pub phase: EngineTurnPhase,
    pub messages: Vec<EngineMessage>,
    pub request_template: Option<EngineRequestTemplate>,
    pub pending_model_request: Option<EngineCorrelationId>,
    pub next_model_request_sequence: u32,
    pub pending_tool_calls: Vec<EngineCorrelationId>,
    pub buffered_tool_results: Vec<EngineBufferedToolResult>,
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
            request_template: None,
            pending_model_request: None,
            next_model_request_sequence: ENGINE_INITIAL_MODEL_REQUEST_SEQUENCE,
            pending_tool_calls: Vec::new(),
            buffered_tool_results: Vec::new(),
        }
    }
}

impl Default for EngineState {
    fn default() -> Self {
        Self::new()
    }
}

#[must_use]
pub fn reduce(state: &EngineState, input: &EngineInput) -> EngineOutcome {
    match input {
        EngineInput::SubmitUserPrompt { submission } => apply_submit_user_prompt(state, submission),
        EngineInput::ModelCompleted {
            request_id,
            response,
        } => apply_model_completion(state, request_id, response),
        EngineInput::ModelFailed { request_id, error } => apply_model_failed(state, request_id, error),
        EngineInput::ToolCompleted { call_id, result } => apply_tool_feedback(state, call_id, result, false),
        EngineInput::ToolFailed {
            call_id,
            error,
            result,
        } => {
            let tool_result_content = if result.is_empty() {
                vec![Content::Text {
                    text: error.clone(),
                }]
            } else {
                result.clone()
            };
            apply_tool_feedback(state, call_id, &tool_result_content, true)
        }
        EngineInput::CancelTurn { reason } => apply_cancel_turn(state, reason),
    }
}

#[must_use]
fn apply_submit_user_prompt(state: &EngineState, submission: &EnginePromptSubmission) -> EngineOutcome {
    if state.phase != EngineTurnPhase::Idle || state.pending_model_request.is_some() {
        return rejected_outcome(state, EngineRejection::Busy);
    }

    let request_template = EngineRequestTemplate {
        model: submission.model.clone(),
        system_prompt: submission.system_prompt.clone(),
        max_tokens: submission.max_tokens,
        temperature: submission.temperature,
        thinking: submission.thinking.clone(),
        tools: submission.tools.clone(),
        no_cache: submission.no_cache,
        cache_ttl: submission.cache_ttl.clone(),
        session_id: submission.session_id.clone(),
    };
    let canonical_messages = canonical_messages_from_agent_messages(&submission.messages);
    let (request_id, next_model_request_sequence) = mint_model_request_id(state.next_model_request_sequence);
    let model_request = build_model_request(&request_template, &canonical_messages, request_id.clone());

    let next_state = EngineState {
        contract_version: state.contract_version,
        core_state: state.core_state.clone(),
        phase: EngineTurnPhase::WaitingForModel,
        messages: canonical_messages,
        request_template: Some(request_template),
        pending_model_request: Some(request_id.clone()),
        next_model_request_sequence,
        pending_tool_calls: Vec::new(),
        buffered_tool_results: Vec::new(),
    };

    EngineOutcome {
        next_state,
        effects: vec![
            EngineEffect::EmitEvent(EngineEvent::BusyChanged { busy: true }),
            EngineEffect::EmitEvent(EngineEvent::Notice {
                message: ENGINE_SUBMIT_PROMPT_NOTICE.to_string(),
            }),
            EngineEffect::RequestModel(model_request),
        ],
        rejection: None,
    }
}

#[must_use]
fn apply_model_completion(
    state: &EngineState,
    request_id: &EngineCorrelationId,
    response: &EngineModelResponse,
) -> EngineOutcome {
    if state.phase != EngineTurnPhase::WaitingForModel {
        return rejected_outcome(state, EngineRejection::InvalidPhase);
    }

    let Some(pending_request_id) = state.pending_model_request.as_ref() else {
        return rejected_outcome(state, EngineRejection::InvalidPhase);
    };
    if pending_request_id != request_id {
        return rejected_outcome(state, EngineRejection::CorrelationMismatch);
    }

    let mut next_messages = state.messages.clone();
    next_messages.push(EngineMessage {
        role: EngineMessageRole::Assistant,
        content: response.output.clone(),
    });

    let tool_calls = extract_tool_calls(&response.output);
    if response.stop_reason == StopReason::ToolUse {
        if tool_calls.is_empty() {
            return rejected_outcome(state, EngineRejection::MissingToolCall);
        }

        let pending_tool_calls = tool_calls.iter().map(|call| call.call_id.clone()).collect();
        let next_state = EngineState {
            contract_version: state.contract_version,
            core_state: state.core_state.clone(),
            phase: EngineTurnPhase::WaitingForTools,
            messages: next_messages,
            request_template: state.request_template.clone(),
            pending_model_request: None,
            next_model_request_sequence: state.next_model_request_sequence,
            pending_tool_calls,
            buffered_tool_results: Vec::new(),
        };
        return EngineOutcome {
            next_state,
            effects: tool_calls.into_iter().map(EngineEffect::ExecuteTool).collect(),
            rejection: None,
        };
    }

    let next_state = EngineState {
        contract_version: state.contract_version,
        core_state: state.core_state.clone(),
        phase: EngineTurnPhase::Finished,
        messages: next_messages,
        request_template: None,
        pending_model_request: None,
        next_model_request_sequence: state.next_model_request_sequence,
        pending_tool_calls: Vec::new(),
        buffered_tool_results: Vec::new(),
    };
    EngineOutcome {
        next_state,
        effects: vec![
            EngineEffect::EmitEvent(EngineEvent::BusyChanged { busy: false }),
            EngineEffect::EmitEvent(EngineEvent::TurnFinished {
                stop_reason: response.stop_reason.clone(),
            }),
        ],
        rejection: None,
    }
}

#[must_use]
fn apply_model_failed(state: &EngineState, request_id: &EngineCorrelationId, error: &str) -> EngineOutcome {
    if state.phase != EngineTurnPhase::WaitingForModel {
        return rejected_outcome(state, EngineRejection::InvalidPhase);
    }

    let Some(pending_request_id) = state.pending_model_request.as_ref() else {
        return rejected_outcome(state, EngineRejection::InvalidPhase);
    };
    if pending_request_id != request_id {
        return rejected_outcome(state, EngineRejection::CorrelationMismatch);
    }

    let next_state = EngineState {
        contract_version: state.contract_version,
        core_state: state.core_state.clone(),
        phase: EngineTurnPhase::Finished,
        messages: state.messages.clone(),
        request_template: None,
        pending_model_request: None,
        next_model_request_sequence: state.next_model_request_sequence,
        pending_tool_calls: Vec::new(),
        buffered_tool_results: Vec::new(),
    };
    EngineOutcome {
        next_state,
        effects: vec![
            EngineEffect::EmitEvent(EngineEvent::BusyChanged { busy: false }),
            EngineEffect::EmitEvent(EngineEvent::Notice {
                message: error.to_string(),
            }),
            EngineEffect::EmitEvent(EngineEvent::TurnFinished {
                stop_reason: StopReason::Stop,
            }),
        ],
        rejection: None,
    }
}

#[must_use]
fn apply_tool_feedback(
    state: &EngineState,
    call_id: &EngineCorrelationId,
    result: &[Content],
    is_error: bool,
) -> EngineOutcome {
    if state.phase != EngineTurnPhase::WaitingForTools {
        return rejected_outcome(state, EngineRejection::InvalidPhase);
    }
    if !state.pending_tool_calls.iter().any(|pending_call_id| pending_call_id == call_id) {
        return rejected_outcome(state, EngineRejection::CorrelationMismatch);
    }
    if state
        .buffered_tool_results
        .iter()
        .any(|buffered_result| buffered_result.call_id == *call_id)
    {
        return rejected_outcome(state, EngineRejection::CorrelationMismatch);
    }

    let mut buffered_tool_results = state.buffered_tool_results.clone();
    buffered_tool_results.push(EngineBufferedToolResult {
        call_id: call_id.clone(),
        content: result.to_vec(),
        is_error,
    });

    if buffered_tool_results.len() < state.pending_tool_calls.len() {
        let next_state = EngineState {
            contract_version: state.contract_version,
            core_state: state.core_state.clone(),
            phase: EngineTurnPhase::WaitingForTools,
            messages: state.messages.clone(),
            request_template: state.request_template.clone(),
            pending_model_request: None,
            next_model_request_sequence: state.next_model_request_sequence,
            pending_tool_calls: state.pending_tool_calls.clone(),
            buffered_tool_results,
        };
        return EngineOutcome {
            next_state,
            effects: Vec::new(),
            rejection: None,
        };
    }

    let Some(request_template) = state.request_template.as_ref() else {
        return rejected_outcome(state, EngineRejection::InvalidPhase);
    };

    let mut next_messages = state.messages.clone();
    for pending_call_id in &state.pending_tool_calls {
        let Some(buffered_result) = buffered_tool_results
            .iter()
            .find(|candidate| candidate.call_id == *pending_call_id)
        else {
            return rejected_outcome(state, EngineRejection::CorrelationMismatch);
        };
        next_messages.push(buffered_tool_result_to_message(buffered_result));
    }

    let (request_id, next_model_request_sequence) = mint_model_request_id(state.next_model_request_sequence);
    let model_request = build_model_request(request_template, &next_messages, request_id.clone());
    let next_state = EngineState {
        contract_version: state.contract_version,
        core_state: state.core_state.clone(),
        phase: EngineTurnPhase::WaitingForModel,
        messages: next_messages,
        request_template: Some(request_template.clone()),
        pending_model_request: Some(request_id.clone()),
        next_model_request_sequence,
        pending_tool_calls: Vec::new(),
        buffered_tool_results: Vec::new(),
    };
    EngineOutcome {
        next_state,
        effects: vec![EngineEffect::RequestModel(model_request)],
        rejection: None,
    }
}

#[must_use]
fn apply_cancel_turn(state: &EngineState, reason: &str) -> EngineOutcome {
    if !matches!(state.phase, EngineTurnPhase::WaitingForModel | EngineTurnPhase::WaitingForTools) {
        return rejected_outcome(state, EngineRejection::InvalidPhase);
    }

    let next_state = EngineState {
        contract_version: state.contract_version,
        core_state: state.core_state.clone(),
        phase: EngineTurnPhase::Finished,
        messages: state.messages.clone(),
        request_template: None,
        pending_model_request: None,
        next_model_request_sequence: state.next_model_request_sequence,
        pending_tool_calls: Vec::new(),
        buffered_tool_results: Vec::new(),
    };
    EngineOutcome {
        next_state,
        effects: vec![
            EngineEffect::EmitEvent(EngineEvent::BusyChanged { busy: false }),
            EngineEffect::EmitEvent(EngineEvent::Notice {
                message: reason.to_string(),
            }),
            EngineEffect::EmitEvent(EngineEvent::TurnFinished {
                stop_reason: StopReason::Stop,
            }),
        ],
        rejection: None,
    }
}

fn rejected_outcome(state: &EngineState, rejection: EngineRejection) -> EngineOutcome {
    EngineOutcome {
        next_state: state.clone(),
        effects: Vec::new(),
        rejection: Some(rejection),
    }
}

fn canonical_messages_from_agent_messages(messages: &[AgentMessage]) -> Vec<EngineMessage> {
    messages
        .iter()
        .filter_map(|message| match message {
            AgentMessage::User(user) => Some(EngineMessage {
                role: EngineMessageRole::User,
                content: user.content.clone(),
            }),
            AgentMessage::Assistant(assistant) => Some(EngineMessage {
                role: EngineMessageRole::Assistant,
                content: assistant.content.clone(),
            }),
            AgentMessage::ToolResult(tool_result) => Some(EngineMessage {
                role: EngineMessageRole::Tool,
                content: vec![Content::ToolResult {
                    tool_use_id: tool_result.call_id.clone(),
                    content: tool_result.content.clone(),
                    is_error: if tool_result.is_error { Some(true) } else { None },
                }],
            }),
            AgentMessage::BashExecution(_)
            | AgentMessage::Custom(_)
            | AgentMessage::BranchSummary(_)
            | AgentMessage::CompactionSummary(_) => None,
        })
        .collect()
}

fn build_model_request(
    request_template: &EngineRequestTemplate,
    messages: &[EngineMessage],
    request_id: EngineCorrelationId,
) -> EngineModelRequest {
    EngineModelRequest {
        request_id,
        model: request_template.model.clone(),
        messages: messages.to_vec(),
        system_prompt: request_template.system_prompt.clone(),
        max_tokens: request_template.max_tokens,
        temperature: request_template.temperature,
        thinking: request_template.thinking.clone(),
        tools: request_template.tools.clone(),
        no_cache: request_template.no_cache,
        cache_ttl: request_template.cache_ttl.clone(),
        session_id: request_template.session_id.clone(),
    }
}

fn buffered_tool_result_to_message(buffered_result: &EngineBufferedToolResult) -> EngineMessage {
    EngineMessage {
        role: EngineMessageRole::Tool,
        content: vec![Content::ToolResult {
            tool_use_id: buffered_result.call_id.0.clone(),
            content: buffered_result.content.clone(),
            is_error: if buffered_result.is_error { Some(true) } else { None },
        }],
    }
}

fn extract_tool_calls(output: &[Content]) -> Vec<EngineToolCall> {
    output
        .iter()
        .filter_map(|content| match content {
            Content::ToolUse { id, name, input } => Some(EngineToolCall {
                call_id: EngineCorrelationId(id.clone()),
                tool_name: name.clone(),
                input: input.clone(),
            }),
            _ => None,
        })
        .collect()
}

fn mint_model_request_id(sequence: u32) -> (EngineCorrelationId, u32) {
    let request_id = EngineCorrelationId(format!("{}-{}", ENGINE_MODEL_REQUEST_PREFIX, sequence));
    let next_sequence = sequence + ENGINE_CORRELATION_SEQUENCE_STEP;
    (request_id, next_sequence)
}

#[cfg(test)]
mod tests {
    use clanker_message::MessageId;
    use clanker_message::ToolResultMessage;
    use clanker_message::UserMessage;
    use serde_json::json;

    use super::*;

    const INITIAL_REQUEST_EFFECT_COUNT: usize = 3;
    const TERMINAL_EFFECT_COUNT: usize = 2;
    const CANCELLATION_EFFECT_COUNT: usize = 3;
    const INITIAL_CANONICAL_MESSAGE_COUNT: usize = 1;
    const TOOL_USE_MESSAGE_COUNT: usize = 2;
    const FOLLOW_UP_CANONICAL_MESSAGE_COUNT: usize = 4;
    const MAX_TOKENS: usize = 100;
    const FOLLOW_UP_REQUEST_SEQUENCE: u32 = ENGINE_INITIAL_MODEL_REQUEST_SEQUENCE + ENGINE_CORRELATION_SEQUENCE_STEP;

    fn test_timestamp() -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now()
    }

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

    fn request_model_effect(outcome: &EngineOutcome) -> &EngineModelRequest {
        outcome
            .effects
            .iter()
            .find_map(|effect| match effect {
                EngineEffect::RequestModel(model_effect) => Some(model_effect),
                _ => None,
            })
            .expect("expected RequestModel effect")
    }

    fn expected_model_request_id(sequence: u32) -> EngineCorrelationId {
        EngineCorrelationId(format!("{}-{}", ENGINE_MODEL_REQUEST_PREFIX, sequence))
    }

    fn submitted_state() -> (EngineState, EngineCorrelationId) {
        let input = EngineInput::SubmitUserPrompt {
            submission: submission_with_session("session-123"),
        };
        let outcome = reduce(&EngineState::new(), &input);
        let request_id = request_model_effect(&outcome).request_id.clone();
        (outcome.next_state, request_id)
    }

    fn waiting_for_tools_state() -> EngineState {
        let (state, request_id) = submitted_state();
        let response = EngineModelResponse {
            output: vec![
                Content::ToolUse {
                    id: "call-1".to_string(),
                    name: "read".to_string(),
                    input: json!({"path": "src/main.rs"}),
                },
                Content::ToolUse {
                    id: "call-2".to_string(),
                    name: "read".to_string(),
                    input: json!({"path": "README.md"}),
                },
            ],
            stop_reason: StopReason::ToolUse,
        };
        let outcome = reduce(
            &state,
            &EngineInput::ModelCompleted {
                request_id,
                response,
            },
        );
        outcome.next_state
    }

    #[test]
    fn submit_user_prompt_builds_request_effect() {
        let state = EngineState::new();
        let outcome = reduce(
            &state,
            &EngineInput::SubmitUserPrompt {
                submission: submission_with_session("session-123"),
            },
        );

        assert!(outcome.rejection.is_none());
        assert_eq!(outcome.next_state.phase, EngineTurnPhase::WaitingForModel);
        assert_eq!(
            outcome.next_state.pending_model_request,
            Some(expected_model_request_id(ENGINE_INITIAL_MODEL_REQUEST_SEQUENCE))
        );
        assert_eq!(outcome.next_state.messages.len(), INITIAL_CANONICAL_MESSAGE_COUNT);
        assert_eq!(outcome.effects.len(), INITIAL_REQUEST_EFFECT_COUNT);

        let model_effect = request_model_effect(&outcome);
        assert_eq!(model_effect.request_id, expected_model_request_id(ENGINE_INITIAL_MODEL_REQUEST_SEQUENCE));
        assert_eq!(model_effect.model, "test-model");
        assert_eq!(model_effect.messages.len(), INITIAL_CANONICAL_MESSAGE_COUNT);
        assert_eq!(model_effect.session_id, "session-123");
    }

    #[test]
    fn submit_user_prompt_rejects_busy_state() {
        let mut state = EngineState::new();
        state.phase = EngineTurnPhase::WaitingForModel;
        state.pending_model_request = Some(EngineCorrelationId("existing".to_string()));

        let outcome = reduce(
            &state,
            &EngineInput::SubmitUserPrompt {
                submission: submission_with_session("session-123"),
            },
        );

        assert!(outcome.effects.is_empty());
        assert_eq!(outcome.rejection, Some(EngineRejection::Busy));
        assert_eq!(outcome.next_state.phase, EngineTurnPhase::WaitingForModel);
        assert_eq!(outcome.next_state.pending_model_request, Some(EngineCorrelationId("existing".to_string())));
    }

    #[test]
    fn submit_user_prompt_preserves_empty_session_id() {
        let outcome = reduce(
            &EngineState::new(),
            &EngineInput::SubmitUserPrompt {
                submission: submission_with_session(""),
            },
        );

        let model_effect = request_model_effect(&outcome);
        assert!(model_effect.session_id.is_empty());
    }

    #[test]
    fn model_completion_schedules_tool_effects_for_tool_use_stop() {
        let (state, request_id) = submitted_state();
        let response = EngineModelResponse {
            output: vec![Content::ToolUse {
                id: "call-1".to_string(),
                name: "read".to_string(),
                input: json!({"path": "src/main.rs"}),
            }],
            stop_reason: StopReason::ToolUse,
        };

        let outcome = reduce(
            &state,
            &EngineInput::ModelCompleted {
                request_id,
                response,
            },
        );

        assert!(outcome.rejection.is_none());
        assert_eq!(outcome.next_state.phase, EngineTurnPhase::WaitingForTools);
        assert_eq!(outcome.next_state.pending_model_request, None);
        assert_eq!(outcome.next_state.messages.len(), TOOL_USE_MESSAGE_COUNT);
        assert_eq!(outcome.next_state.pending_tool_calls, vec![EngineCorrelationId("call-1".to_string())]);
        assert!(outcome.effects.iter().all(|effect| matches!(effect, EngineEffect::ExecuteTool(_))));
        assert_eq!(outcome.effects.len(), 1);
        let EngineEffect::ExecuteTool(tool_effect) = &outcome.effects[0] else {
            panic!("expected ExecuteTool effect");
        };
        assert_eq!(tool_effect.call_id, EngineCorrelationId("call-1".to_string()));
        assert_eq!(tool_effect.tool_name, "read");
        assert_eq!(tool_effect.input, json!({"path": "src/main.rs"}));
    }

    #[test]
    fn model_completion_finishes_turn_for_terminal_stop_reason() {
        let (state, request_id) = submitted_state();
        let response = EngineModelResponse {
            output: vec![Content::Text {
                text: "done".to_string(),
            }],
            stop_reason: StopReason::Stop,
        };

        let outcome = reduce(
            &state,
            &EngineInput::ModelCompleted {
                request_id,
                response,
            },
        );

        assert!(outcome.rejection.is_none());
        assert_eq!(outcome.next_state.phase, EngineTurnPhase::Finished);
        assert_eq!(outcome.next_state.messages.len(), TOOL_USE_MESSAGE_COUNT);
        assert!(outcome.next_state.request_template.is_none());
        assert_eq!(outcome.effects.len(), TERMINAL_EFFECT_COUNT);
        assert!(matches!(&outcome.effects[0], EngineEffect::EmitEvent(EngineEvent::BusyChanged { busy: false })));
        assert!(matches!(
            &outcome.effects[1],
            EngineEffect::EmitEvent(EngineEvent::TurnFinished {
                stop_reason: StopReason::Stop,
            })
        ));
    }

    #[test]
    fn model_completion_rejects_mismatched_request_id() {
        let (state, _) = submitted_state();
        let response = EngineModelResponse {
            output: Vec::new(),
            stop_reason: StopReason::Stop,
        };

        let outcome = reduce(
            &state,
            &EngineInput::ModelCompleted {
                request_id: EngineCorrelationId("wrong".to_string()),
                response,
            },
        );

        assert!(outcome.effects.is_empty());
        assert_eq!(outcome.rejection, Some(EngineRejection::CorrelationMismatch));
        assert_eq!(outcome.next_state.phase, EngineTurnPhase::WaitingForModel);
    }

    #[test]
    fn model_completion_rejects_invalid_phase() {
        let response = EngineModelResponse {
            output: Vec::new(),
            stop_reason: StopReason::Stop,
        };

        let outcome = reduce(
            &EngineState::new(),
            &EngineInput::ModelCompleted {
                request_id: expected_model_request_id(ENGINE_INITIAL_MODEL_REQUEST_SEQUENCE),
                response,
            },
        );

        assert!(outcome.effects.is_empty());
        assert_eq!(outcome.rejection, Some(EngineRejection::InvalidPhase));
        assert_eq!(outcome.next_state.phase, EngineTurnPhase::Idle);
    }

    #[test]
    fn model_completion_rejects_tool_use_without_tool_call() {
        let (state, request_id) = submitted_state();
        let response = EngineModelResponse {
            output: vec![Content::Text {
                text: "missing tool".to_string(),
            }],
            stop_reason: StopReason::ToolUse,
        };

        let outcome = reduce(
            &state,
            &EngineInput::ModelCompleted {
                request_id,
                response,
            },
        );

        assert!(outcome.effects.is_empty());
        assert_eq!(outcome.rejection, Some(EngineRejection::MissingToolCall));
        assert_eq!(outcome.next_state.phase, EngineTurnPhase::WaitingForModel);
    }

    #[test]
    fn tool_feedback_waits_for_all_pending_results_before_continuing() {
        let state = waiting_for_tools_state();
        let partial_outcome = reduce(
            &state,
            &EngineInput::ToolCompleted {
                call_id: EngineCorrelationId("call-2".to_string()),
                result: vec![Content::Text {
                    text: "second result".to_string(),
                }],
            },
        );

        assert!(partial_outcome.rejection.is_none());
        assert_eq!(partial_outcome.next_state.phase, EngineTurnPhase::WaitingForTools);
        assert!(partial_outcome.effects.is_empty());
        assert_eq!(partial_outcome.next_state.buffered_tool_results.len(), 1);

        let final_outcome = reduce(
            &partial_outcome.next_state,
            &EngineInput::ToolFailed {
                call_id: EngineCorrelationId("call-1".to_string()),
                error: "tool failed".to_string(),
                result: vec![Content::Text {
                    text: "tool failed".to_string(),
                }],
            },
        );

        assert!(final_outcome.rejection.is_none());
        assert_eq!(final_outcome.next_state.phase, EngineTurnPhase::WaitingForModel);
        assert_eq!(
            final_outcome.next_state.pending_model_request,
            Some(expected_model_request_id(FOLLOW_UP_REQUEST_SEQUENCE))
        );
        assert!(final_outcome.next_state.pending_tool_calls.is_empty());
        assert!(final_outcome.next_state.buffered_tool_results.is_empty());
        assert_eq!(final_outcome.next_state.messages.len(), FOLLOW_UP_CANONICAL_MESSAGE_COUNT);

        let follow_up_request = request_model_effect(&final_outcome);
        assert_eq!(follow_up_request.request_id, expected_model_request_id(FOLLOW_UP_REQUEST_SEQUENCE));
        assert_eq!(follow_up_request.messages.len(), FOLLOW_UP_CANONICAL_MESSAGE_COUNT);
        let EngineMessage {
            role: first_tool_role,
            content: first_tool_content,
        } = &follow_up_request.messages[TOOL_USE_MESSAGE_COUNT];
        assert_eq!(*first_tool_role, EngineMessageRole::Tool);
        let Content::ToolResult {
            tool_use_id,
            is_error,
            ..
        } = &first_tool_content[0]
        else {
            panic!("expected first tool result content block");
        };
        assert_eq!(tool_use_id, "call-1");
        assert_eq!(*is_error, Some(true));
    }

    #[test]
    fn tool_feedback_rejects_unknown_call_id() {
        let state = waiting_for_tools_state();
        let outcome = reduce(
            &state,
            &EngineInput::ToolCompleted {
                call_id: EngineCorrelationId("wrong".to_string()),
                result: Vec::new(),
            },
        );

        assert!(outcome.effects.is_empty());
        assert_eq!(outcome.rejection, Some(EngineRejection::CorrelationMismatch));
        assert_eq!(outcome.next_state.phase, EngineTurnPhase::WaitingForTools);
    }

    #[test]
    fn tool_feedback_rejects_duplicate_call_id() {
        let state = waiting_for_tools_state();
        let partial_outcome = reduce(
            &state,
            &EngineInput::ToolCompleted {
                call_id: EngineCorrelationId("call-1".to_string()),
                result: vec![Content::Text {
                    text: "result".to_string(),
                }],
            },
        );
        let duplicate_outcome = reduce(
            &partial_outcome.next_state,
            &EngineInput::ToolCompleted {
                call_id: EngineCorrelationId("call-1".to_string()),
                result: vec![Content::Text {
                    text: "result".to_string(),
                }],
            },
        );

        assert!(duplicate_outcome.effects.is_empty());
        assert_eq!(duplicate_outcome.rejection, Some(EngineRejection::CorrelationMismatch));
        assert_eq!(duplicate_outcome.next_state.phase, EngineTurnPhase::WaitingForTools);
    }

    #[test]
    fn tool_feedback_rejects_wrong_phase() {
        let outcome = reduce(
            &EngineState::new(),
            &EngineInput::ToolCompleted {
                call_id: EngineCorrelationId("call-1".to_string()),
                result: Vec::new(),
            },
        );

        assert!(outcome.effects.is_empty());
        assert_eq!(outcome.rejection, Some(EngineRejection::InvalidPhase));
        assert_eq!(outcome.next_state.phase, EngineTurnPhase::Idle);
    }

    #[test]
    fn cancel_turn_terminalizes_pending_model_work() {
        let (state, _) = submitted_state();
        let outcome = reduce(
            &state,
            &EngineInput::CancelTurn {
                reason: "cancelled".to_string(),
            },
        );

        assert!(outcome.rejection.is_none());
        assert_eq!(outcome.next_state.phase, EngineTurnPhase::Finished);
        assert!(outcome.next_state.pending_model_request.is_none());
        assert!(outcome.next_state.pending_tool_calls.is_empty());
        assert_eq!(outcome.effects.len(), CANCELLATION_EFFECT_COUNT);
    }

    #[test]
    fn cancel_turn_terminalizes_pending_tool_work() {
        let state = waiting_for_tools_state();
        let outcome = reduce(
            &state,
            &EngineInput::CancelTurn {
                reason: "cancelled".to_string(),
            },
        );

        assert!(outcome.rejection.is_none());
        assert_eq!(outcome.next_state.phase, EngineTurnPhase::Finished);
        assert!(outcome.next_state.pending_model_request.is_none());
        assert!(outcome.next_state.pending_tool_calls.is_empty());
        assert_eq!(outcome.effects.len(), CANCELLATION_EFFECT_COUNT);
    }

    #[test]
    fn cancel_turn_rejects_idle_phase() {
        let outcome = reduce(
            &EngineState::new(),
            &EngineInput::CancelTurn {
                reason: "cancelled".to_string(),
            },
        );

        assert!(outcome.effects.is_empty());
        assert_eq!(outcome.rejection, Some(EngineRejection::InvalidPhase));
        assert_eq!(outcome.next_state.phase, EngineTurnPhase::Idle);
    }

    #[test]
    fn model_failed_terminalizes_pending_request() {
        let (state, request_id) = submitted_state();
        let outcome = reduce(
            &state,
            &EngineInput::ModelFailed {
                request_id,
                error: "provider failed".to_string(),
            },
        );

        assert!(outcome.rejection.is_none());
        assert_eq!(outcome.next_state.phase, EngineTurnPhase::Finished);
        assert!(outcome.next_state.pending_model_request.is_none());
        assert!(outcome.next_state.request_template.is_none());
        assert_eq!(outcome.effects.len(), CANCELLATION_EFFECT_COUNT);
    }

    #[test]
    fn submit_user_prompt_strips_non_conversation_metadata_messages() {
        let mut submission = submission_with_session("session-123");
        submission.messages.push(AgentMessage::ToolResult(ToolResultMessage {
            id: MessageId::new("tool-1"),
            call_id: "call-1".to_string(),
            tool_name: "read".to_string(),
            content: vec![Content::Text {
                text: "tool output".to_string(),
            }],
            is_error: false,
            details: None,
            timestamp: test_timestamp(),
        }));
        submission.messages.push(AgentMessage::Custom(clanker_message::CustomMessage {
            id: MessageId::new("custom-1"),
            kind: "meta".to_string(),
            data: json!({"ignored": true}),
            timestamp: test_timestamp(),
        }));

        let outcome = reduce(
            &EngineState::new(),
            &EngineInput::SubmitUserPrompt { submission },
        );

        let model_effect = request_model_effect(&outcome);
        assert_eq!(model_effect.messages.len(), 2);
        assert_eq!(model_effect.messages[0].role, EngineMessageRole::User);
        assert_eq!(model_effect.messages[1].role, EngineMessageRole::Tool);
    }
}
