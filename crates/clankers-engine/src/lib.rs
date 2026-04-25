//! Host-facing reusable engine contracts for model/tool turn policy that compose
//! alongside `clankers-core` through controller/agent adapter seams.
//!
//! Ownership matrix:
//!
//! - `clankers-core` owns prompt lifecycle, queued prompt replay, loop/auto-test follow-up
//!   dispatch/completion, thinking-level state, disabled-tool filters, and cancellation before
//!   accepted work reaches the engine.
//! - `clankers-engine` owns accepted model/tool turn policy: model request correlation, model
//!   completion/failure, tool-call planning, tool feedback, retry scheduling, continuation budget,
//!   cancellation during model/tool/retry phases, and terminal turn outcomes.
//! - Adapters hold any core lifecycle correlation such as `CoreEffectId` and feed core completion
//!   feedback back to `clankers-core`; engine state stays free of core reducer state and core
//!   lifecycle IDs.

use core::time::Duration;

use clanker_message::Content;
use clanker_message::StopReason;
use clanker_message::ThinkingConfig;
use clanker_message::ToolDefinition;
use serde_json::Value;

pub const ENGINE_CONTRACT_VERSION: u32 = 1;
pub const ENGINE_MODEL_REQUEST_PREFIX: &str = "model-request";
pub const ENGINE_INITIAL_MODEL_REQUEST_SEQUENCE: u32 = 1;
pub const ENGINE_CORRELATION_SEQUENCE_STEP: u32 = 1;
pub const ENGINE_SUBMIT_PROMPT_NOTICE: &str = "engine queued initial model request";

pub const ENGINE_DEFAULT_RETRY_DELAY_COUNT: usize = 2;
pub const ENGINE_FIRST_RETRY_DELAY_SECONDS: u64 = 1;
pub const ENGINE_SECOND_RETRY_DELAY_SECONDS: u64 = 4;
pub const ENGINE_DEFAULT_RETRY_DELAYS: [Duration; ENGINE_DEFAULT_RETRY_DELAY_COUNT] = [
    Duration::from_secs(ENGINE_FIRST_RETRY_DELAY_SECONDS),
    Duration::from_secs(ENGINE_SECOND_RETRY_DELAY_SECONDS),
];
pub const ENGINE_MIN_MODEL_REQUEST_SLOT_BUDGET: u32 = 1;
pub const ENGINE_MODEL_REQUEST_SLOT_COST: u32 = 1;
pub const ENGINE_BUDGET_EXHAUSTED_NOTICE: &str = "engine model request slot budget exhausted";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EngineCorrelationId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineTurnPhase {
    Idle,
    WaitingForModel,
    WaitingForRetry,
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
pub struct EngineRetryPolicy {
    pub retry_delays: Vec<Duration>,
}

impl Default for EngineRetryPolicy {
    fn default() -> Self {
        Self {
            retry_delays: ENGINE_DEFAULT_RETRY_DELAYS.to_vec(),
        }
    }
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
    pub retry_policy: EngineRetryPolicy,
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
pub struct EngineTerminalFailure {
    pub message: String,
    pub status: Option<u16>,
    pub retryable: bool,
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
    ScheduleRetry {
        request_id: EngineCorrelationId,
        delay: Duration,
    },
    EmitEvent(EngineEvent),
}

#[derive(Debug, Clone)]
pub enum EngineInput {
    SubmitUserPrompt {
        submission: EnginePromptSubmission,
    },
    ModelCompleted {
        request_id: EngineCorrelationId,
        response: EngineModelResponse,
    },
    ModelFailed {
        request_id: EngineCorrelationId,
        failure: EngineTerminalFailure,
    },
    RetryReady {
        request_id: EngineCorrelationId,
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

impl EngineInput {
    #[must_use]
    pub fn submit_user_prompt(submission: EnginePromptSubmission) -> Self {
        Self::SubmitUserPrompt { submission }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineRejection {
    Busy,
    CorrelationMismatch,
    InvalidPhase,
    MissingToolCall,
    InvalidBudget,
}
#[derive(Debug, Clone)]
pub struct EngineState {
    pub contract_version: u32,
    pub phase: EngineTurnPhase,
    pub messages: Vec<EngineMessage>,
    pub request_template: Option<EngineRequestTemplate>,
    pub pending_model_request: Option<EngineCorrelationId>,
    pub next_model_request_sequence: u32,
    pub pending_tool_calls: Vec<EngineCorrelationId>,
    pub buffered_tool_results: Vec<EngineBufferedToolResult>,
    pub retry_attempts_for_pending_model_request: u32,
    pub model_request_slot_budget: u32,
    pub model_request_slots_used: u32,
}

#[derive(Debug, Clone)]
pub struct EngineOutcome {
    pub next_state: EngineState,
    pub effects: Vec<EngineEffect>,
    pub rejection: Option<EngineRejection>,
    pub terminal_failure: Option<EngineTerminalFailure>,
}

#[derive(Debug, Clone)]
pub struct EnginePromptSubmission {
    pub messages: Vec<EngineMessage>,
    pub model: String,
    pub system_prompt: String,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f64>,
    pub thinking: Option<ThinkingConfig>,
    pub tools: Vec<ToolDefinition>,
    pub no_cache: bool,
    pub cache_ttl: Option<String>,
    pub session_id: String,
    pub model_request_slot_budget: u32,
}

impl EngineState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            contract_version: ENGINE_CONTRACT_VERSION,
            phase: EngineTurnPhase::Idle,
            messages: Vec::new(),
            request_template: None,
            pending_model_request: None,
            next_model_request_sequence: ENGINE_INITIAL_MODEL_REQUEST_SEQUENCE,
            pending_tool_calls: Vec::new(),
            buffered_tool_results: Vec::new(),
            retry_attempts_for_pending_model_request: 0,
            model_request_slot_budget: 0,
            model_request_slots_used: 0,
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
        EngineInput::ModelCompleted { request_id, response } => apply_model_completion(state, request_id, response),
        EngineInput::ModelFailed { request_id, failure } => apply_model_failed(state, request_id, failure),
        EngineInput::RetryReady { request_id } => apply_retry_ready(state, request_id),
        EngineInput::ToolCompleted { call_id, result } => apply_tool_feedback(state, call_id, result, false),
        EngineInput::ToolFailed { call_id, error, result } => {
            let tool_result_content = if result.is_empty() {
                vec![Content::Text { text: error.clone() }]
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
    if submission.model_request_slot_budget < ENGINE_MIN_MODEL_REQUEST_SLOT_BUDGET {
        return rejected_outcome(state, EngineRejection::InvalidBudget);
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
        retry_policy: EngineRetryPolicy::default(),
    };
    let canonical_messages = submission.messages.clone();
    let (request_id, next_model_request_sequence) = mint_model_request_id(state.next_model_request_sequence);
    let model_request = build_model_request(&request_template, &canonical_messages, request_id.clone());

    let next_state = EngineState {
        contract_version: state.contract_version,
        phase: EngineTurnPhase::WaitingForModel,
        messages: canonical_messages,
        request_template: Some(request_template),
        pending_model_request: Some(request_id.clone()),
        next_model_request_sequence,
        pending_tool_calls: Vec::new(),
        buffered_tool_results: Vec::new(),
        retry_attempts_for_pending_model_request: 0,
        model_request_slot_budget: submission.model_request_slot_budget,
        model_request_slots_used: ENGINE_MODEL_REQUEST_SLOT_COST,
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
        terminal_failure: None,
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

    match response.stop_reason {
        StopReason::ToolUse => apply_tool_use_model_completion(state, next_messages, &response.output),
        StopReason::MaxTokens => terminal_success_outcome(state, next_messages, StopReason::MaxTokens),
        _ => terminal_success_outcome(state, next_messages, response.stop_reason.clone()),
    }
}

#[must_use]
fn apply_tool_use_model_completion(
    state: &EngineState,
    next_messages: Vec<EngineMessage>,
    output: &[Content],
) -> EngineOutcome {
    let tool_calls = extract_tool_calls(output);
    if tool_calls.is_empty() {
        return rejected_outcome(state, EngineRejection::MissingToolCall);
    }

    let pending_tool_calls = tool_calls.iter().map(|call| call.call_id.clone()).collect();
    let next_state = EngineState {
        contract_version: state.contract_version,
        phase: EngineTurnPhase::WaitingForTools,
        messages: next_messages,
        request_template: state.request_template.clone(),
        pending_model_request: None,
        next_model_request_sequence: state.next_model_request_sequence,
        pending_tool_calls,
        buffered_tool_results: Vec::new(),
        retry_attempts_for_pending_model_request: 0,
        model_request_slot_budget: state.model_request_slot_budget,
        model_request_slots_used: state.model_request_slots_used,
    };
    EngineOutcome {
        next_state,
        effects: tool_calls.into_iter().map(EngineEffect::ExecuteTool).collect(),
        rejection: None,
        terminal_failure: None,
    }
}

#[must_use]
fn apply_model_failed(
    state: &EngineState,
    request_id: &EngineCorrelationId,
    failure: &EngineTerminalFailure,
) -> EngineOutcome {
    if state.phase == EngineTurnPhase::WaitingForRetry {
        return retry_wait_feedback_rejection(state, request_id);
    }
    if state.phase != EngineTurnPhase::WaitingForModel {
        return rejected_outcome(state, EngineRejection::InvalidPhase);
    }

    let Some(pending_request_id) = state.pending_model_request.as_ref() else {
        return rejected_outcome(state, EngineRejection::InvalidPhase);
    };
    if pending_request_id != request_id {
        return rejected_outcome(state, EngineRejection::CorrelationMismatch);
    }

    if failure.retryable
        && let Some(delay) = retry_delay_for_attempt(state)
    {
        let next_state = EngineState {
            contract_version: state.contract_version,
            phase: EngineTurnPhase::WaitingForRetry,
            messages: state.messages.clone(),
            request_template: state.request_template.clone(),
            pending_model_request: Some(request_id.clone()),
            next_model_request_sequence: state.next_model_request_sequence,
            pending_tool_calls: Vec::new(),
            buffered_tool_results: Vec::new(),
            retry_attempts_for_pending_model_request: state.retry_attempts_for_pending_model_request + 1,
            model_request_slot_budget: state.model_request_slot_budget,
            model_request_slots_used: state.model_request_slots_used,
        };
        return EngineOutcome {
            next_state,
            effects: vec![EngineEffect::ScheduleRetry {
                request_id: request_id.clone(),
                delay,
            }],
            rejection: None,
            terminal_failure: None,
        };
    }

    terminal_failure_outcome(state, failure.clone())
}

#[must_use]
fn apply_retry_ready(state: &EngineState, request_id: &EngineCorrelationId) -> EngineOutcome {
    if state.phase != EngineTurnPhase::WaitingForRetry {
        return rejected_outcome(state, EngineRejection::InvalidPhase);
    }
    let Some(pending_request_id) = state.pending_model_request.as_ref() else {
        return rejected_outcome(state, EngineRejection::InvalidPhase);
    };
    if pending_request_id != request_id {
        return rejected_outcome(state, EngineRejection::CorrelationMismatch);
    }
    let Some(request_template) = state.request_template.as_ref() else {
        return rejected_outcome(state, EngineRejection::InvalidPhase);
    };

    let model_request = build_model_request(request_template, &state.messages, request_id.clone());
    let next_state = EngineState {
        contract_version: state.contract_version,
        phase: EngineTurnPhase::WaitingForModel,
        messages: state.messages.clone(),
        request_template: state.request_template.clone(),
        pending_model_request: Some(request_id.clone()),
        next_model_request_sequence: state.next_model_request_sequence,
        pending_tool_calls: Vec::new(),
        buffered_tool_results: Vec::new(),
        retry_attempts_for_pending_model_request: state.retry_attempts_for_pending_model_request,
        model_request_slot_budget: state.model_request_slot_budget,
        model_request_slots_used: state.model_request_slots_used,
    };
    EngineOutcome {
        next_state,
        effects: vec![EngineEffect::RequestModel(model_request)],
        rejection: None,
        terminal_failure: None,
    }
}

#[must_use]
fn retry_wait_feedback_rejection(state: &EngineState, request_id: &EngineCorrelationId) -> EngineOutcome {
    let Some(pending_request_id) = state.pending_model_request.as_ref() else {
        return rejected_outcome(state, EngineRejection::InvalidPhase);
    };
    if pending_request_id != request_id {
        return rejected_outcome(state, EngineRejection::CorrelationMismatch);
    }
    rejected_outcome(state, EngineRejection::InvalidPhase)
}

#[must_use]
fn retry_delay_for_attempt(state: &EngineState) -> Option<Duration> {
    let request_template = state.request_template.as_ref()?;
    request_template
        .retry_policy
        .retry_delays
        .get(state.retry_attempts_for_pending_model_request as usize)
        .copied()
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
    if state.buffered_tool_results.iter().any(|buffered_result| buffered_result.call_id == *call_id) {
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
            phase: EngineTurnPhase::WaitingForTools,
            messages: state.messages.clone(),
            request_template: state.request_template.clone(),
            pending_model_request: None,
            next_model_request_sequence: state.next_model_request_sequence,
            pending_tool_calls: state.pending_tool_calls.clone(),
            buffered_tool_results,
            retry_attempts_for_pending_model_request: 0,
            model_request_slot_budget: state.model_request_slot_budget,
            model_request_slots_used: state.model_request_slots_used,
        };
        return EngineOutcome {
            next_state,
            effects: Vec::new(),
            rejection: None,
            terminal_failure: None,
        };
    }

    let Some(request_template) = state.request_template.as_ref() else {
        return rejected_outcome(state, EngineRejection::InvalidPhase);
    };

    let mut next_messages = state.messages.clone();
    for pending_call_id in &state.pending_tool_calls {
        let Some(buffered_result) =
            buffered_tool_results.iter().find(|candidate| candidate.call_id == *pending_call_id)
        else {
            return rejected_outcome(state, EngineRejection::CorrelationMismatch);
        };
        next_messages.push(buffered_tool_result_to_message(buffered_result));
    }

    if state.model_request_slots_used >= state.model_request_slot_budget {
        return terminal_notice_outcome(state, next_messages, ENGINE_BUDGET_EXHAUSTED_NOTICE, StopReason::Stop);
    }

    let (request_id, next_model_request_sequence) = mint_model_request_id(state.next_model_request_sequence);
    let model_request = build_model_request(request_template, &next_messages, request_id.clone());
    let next_state = EngineState {
        contract_version: state.contract_version,
        phase: EngineTurnPhase::WaitingForModel,
        messages: next_messages,
        request_template: Some(request_template.clone()),
        pending_model_request: Some(request_id.clone()),
        next_model_request_sequence,
        pending_tool_calls: Vec::new(),
        buffered_tool_results: Vec::new(),
        retry_attempts_for_pending_model_request: 0,
        model_request_slot_budget: state.model_request_slot_budget,
        model_request_slots_used: state.model_request_slots_used + ENGINE_MODEL_REQUEST_SLOT_COST,
    };
    EngineOutcome {
        next_state,
        effects: vec![EngineEffect::RequestModel(model_request)],
        rejection: None,
        terminal_failure: None,
    }
}

#[must_use]
fn apply_cancel_turn(state: &EngineState, reason: &str) -> EngineOutcome {
    if !matches!(
        state.phase,
        EngineTurnPhase::WaitingForModel | EngineTurnPhase::WaitingForRetry | EngineTurnPhase::WaitingForTools
    ) {
        return rejected_outcome(state, EngineRejection::InvalidPhase);
    }

    terminal_notice_outcome(state, state.messages.clone(), reason, StopReason::Stop)
}

fn terminal_success_outcome(
    state: &EngineState,
    messages: Vec<EngineMessage>,
    stop_reason: StopReason,
) -> EngineOutcome {
    EngineOutcome {
        next_state: terminal_state_with_messages(state, messages),
        effects: vec![
            EngineEffect::EmitEvent(EngineEvent::BusyChanged { busy: false }),
            EngineEffect::EmitEvent(EngineEvent::TurnFinished { stop_reason }),
        ],
        rejection: None,
        terminal_failure: None,
    }
}

fn terminal_notice_outcome(
    state: &EngineState,
    messages: Vec<EngineMessage>,
    notice: &str,
    stop_reason: StopReason,
) -> EngineOutcome {
    EngineOutcome {
        next_state: terminal_state_with_messages(state, messages),
        effects: vec![
            EngineEffect::EmitEvent(EngineEvent::BusyChanged { busy: false }),
            EngineEffect::EmitEvent(EngineEvent::Notice {
                message: notice.to_string(),
            }),
            EngineEffect::EmitEvent(EngineEvent::TurnFinished { stop_reason }),
        ],
        rejection: None,
        terminal_failure: None,
    }
}

fn terminal_failure_outcome(state: &EngineState, failure: EngineTerminalFailure) -> EngineOutcome {
    EngineOutcome {
        next_state: terminal_state_with_messages(state, state.messages.clone()),
        effects: vec![
            EngineEffect::EmitEvent(EngineEvent::BusyChanged { busy: false }),
            EngineEffect::EmitEvent(EngineEvent::Notice {
                message: failure.message.clone(),
            }),
            EngineEffect::EmitEvent(EngineEvent::TurnFinished {
                stop_reason: StopReason::Stop,
            }),
        ],
        rejection: None,
        terminal_failure: Some(failure),
    }
}

fn terminal_state_with_messages(state: &EngineState, messages: Vec<EngineMessage>) -> EngineState {
    EngineState {
        contract_version: state.contract_version,
        phase: EngineTurnPhase::Finished,
        messages,
        request_template: None,
        pending_model_request: None,
        next_model_request_sequence: state.next_model_request_sequence,
        pending_tool_calls: Vec::new(),
        buffered_tool_results: Vec::new(),
        retry_attempts_for_pending_model_request: 0,
        model_request_slot_budget: state.model_request_slot_budget,
        model_request_slots_used: state.model_request_slots_used,
    }
}

fn rejected_outcome(state: &EngineState, rejection: EngineRejection) -> EngineOutcome {
    EngineOutcome {
        next_state: state.clone(),
        effects: Vec::new(),
        rejection: Some(rejection),
        terminal_failure: None,
    }
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
    const FIRST_RETRY_DELAY_SECONDS: u64 = 1;
    const SECOND_RETRY_DELAY_SECONDS: u64 = 4;
    const RETRYABLE_STATUS: u16 = 502;
    const NON_RETRYABLE_STATUS: u16 = 400;
    const DEFAULT_TEST_MODEL_REQUEST_SLOT_BUDGET: u32 = 8;
    const TWO_MODEL_REQUEST_SLOT_BUDGET: u32 = 2;
    const ZERO_MODEL_REQUEST_SLOT_BUDGET: u32 = 0;
    const BUDGET_EXHAUSTED_NOTICE: &str = "engine model request slot budget exhausted";
    const TURN_CANCELLED_REASON: &str = "turn cancelled";
    const ENGINE_STATE_FIELD_STRUCT_START: &str = "pub struct EngineState {";
    const RUST_PUBLIC_FIELD_PREFIX: &str = "pub ";
    const RUST_FIELD_NAME_SEPARATOR: char = ':';
    const ENGINE_STATE_FIELD_INVENTORY: [(&str, &str); 11] = [
        ("contract_version", "checked by submit_user_prompt_builds_request_effect and terminal transitions"),
        ("phase", "checked by prompt, model, tool, retry, cancellation, and terminal phase tests"),
        ("messages", "checked by message evolution and no-mutation tests"),
        ("request_template", "checked by retry and continuation request tests"),
        ("pending_model_request", "checked by model correlation and retry tests"),
        ("next_model_request_sequence", "checked by follow-up request sequencing tests"),
        ("pending_tool_calls", "checked by tool planning and duplicate/unknown feedback tests"),
        ("buffered_tool_results", "checked by waits-for-all-tools and duplicate feedback tests"),
        ("retry_attempts_for_pending_model_request", "checked by retry scheduling and exhaustion tests"),
        ("model_request_slot_budget", "checked by zero-budget and continuation-budget tests"),
        ("model_request_slots_used", "checked by continuation-budget exhaustion tests"),
    ];

    fn submission_with_session(session_id: &str) -> EnginePromptSubmission {
        EnginePromptSubmission {
            messages: vec![EngineMessage {
                role: EngineMessageRole::User,
                content: vec![Content::Text {
                    text: "hello".to_string(),
                }],
            }],
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
            model_request_slot_budget: DEFAULT_TEST_MODEL_REQUEST_SLOT_BUDGET,
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

    fn engine_failure(message: &str, status: Option<u16>, retryable: bool) -> EngineTerminalFailure {
        EngineTerminalFailure {
            message: message.to_string(),
            status,
            retryable,
        }
    }

    fn retryable_failure(message: &str) -> EngineTerminalFailure {
        engine_failure(message, Some(RETRYABLE_STATUS), true)
    }

    fn non_retryable_failure(message: &str) -> EngineTerminalFailure {
        engine_failure(message, Some(NON_RETRYABLE_STATUS), false)
    }

    fn retry_ready_input(request_id: &EngineCorrelationId) -> EngineInput {
        EngineInput::RetryReady {
            request_id: request_id.clone(),
        }
    }

    fn model_failed_input(request_id: &EngineCorrelationId, failure: EngineTerminalFailure) -> EngineInput {
        EngineInput::ModelFailed {
            request_id: request_id.clone(),
            failure,
        }
    }

    fn text_model_response(text: &str, stop_reason: StopReason) -> EngineModelResponse {
        EngineModelResponse {
            output: vec![Content::Text { text: text.to_string() }],
            stop_reason,
        }
    }

    fn one_tool_use_response(call_id: &str) -> EngineModelResponse {
        EngineModelResponse {
            output: vec![Content::ToolUse {
                id: call_id.to_string(),
                name: "read".to_string(),
                input: json!({"path": "src/main.rs"}),
            }],
            stop_reason: StopReason::ToolUse,
        }
    }

    fn schedule_retry_effect(outcome: &EngineOutcome) -> (&EngineCorrelationId, core::time::Duration) {
        outcome
            .effects
            .iter()
            .find_map(|effect| match effect {
                EngineEffect::ScheduleRetry { request_id, delay } => Some((request_id, *delay)),
                _ => None,
            })
            .expect("expected ScheduleRetry effect")
    }

    fn no_request_model_effect(outcome: &EngineOutcome) -> bool {
        outcome.effects.iter().all(|effect| !matches!(effect, EngineEffect::RequestModel(_)))
    }

    fn no_retry_effect(outcome: &EngineOutcome) -> bool {
        outcome.effects.iter().all(|effect| !matches!(effect, EngineEffect::ScheduleRetry { .. }))
    }

    fn emitted_events(outcome: &EngineOutcome) -> Vec<EngineEvent> {
        outcome
            .effects
            .iter()
            .filter_map(|effect| match effect {
                EngineEffect::EmitEvent(event) => Some(event.clone()),
                _ => None,
            })
            .collect()
    }

    fn assert_terminal_failure_events(outcome: &EngineOutcome, notice: &str) {
        assert_eq!(emitted_events(outcome), vec![
            EngineEvent::BusyChanged { busy: false },
            EngineEvent::Notice {
                message: notice.to_string(),
            },
            EngineEvent::TurnFinished {
                stop_reason: StopReason::Stop,
            },
        ]);
    }

    fn assert_rejected_without_state_change(state: &EngineState, outcome: &EngineOutcome, rejection: EngineRejection) {
        assert_eq!(outcome.rejection, Some(rejection));
        assert!(outcome.effects.is_empty());
        assert!(outcome.terminal_failure.is_none());
        assert_eq!(format!("{:?}", outcome.next_state), format!("{:?}", state));
    }

    fn assert_messages_unchanged(actual: &[EngineMessage], expected: &[EngineMessage]) {
        assert_eq!(format!("{:?}", actual), format!("{:?}", expected));
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
        let outcome = reduce(&state, &EngineInput::ModelCompleted { request_id, response });
        outcome.next_state
    }

    #[test]
    fn retryable_model_failure_schedules_retry_and_retry_ready_reemits_same_request() {
        let (state, request_id) = submitted_state();
        let original_messages = state.messages.clone();

        let first_retry = reduce(&state, &model_failed_input(&request_id, retryable_failure("temporary failure")));

        assert!(first_retry.rejection.is_none());
        assert_eq!(first_retry.next_state.phase, EngineTurnPhase::WaitingForRetry);
        assert_eq!(first_retry.next_state.pending_model_request, Some(request_id.clone()));
        assert_messages_unchanged(&first_retry.next_state.messages, &original_messages);
        assert!(first_retry.terminal_failure.is_none());
        let (first_retry_id, first_delay) = schedule_retry_effect(&first_retry);
        assert_eq!(first_retry_id, &request_id);
        assert_eq!(first_delay, core::time::Duration::from_secs(FIRST_RETRY_DELAY_SECONDS));
        assert!(no_request_model_effect(&first_retry));

        let retry_ready = reduce(&first_retry.next_state, &retry_ready_input(&request_id));

        assert!(retry_ready.rejection.is_none());
        assert_eq!(retry_ready.next_state.phase, EngineTurnPhase::WaitingForModel);
        assert_eq!(retry_ready.next_state.pending_model_request, Some(request_id.clone()));
        assert_messages_unchanged(&retry_ready.next_state.messages, &original_messages);
        let retried_request = request_model_effect(&retry_ready);
        assert_eq!(retried_request.request_id, request_id);

        let second_retry = reduce(
            &retry_ready.next_state,
            &model_failed_input(&request_id, retryable_failure("temporary failure again")),
        );

        assert!(second_retry.rejection.is_none());
        let (second_retry_id, second_delay) = schedule_retry_effect(&second_retry);
        assert_eq!(second_retry_id, &request_id);
        assert_eq!(second_delay, core::time::Duration::from_secs(SECOND_RETRY_DELAY_SECONDS));
    }

    #[test]
    fn successful_model_feedback_resets_retry_counter_for_follow_up_request() {
        let (state, initial_request_id) = submitted_state();
        let first_retry = reduce(&state, &model_failed_input(&initial_request_id, retryable_failure("first failure")));
        let retry_ready = reduce(&first_retry.next_state, &retry_ready_input(&initial_request_id));
        let tool_use = reduce(&retry_ready.next_state, &EngineInput::ModelCompleted {
            request_id: initial_request_id.clone(),
            response: one_tool_use_response("call-1"),
        });
        let follow_up = reduce(&tool_use.next_state, &EngineInput::ToolCompleted {
            call_id: EngineCorrelationId("call-1".to_string()),
            result: vec![Content::Text {
                text: "tool result".to_string(),
            }],
        });
        let follow_up_request_id = request_model_effect(&follow_up).request_id.clone();

        let follow_up_retry = reduce(
            &follow_up.next_state,
            &model_failed_input(&follow_up_request_id, retryable_failure("follow-up first failure")),
        );

        assert!(follow_up_retry.rejection.is_none());
        let (scheduled_request_id, delay) = schedule_retry_effect(&follow_up_retry);
        assert_eq!(scheduled_request_id, &follow_up_request_id);
        assert_eq!(delay, core::time::Duration::from_secs(FIRST_RETRY_DELAY_SECONDS));
    }

    #[test]
    fn retry_exhaustion_terminalizes_with_latest_failure_without_message_mutation() {
        let (state, request_id) = submitted_state();
        let original_messages = state.messages.clone();
        let first_retry = reduce(&state, &model_failed_input(&request_id, retryable_failure("first failure")));
        let first_ready = reduce(&first_retry.next_state, &retry_ready_input(&request_id));
        let second_retry =
            reduce(&first_ready.next_state, &model_failed_input(&request_id, retryable_failure("second failure")));
        let second_ready = reduce(&second_retry.next_state, &retry_ready_input(&request_id));
        let terminal =
            reduce(&second_ready.next_state, &model_failed_input(&request_id, retryable_failure("third failure")));

        assert!(terminal.rejection.is_none());
        assert_eq!(terminal.next_state.phase, EngineTurnPhase::Finished);
        assert!(terminal.next_state.pending_model_request.is_none());
        assert_messages_unchanged(&terminal.next_state.messages, &original_messages);
        assert_eq!(terminal.terminal_failure, Some(retryable_failure("third failure")));
        assert!(no_request_model_effect(&terminal));
        assert!(no_retry_effect(&terminal));
        assert_terminal_failure_events(&terminal, "third failure");
    }

    #[test]
    fn non_retryable_model_failure_terminalizes_without_retry_or_message_mutation() {
        let (state, request_id) = submitted_state();
        let original_messages = state.messages.clone();

        let terminal = reduce(&state, &model_failed_input(&request_id, non_retryable_failure("bad request")));

        assert!(terminal.rejection.is_none());
        assert_eq!(terminal.next_state.phase, EngineTurnPhase::Finished);
        assert!(terminal.next_state.pending_model_request.is_none());
        assert_messages_unchanged(&terminal.next_state.messages, &original_messages);
        assert_eq!(terminal.terminal_failure, Some(non_retryable_failure("bad request")));
        assert!(no_request_model_effect(&terminal));
        assert!(no_retry_effect(&terminal));
        assert_terminal_failure_events(&terminal, "bad request");
    }

    #[test]
    fn cancel_turn_while_retry_scheduled_clears_work_and_rejects_late_feedback() {
        let (state, request_id) = submitted_state();
        let scheduled_retry = reduce(&state, &model_failed_input(&request_id, retryable_failure("temporary failure")));

        let cancelled = reduce(&scheduled_retry.next_state, &EngineInput::CancelTurn {
            reason: TURN_CANCELLED_REASON.to_string(),
        });

        assert!(cancelled.rejection.is_none());
        assert_eq!(cancelled.next_state.phase, EngineTurnPhase::Finished);
        assert!(cancelled.next_state.pending_model_request.is_none());
        assert!(no_request_model_effect(&cancelled));
        assert_eq!(emitted_events(&cancelled), vec![
            EngineEvent::BusyChanged { busy: false },
            EngineEvent::Notice {
                message: TURN_CANCELLED_REASON.to_string(),
            },
            EngineEvent::TurnFinished {
                stop_reason: StopReason::Stop,
            },
        ]);

        let retry_ready = reduce(&cancelled.next_state, &retry_ready_input(&request_id));
        assert_rejected_without_state_change(&cancelled.next_state, &retry_ready, EngineRejection::InvalidPhase);
        let model_success = reduce(&cancelled.next_state, &EngineInput::ModelCompleted {
            request_id: request_id.clone(),
            response: text_model_response("late", StopReason::Stop),
        });
        assert_rejected_without_state_change(&cancelled.next_state, &model_success, EngineRejection::InvalidPhase);
        let model_failure =
            reduce(&cancelled.next_state, &model_failed_input(&request_id, retryable_failure("late failure")));
        assert_rejected_without_state_change(&cancelled.next_state, &model_failure, EngineRejection::InvalidPhase);
    }

    #[test]
    fn failed_retry_attempts_do_not_mutate_canonical_messages() {
        let (state, request_id) = submitted_state();
        let original_messages = state.messages.clone();

        let first_retry = reduce(&state, &model_failed_input(&request_id, retryable_failure("first failure")));
        let first_ready = reduce(&first_retry.next_state, &retry_ready_input(&request_id));
        let second_retry =
            reduce(&first_ready.next_state, &model_failed_input(&request_id, retryable_failure("second failure")));
        let second_ready = reduce(&second_retry.next_state, &retry_ready_input(&request_id));
        let terminal =
            reduce(&second_ready.next_state, &model_failed_input(&request_id, retryable_failure("third failure")));

        assert_messages_unchanged(&first_retry.next_state.messages, &original_messages);
        assert_messages_unchanged(&second_retry.next_state.messages, &original_messages);
        assert_messages_unchanged(&terminal.next_state.messages, &original_messages);
    }

    #[test]
    fn model_continuation_budget_counts_requests_and_terminalizes_after_accepted_tool_feedback() {
        let mut submission = submission_with_session("session-budget");
        submission.model_request_slot_budget = TWO_MODEL_REQUEST_SLOT_BUDGET;
        let submit = reduce(&EngineState::new(), &EngineInput::SubmitUserPrompt { submission });
        let initial_request_id = request_model_effect(&submit).request_id.clone();
        let first_tool_use = reduce(&submit.next_state, &EngineInput::ModelCompleted {
            request_id: initial_request_id,
            response: one_tool_use_response("call-1"),
        });
        let follow_up = reduce(&first_tool_use.next_state, &EngineInput::ToolCompleted {
            call_id: EngineCorrelationId("call-1".to_string()),
            result: vec![Content::Text {
                text: "first tool result".to_string(),
            }],
        });
        let follow_up_request_id = request_model_effect(&follow_up).request_id.clone();
        let retry = reduce(
            &follow_up.next_state,
            &model_failed_input(&follow_up_request_id, retryable_failure("retry does not consume slots")),
        );
        let retry_ready = reduce(&retry.next_state, &retry_ready_input(&follow_up_request_id));
        let second_tool_use = reduce(&retry_ready.next_state, &EngineInput::ModelCompleted {
            request_id: follow_up_request_id,
            response: one_tool_use_response("call-2"),
        });

        let exhausted = reduce(&second_tool_use.next_state, &EngineInput::ToolCompleted {
            call_id: EngineCorrelationId("call-2".to_string()),
            result: vec![Content::Text {
                text: "second tool result".to_string(),
            }],
        });

        assert!(exhausted.rejection.is_none());
        assert_eq!(exhausted.next_state.phase, EngineTurnPhase::Finished);
        assert!(exhausted.next_state.pending_model_request.is_none());
        assert!(exhausted.next_state.pending_tool_calls.is_empty());
        assert!(exhausted.terminal_failure.is_none());
        assert!(no_request_model_effect(&exhausted));
        assert_eq!(emitted_events(&exhausted), vec![
            EngineEvent::BusyChanged { busy: false },
            EngineEvent::Notice {
                message: BUDGET_EXHAUSTED_NOTICE.to_string(),
            },
            EngineEvent::TurnFinished {
                stop_reason: StopReason::Stop,
            },
        ]);
        let last_message = exhausted.next_state.messages.last().expect("accepted tool result recorded");
        assert_eq!(last_message.role, EngineMessageRole::Tool);
    }

    #[test]
    fn zero_model_request_budget_rejects_prompt_before_provider_work() {
        let state = EngineState::new();
        let mut submission = submission_with_session("session-zero-budget");
        submission.model_request_slot_budget = ZERO_MODEL_REQUEST_SLOT_BUDGET;

        let outcome = reduce(&state, &EngineInput::SubmitUserPrompt { submission });

        assert_rejected_without_state_change(&state, &outcome, EngineRejection::InvalidBudget);
    }

    #[test]
    fn max_tokens_terminalizes_after_accepting_assistant_content() {
        let (state, request_id) = submitted_state();

        let outcome = reduce(&state, &EngineInput::ModelCompleted {
            request_id,
            response: text_model_response("partial answer", StopReason::MaxTokens),
        });

        assert!(outcome.rejection.is_none());
        assert_eq!(outcome.next_state.phase, EngineTurnPhase::Finished);
        assert!(outcome.next_state.pending_model_request.is_none());
        assert_eq!(outcome.next_state.messages.len(), TOOL_USE_MESSAGE_COUNT);
        assert!(matches!(
            outcome.next_state.messages.last(),
            Some(EngineMessage {
                role: EngineMessageRole::Assistant,
                ..
            })
        ));
        assert!(outcome.terminal_failure.is_none());
        assert!(no_request_model_effect(&outcome));
        assert!(no_retry_effect(&outcome));
        assert!(outcome.effects.iter().all(|effect| !matches!(effect, EngineEffect::ExecuteTool(_))));
        assert_eq!(emitted_events(&outcome), vec![
            EngineEvent::BusyChanged { busy: false },
            EngineEvent::TurnFinished {
                stop_reason: StopReason::MaxTokens,
            },
        ]);
    }

    #[test]
    fn retry_feedback_rejects_mismatched_request_ids_without_state_mutation() {
        let (state, request_id) = submitted_state();
        let wrong_request_id = EngineCorrelationId("wrong-request".to_string());
        let mismatched_failure =
            reduce(&state, &model_failed_input(&wrong_request_id, retryable_failure("wrong failure")));
        assert_rejected_without_state_change(&state, &mismatched_failure, EngineRejection::CorrelationMismatch);

        let scheduled_retry = reduce(&state, &model_failed_input(&request_id, retryable_failure("temporary failure")));
        let mismatched_retry_ready = reduce(&scheduled_retry.next_state, &retry_ready_input(&wrong_request_id));
        assert_rejected_without_state_change(
            &scheduled_retry.next_state,
            &mismatched_retry_ready,
            EngineRejection::CorrelationMismatch,
        );
    }

    #[test]
    fn retry_feedback_rejects_wrong_phase_duplicate_and_post_terminal_feedback() {
        let idle = EngineState::new();
        let request_id = expected_model_request_id(ENGINE_INITIAL_MODEL_REQUEST_SEQUENCE);
        let idle_retry_ready = reduce(&idle, &retry_ready_input(&request_id));
        assert_rejected_without_state_change(&idle, &idle_retry_ready, EngineRejection::InvalidPhase);

        let (state, pending_request_id) = submitted_state();
        let scheduled_retry =
            reduce(&state, &model_failed_input(&pending_request_id, retryable_failure("temporary failure")));
        let duplicate_failure = reduce(
            &scheduled_retry.next_state,
            &model_failed_input(&pending_request_id, retryable_failure("duplicate failure")),
        );
        assert_rejected_without_state_change(
            &scheduled_retry.next_state,
            &duplicate_failure,
            EngineRejection::InvalidPhase,
        );
        let model_success_during_retry_wait = reduce(&scheduled_retry.next_state, &EngineInput::ModelCompleted {
            request_id: pending_request_id.clone(),
            response: text_model_response("too early", StopReason::Stop),
        });
        assert_rejected_without_state_change(
            &scheduled_retry.next_state,
            &model_success_during_retry_wait,
            EngineRejection::InvalidPhase,
        );

        let terminal =
            reduce(&state, &model_failed_input(&pending_request_id, non_retryable_failure("terminal failure")));
        let post_terminal_retry_ready = reduce(&terminal.next_state, &retry_ready_input(&pending_request_id));
        assert_rejected_without_state_change(
            &terminal.next_state,
            &post_terminal_retry_ready,
            EngineRejection::InvalidPhase,
        );
        let post_terminal_success = reduce(&terminal.next_state, &EngineInput::ModelCompleted {
            request_id: pending_request_id.clone(),
            response: text_model_response("late success", StopReason::Stop),
        });
        assert_rejected_without_state_change(
            &terminal.next_state,
            &post_terminal_success,
            EngineRejection::InvalidPhase,
        );
        let post_terminal_failure =
            reduce(&terminal.next_state, &model_failed_input(&pending_request_id, retryable_failure("late failure")));
        assert_rejected_without_state_change(
            &terminal.next_state,
            &post_terminal_failure,
            EngineRejection::InvalidPhase,
        );
    }

    #[test]
    fn submit_user_prompt_builds_request_effect() {
        let state = EngineState::new();
        let outcome = reduce(&state, &EngineInput::SubmitUserPrompt {
            submission: submission_with_session("session-123"),
        });

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

        let outcome = reduce(&state, &EngineInput::SubmitUserPrompt {
            submission: submission_with_session("session-123"),
        });

        assert!(outcome.effects.is_empty());
        assert_eq!(outcome.rejection, Some(EngineRejection::Busy));
        assert_eq!(outcome.next_state.phase, EngineTurnPhase::WaitingForModel);
        assert_eq!(outcome.next_state.pending_model_request, Some(EngineCorrelationId("existing".to_string())));
    }

    #[test]
    fn submit_user_prompt_preserves_empty_session_id() {
        let outcome = reduce(&EngineState::new(), &EngineInput::SubmitUserPrompt {
            submission: submission_with_session(""),
        });

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

        let outcome = reduce(&state, &EngineInput::ModelCompleted { request_id, response });

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

        let outcome = reduce(&state, &EngineInput::ModelCompleted { request_id, response });

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

        let outcome = reduce(&state, &EngineInput::ModelCompleted {
            request_id: EngineCorrelationId("wrong".to_string()),
            response,
        });

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

        let outcome = reduce(&EngineState::new(), &EngineInput::ModelCompleted {
            request_id: expected_model_request_id(ENGINE_INITIAL_MODEL_REQUEST_SEQUENCE),
            response,
        });

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

        let outcome = reduce(&state, &EngineInput::ModelCompleted { request_id, response });

        assert!(outcome.effects.is_empty());
        assert_eq!(outcome.rejection, Some(EngineRejection::MissingToolCall));
        assert_eq!(outcome.next_state.phase, EngineTurnPhase::WaitingForModel);
    }

    #[test]
    fn tool_feedback_waits_for_all_pending_results_before_continuing() {
        let state = waiting_for_tools_state();
        let partial_outcome = reduce(&state, &EngineInput::ToolCompleted {
            call_id: EngineCorrelationId("call-2".to_string()),
            result: vec![Content::Text {
                text: "second result".to_string(),
            }],
        });

        assert!(partial_outcome.rejection.is_none());
        assert_eq!(partial_outcome.next_state.phase, EngineTurnPhase::WaitingForTools);
        assert!(partial_outcome.effects.is_empty());
        assert_eq!(partial_outcome.next_state.buffered_tool_results.len(), 1);

        let final_outcome = reduce(&partial_outcome.next_state, &EngineInput::ToolFailed {
            call_id: EngineCorrelationId("call-1".to_string()),
            error: "tool failed".to_string(),
            result: vec![Content::Text {
                text: "tool failed".to_string(),
            }],
        });

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
            tool_use_id, is_error, ..
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
        let outcome = reduce(&state, &EngineInput::ToolCompleted {
            call_id: EngineCorrelationId("wrong".to_string()),
            result: Vec::new(),
        });

        assert!(outcome.effects.is_empty());
        assert_eq!(outcome.rejection, Some(EngineRejection::CorrelationMismatch));
        assert_eq!(outcome.next_state.phase, EngineTurnPhase::WaitingForTools);
    }

    #[test]
    fn tool_feedback_rejects_duplicate_call_id() {
        let state = waiting_for_tools_state();
        let partial_outcome = reduce(&state, &EngineInput::ToolCompleted {
            call_id: EngineCorrelationId("call-1".to_string()),
            result: vec![Content::Text {
                text: "result".to_string(),
            }],
        });
        let duplicate_outcome = reduce(&partial_outcome.next_state, &EngineInput::ToolCompleted {
            call_id: EngineCorrelationId("call-1".to_string()),
            result: vec![Content::Text {
                text: "result".to_string(),
            }],
        });

        assert!(duplicate_outcome.effects.is_empty());
        assert_eq!(duplicate_outcome.rejection, Some(EngineRejection::CorrelationMismatch));
        assert_eq!(duplicate_outcome.next_state.phase, EngineTurnPhase::WaitingForTools);
    }

    #[test]
    fn tool_feedback_rejects_wrong_phase() {
        let outcome = reduce(&EngineState::new(), &EngineInput::ToolCompleted {
            call_id: EngineCorrelationId("call-1".to_string()),
            result: Vec::new(),
        });

        assert!(outcome.effects.is_empty());
        assert_eq!(outcome.rejection, Some(EngineRejection::InvalidPhase));
        assert_eq!(outcome.next_state.phase, EngineTurnPhase::Idle);
    }

    #[test]
    fn cancel_turn_terminalizes_pending_model_work() {
        let (state, _) = submitted_state();
        let outcome = reduce(&state, &EngineInput::CancelTurn {
            reason: "cancelled".to_string(),
        });

        assert!(outcome.rejection.is_none());
        assert_eq!(outcome.next_state.phase, EngineTurnPhase::Finished);
        assert!(outcome.next_state.pending_model_request.is_none());
        assert!(outcome.next_state.pending_tool_calls.is_empty());
        assert_eq!(outcome.effects.len(), CANCELLATION_EFFECT_COUNT);
    }

    #[test]
    fn cancel_turn_terminalizes_pending_tool_work() {
        let state = waiting_for_tools_state();
        let outcome = reduce(&state, &EngineInput::CancelTurn {
            reason: "cancelled".to_string(),
        });

        assert!(outcome.rejection.is_none());
        assert_eq!(outcome.next_state.phase, EngineTurnPhase::Finished);
        assert!(outcome.next_state.pending_model_request.is_none());
        assert!(outcome.next_state.pending_tool_calls.is_empty());
        assert_eq!(outcome.effects.len(), CANCELLATION_EFFECT_COUNT);
    }

    #[test]
    fn cancel_turn_rejects_idle_phase() {
        let outcome = reduce(&EngineState::new(), &EngineInput::CancelTurn {
            reason: "cancelled".to_string(),
        });

        assert!(outcome.effects.is_empty());
        assert_eq!(outcome.rejection, Some(EngineRejection::InvalidPhase));
        assert_eq!(outcome.next_state.phase, EngineTurnPhase::Idle);
    }

    #[test]
    fn model_failed_terminalizes_pending_request() {
        let (state, request_id) = submitted_state();
        let outcome = reduce(&state, &model_failed_input(&request_id, non_retryable_failure("provider failed")));

        assert!(outcome.rejection.is_none());
        assert_eq!(outcome.next_state.phase, EngineTurnPhase::Finished);
        assert!(outcome.next_state.pending_model_request.is_none());
        assert!(outcome.next_state.request_template.is_none());
        assert_eq!(outcome.effects.len(), CANCELLATION_EFFECT_COUNT);
    }

    #[test]
    fn submit_user_prompt_preserves_engine_native_transcript_messages() {
        let mut submission = submission_with_session("session-123");
        submission.messages.push(EngineMessage {
            role: EngineMessageRole::Tool,
            content: vec![Content::ToolResult {
                tool_use_id: "call-1".to_string(),
                content: vec![Content::Text {
                    text: "tool output".to_string(),
                }],
                is_error: Some(false),
            }],
        });

        let outcome = reduce(&EngineState::new(), &EngineInput::SubmitUserPrompt { submission });

        let model_effect = request_model_effect(&outcome);
        assert_eq!(model_effect.messages.len(), 2);
        assert_eq!(model_effect.messages[0].role, EngineMessageRole::User);
        assert_eq!(model_effect.messages[1].role, EngineMessageRole::Tool);
    }

    fn engine_state_field_names_from_source(source: &str) -> Vec<String> {
        let Some(start_index) = source.find(ENGINE_STATE_FIELD_STRUCT_START) else {
            panic!("EngineState struct must exist in source");
        };
        let after_start = &source[start_index + ENGINE_STATE_FIELD_STRUCT_START.len()..];
        let Some(end_index) = after_start.find('}') else {
            panic!("EngineState struct must have a closing brace");
        };
        after_start[..end_index]
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                let field = trimmed.strip_prefix(RUST_PUBLIC_FIELD_PREFIX)?;
                let field_name = field.split(RUST_FIELD_NAME_SEPARATOR).next()?;
                Some(field_name.trim().to_string())
            })
            .collect()
    }

    #[test]
    fn engine_state_fields_are_active() {
        let source = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs"))
            .expect("engine source must be readable");
        let source_fields = engine_state_field_names_from_source(&source);
        let inventory_fields = ENGINE_STATE_FIELD_INVENTORY
            .iter()
            .map(|(field, _justification)| (*field).to_string())
            .collect::<Vec<_>>();

        assert_eq!(source_fields, inventory_fields, "EngineState field inventory must stay exact");
        for (field, justification) in ENGINE_STATE_FIELD_INVENTORY {
            assert!(!field.is_empty(), "EngineState inventory field names must be non-empty");
            assert!(
                !justification.is_empty(),
                "EngineState inventory field {field} must explain active reducer coverage"
            );
        }
    }
}
