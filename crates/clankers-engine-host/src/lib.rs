//! Reusable async host runner for `clankers-engine` effects.
//!
//! The runner owns effect interpretation and correlation plumbing while callers
//! supply model, tool, sleep, event, cancellation, and usage adapters.

pub mod stream;

use core::time::Duration;

use clanker_message::Usage;
use clankers_engine::{
    reduce, EngineCorrelationId, EngineEffect, EngineEvent, EngineInput, EngineModelRequest, EngineModelResponse,
    EngineOutcome, EngineState, EngineTerminalFailure, EngineToolCall,
};
use clankers_tool_host::{ToolExecutor, ToolHostOutcome};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const HOST_CANCELLED_REASON: &str = "turn cancelled";
pub const MISSING_TOOL_ERROR_PREFIX: &str = "missing tool";
pub const CAPABILITY_DENIED_ERROR_PREFIX: &str = "capability denied";
pub const TOOL_CANCELLED_ERROR_PREFIX: &str = "tool cancelled";

#[derive(Debug, Clone)]
pub struct EngineRunSeed {
    pub initial_state: EngineState,
    pub first_outcome: EngineOutcome,
}

impl EngineRunSeed {
    #[must_use]
    pub fn new(initial_state: EngineState, first_outcome: EngineOutcome) -> Self {
        Self {
            initial_state,
            first_outcome,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EngineRunReport {
    pub initial_state: EngineState,
    pub final_state: EngineState,
    pub last_outcome: EngineOutcome,
    pub observed_events: Vec<EngineEvent>,
    pub usage_observations: Vec<UsageObservation>,
    pub adapter_diagnostics: Vec<AdapterDiagnostic>,
}

impl EngineRunReport {
    #[must_use]
    pub fn new(seed: &EngineRunSeed) -> Self {
        Self {
            initial_state: seed.initial_state.clone(),
            final_state: seed.first_outcome.next_state.clone(),
            last_outcome: seed.first_outcome.clone(),
            observed_events: Vec::new(),
            usage_observations: Vec::new(),
            adapter_diagnostics: Vec::new(),
        }
    }

    fn replace_reducer_outcome(&mut self, outcome: EngineOutcome) {
        self.final_state = outcome.next_state.clone();
        self.last_outcome = outcome;
    }

    fn push_diagnostic(&mut self, component: HostAdapterComponent, message: impl Into<String>) {
        self.adapter_diagnostics.push(AdapterDiagnostic {
            component,
            message: message.into(),
        });
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageObservation {
    pub kind: UsageObservationKind,
    pub usage: Usage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UsageObservationKind {
    StreamDelta,
    FinalSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterDiagnostic {
    pub component: HostAdapterComponent,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostAdapterComponent {
    Model,
    Tool,
    RetrySleeper,
    EventSink,
    Cancellation,
    UsageObserver,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum HostAdapterError {
    #[error("adapter failed: {message}")]
    Failed { message: String },
}

impl HostAdapterError {
    #[must_use]
    pub fn failed(message: impl Into<String>) -> Self {
        Self::Failed {
            message: message.into(),
        }
    }

    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::Failed { message } => message,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ModelHostOutcome {
    Completed {
        response: EngineModelResponse,
        usage: Option<Usage>,
    },
    Failed {
        failure: EngineTerminalFailure,
    },
}

pub trait ModelHost {
    fn execute_model(&mut self, request: EngineModelRequest) -> impl core::future::Future<Output = ModelHostOutcome> + Send;
}

pub trait RetrySleeper {
    fn sleep_for_retry(
        &mut self,
        request_id: EngineCorrelationId,
        delay: Duration,
    ) -> impl core::future::Future<Output = Result<(), HostAdapterError>> + Send;
}

pub trait EngineEventSink {
    fn emit_engine_event(&mut self, event: &EngineEvent) -> Result<(), HostAdapterError>;
}

pub trait CancellationSource {
    fn is_cancelled(&mut self) -> bool;

    fn cancellation_reason(&mut self) -> String {
        HOST_CANCELLED_REASON.to_string()
    }
}

pub trait UsageObserver {
    fn observe_usage(&mut self, observation: &UsageObservation) -> Result<(), HostAdapterError>;
}

pub struct HostAdapters<'a, M, T, R, E, C, U>
where
    M: ModelHost,
    T: ToolExecutor,
    R: RetrySleeper,
    E: EngineEventSink,
    C: CancellationSource,
    U: UsageObserver,
{
    pub model: &'a mut M,
    pub tools: &'a mut T,
    pub retry_sleeper: &'a mut R,
    pub event_sink: &'a mut E,
    pub cancellation: &'a mut C,
    pub usage_observer: &'a mut U,
}

pub async fn run_engine_turn<M, T, R, E, C, U>(
    seed: EngineRunSeed,
    mut hosts: HostAdapters<'_, M, T, R, E, C, U>,
) -> EngineRunReport
where
    M: ModelHost,
    T: ToolExecutor,
    R: RetrySleeper,
    E: EngineEventSink,
    C: CancellationSource,
    U: UsageObserver,
{
    let mut report = EngineRunReport::new(&seed);
    let mut state = seed.first_outcome.next_state.clone();
    let mut outcome = seed.first_outcome;

    loop {
        if outcome.rejection.is_some() || outcome.terminal_failure.is_some() || outcome.effects.is_empty() {
            report.replace_reducer_outcome(outcome);
            return report;
        }

        let effects = outcome.effects.clone();
        let mut advanced_reducer = false;
        for effect in effects {
            match effect {
                EngineEffect::EmitEvent(event) => {
                    observe_event(&mut report, hosts.event_sink, event);
                }
                EngineEffect::RequestModel(request) => {
                    let input = model_input_from_effect(&mut report, &mut hosts, request).await;
                    outcome = reduce(&state, &input);
                    state = outcome.next_state.clone();
                    advanced_reducer = true;
                    break;
                }
                EngineEffect::ScheduleRetry { request_id, delay } => {
                    let input = retry_input_from_effect(&mut report, &mut hosts, request_id, delay).await;
                    outcome = reduce(&state, &input);
                    state = outcome.next_state.clone();
                    advanced_reducer = true;
                    break;
                }
                EngineEffect::ExecuteTool(call) => {
                    let input = tool_input_from_effect(&mut hosts, call).await;
                    outcome = reduce(&state, &input);
                    state = outcome.next_state.clone();
                    advanced_reducer = true;
                    if outcome.rejection.is_some() || outcome.terminal_failure.is_some() {
                        break;
                    }
                }
            }
        }

        report.replace_reducer_outcome(outcome.clone());
        if !advanced_reducer {
            return report;
        }
    }
}

async fn model_input_from_effect<M, T, R, E, C, U>(
    report: &mut EngineRunReport,
    hosts: &mut HostAdapters<'_, M, T, R, E, C, U>,
    request: EngineModelRequest,
) -> EngineInput
where
    M: ModelHost,
    T: ToolExecutor,
    R: RetrySleeper,
    E: EngineEventSink,
    C: CancellationSource,
    U: UsageObserver,
{
    if hosts.cancellation.is_cancelled() {
        return cancel_input(hosts.cancellation);
    }
    let request_id = request.request_id.clone();
    match hosts.model.execute_model(request).await {
        ModelHostOutcome::Completed { response, usage } => {
            if let Some(usage) = usage {
                observe_usage(report, hosts.usage_observer, UsageObservationKind::FinalSummary, usage);
            }
            EngineInput::ModelCompleted { request_id, response }
        }
        ModelHostOutcome::Failed { failure } => EngineInput::ModelFailed { request_id, failure },
    }
}

async fn retry_input_from_effect<M, T, R, E, C, U>(
    report: &mut EngineRunReport,
    hosts: &mut HostAdapters<'_, M, T, R, E, C, U>,
    request_id: EngineCorrelationId,
    delay: Duration,
) -> EngineInput
where
    M: ModelHost,
    T: ToolExecutor,
    R: RetrySleeper,
    E: EngineEventSink,
    C: CancellationSource,
    U: UsageObserver,
{
    if hosts.cancellation.is_cancelled() {
        return cancel_input(hosts.cancellation);
    }
    if let Err(error) = hosts.retry_sleeper.sleep_for_retry(request_id.clone(), delay).await {
        report.push_diagnostic(HostAdapterComponent::RetrySleeper, error.message());
    }
    if hosts.cancellation.is_cancelled() {
        return cancel_input(hosts.cancellation);
    }
    EngineInput::RetryReady { request_id }
}

async fn tool_input_from_effect<M, T, R, E, C, U>(
    hosts: &mut HostAdapters<'_, M, T, R, E, C, U>,
    call: EngineToolCall,
) -> EngineInput
where
    M: ModelHost,
    T: ToolExecutor,
    R: RetrySleeper,
    E: EngineEventSink,
    C: CancellationSource,
    U: UsageObserver,
{
    if hosts.cancellation.is_cancelled() {
        return cancel_input(hosts.cancellation);
    }
    let call_id = call.call_id.clone();
    tool_outcome_to_input(call_id, hosts.tools.execute_tool(call).await, hosts.cancellation)
}

fn tool_outcome_to_input<C: CancellationSource>(
    call_id: EngineCorrelationId,
    outcome: ToolHostOutcome,
    cancellation: &mut C,
) -> EngineInput {
    if cancellation.is_cancelled() || matches!(outcome, ToolHostOutcome::Cancelled { .. }) {
        return cancel_input(cancellation);
    }
    match outcome {
        ToolHostOutcome::Succeeded { content, .. } | ToolHostOutcome::Truncated { content, .. } => {
            EngineInput::ToolCompleted { call_id, result: content }
        }
        ToolHostOutcome::ToolError { content, message, .. } => EngineInput::ToolFailed {
            call_id,
            error: message,
            result: content,
        },
        ToolHostOutcome::MissingTool { name } => tool_failed_with_message(
            call_id,
            format_tool_error(MISSING_TOOL_ERROR_PREFIX, &name),
        ),
        ToolHostOutcome::CapabilityDenied { name, reason } => tool_failed_with_message(
            call_id,
            format!("{CAPABILITY_DENIED_ERROR_PREFIX}: {name}: {reason}"),
        ),
        ToolHostOutcome::Cancelled { name } => tool_failed_with_message(
            call_id,
            format_tool_error(TOOL_CANCELLED_ERROR_PREFIX, &name),
        ),
    }
}

fn tool_failed_with_message(call_id: EngineCorrelationId, message: String) -> EngineInput {
    EngineInput::ToolFailed {
        call_id,
        error: message,
        result: Vec::new(),
    }
}

fn cancel_input<C: CancellationSource>(cancellation: &mut C) -> EngineInput {
    EngineInput::CancelTurn {
        reason: cancellation.cancellation_reason(),
    }
}

fn observe_event<E: EngineEventSink>(report: &mut EngineRunReport, sink: &mut E, event: EngineEvent) {
    report.observed_events.push(event.clone());
    if let Err(error) = sink.emit_engine_event(&event) {
        report.push_diagnostic(HostAdapterComponent::EventSink, error.message());
    }
}

fn observe_usage<U: UsageObserver>(
    report: &mut EngineRunReport,
    observer: &mut U,
    kind: UsageObservationKind,
    usage: Usage,
) {
    let observation = UsageObservation { kind, usage };
    if let Err(error) = observer.observe_usage(&observation) {
        report.push_diagnostic(HostAdapterComponent::UsageObserver, error.message());
    }
    report.usage_observations.push(observation);
}

fn format_tool_error(prefix: &str, name: &str) -> String {
    format!("{prefix}: {name}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use clanker_message::Content;
    use clanker_message::StopReason;
    use clankers_engine::{EngineInput, EnginePromptSubmission, EngineRejection};
    use serde_json::json;

    const TEST_MODEL: &str = "test-model";
    const TEST_PROMPT: &str = "system";
    const TEST_SESSION: &str = "session";
    const TEST_TOOL: &str = "tool";
    const TEST_USAGE_INPUT: usize = 3;
    const TEST_USAGE_OUTPUT: usize = 5;

    fn block_on<F: core::future::Future>(future: F) -> F::Output {
        use std::sync::Arc;
        use std::task::{Context, Poll, Wake, Waker};

        struct NoopWaker;

        impl Wake for NoopWaker {
            fn wake(self: Arc<Self>) {}
        }

        let waker: Waker = Waker::from(Arc::new(NoopWaker));
        let mut context = Context::from_waker(&waker);
        let mut future = Box::pin(future);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(output) => return output,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    #[derive(Default)]
    struct FakeModel {
        outcomes: Vec<ModelHostOutcome>,
    }

    impl ModelHost for FakeModel {
        async fn execute_model(&mut self, _request: EngineModelRequest) -> ModelHostOutcome {
            assert!(!self.outcomes.is_empty(), "fake model outcome must exist");
            self.outcomes.remove(0)
        }
    }

    #[derive(Default)]
    struct FakeTools {
        outcomes: Vec<ToolHostOutcome>,
    }

    impl ToolExecutor for FakeTools {
        async fn execute_tool(&mut self, _call: EngineToolCall) -> ToolHostOutcome {
            assert!(!self.outcomes.is_empty(), "fake tool outcome must exist");
            self.outcomes.remove(0)
        }
    }

    #[derive(Default)]
    struct FakeSleeper {
        slept: Vec<(EngineCorrelationId, Duration)>,
    }

    impl RetrySleeper for FakeSleeper {
        async fn sleep_for_retry(
            &mut self,
            request_id: EngineCorrelationId,
            delay: Duration,
        ) -> Result<(), HostAdapterError> {
            self.slept.push((request_id, delay));
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeEvents {
        events: Vec<EngineEvent>,
        fail: bool,
    }

    impl EngineEventSink for FakeEvents {
        fn emit_engine_event(&mut self, event: &EngineEvent) -> Result<(), HostAdapterError> {
            self.events.push(event.clone());
            if self.fail {
                return Err(HostAdapterError::failed("event sink failed"));
            }
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeCancel {
        cancelled: bool,
    }

    impl CancellationSource for FakeCancel {
        fn is_cancelled(&mut self) -> bool {
            self.cancelled
        }
    }

    #[derive(Default)]
    struct FakeUsage {
        observations: Vec<UsageObservation>,
    }

    impl UsageObserver for FakeUsage {
        fn observe_usage(&mut self, observation: &UsageObservation) -> Result<(), HostAdapterError> {
            self.observations.push(observation.clone());
            Ok(())
        }
    }

    fn seed() -> EngineRunSeed {
        let submission = EnginePromptSubmission {
            messages: vec![clankers_engine::EngineMessage {
                role: clankers_engine::EngineMessageRole::User,
                content: vec![Content::Text { text: "hello".to_string() }],
            }],
            model: TEST_MODEL.to_string(),
            system_prompt: TEST_PROMPT.to_string(),
            max_tokens: None,
            temperature: None,
            thinking: None,
            tools: Vec::new(),
            no_cache: false,
            cache_ttl: None,
            session_id: TEST_SESSION.to_string(),
            model_request_slot_budget: 1,
        };
        let state = EngineState::new();
        let outcome = clankers_engine::reduce(&state, &EngineInput::submit_user_prompt(submission));
        EngineRunSeed::new(state, outcome)
    }

    async fn run_with<M, T>(model: &mut M, tools: &mut T, events: &mut FakeEvents, cancel: &mut FakeCancel) -> EngineRunReport
    where
        M: ModelHost,
        T: ToolExecutor,
    {
        let mut sleeper = FakeSleeper::default();
        let mut usage = FakeUsage::default();
        run_engine_turn(
            seed(),
            HostAdapters {
                model,
                tools,
                retry_sleeper: &mut sleeper,
                event_sink: events,
                cancellation: cancel,
                usage_observer: &mut usage,
            },
        )
        .await
    }

    #[test]
    fn runner_completes_model_success_and_records_usage() {
        let mut model = FakeModel {
            outcomes: vec![ModelHostOutcome::Completed {
                response: EngineModelResponse {
                    output: vec![Content::Text { text: "hi".to_string() }],
                    stop_reason: StopReason::Stop,
                },
                usage: Some(Usage {
                    input_tokens: TEST_USAGE_INPUT,
                    output_tokens: TEST_USAGE_OUTPUT,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                }),
            }],
        };
        let mut tools = FakeTools::default();
        let mut events = FakeEvents::default();
        let mut cancel = FakeCancel::default();
        let report = block_on(run_with(&mut model, &mut tools, &mut events, &mut cancel));
        assert!(report.last_outcome.rejection.is_none());
        assert!(report.last_outcome.terminal_failure.is_none());
        assert_eq!(report.usage_observations.len(), 1);
        assert!(report
            .observed_events
            .iter()
            .any(|event| matches!(event, EngineEvent::TurnFinished { .. })));
    }

    #[test]
    fn event_sink_failures_become_diagnostics_without_reducer_failure() {
        let mut model = FakeModel {
            outcomes: vec![ModelHostOutcome::Completed {
                response: EngineModelResponse {
                    output: vec![Content::Text { text: "hi".to_string() }],
                    stop_reason: StopReason::Stop,
                },
                usage: None,
            }],
        };
        let mut tools = FakeTools::default();
        let mut events = FakeEvents { events: Vec::new(), fail: true };
        let mut cancel = FakeCancel::default();
        let report = block_on(run_with(&mut model, &mut tools, &mut events, &mut cancel));
        assert!(report.last_outcome.rejection.is_none());
        assert!(!report.adapter_diagnostics.is_empty());
        assert!(report
            .adapter_diagnostics
            .iter()
            .all(|diagnostic| diagnostic.component == HostAdapterComponent::EventSink));
    }

    #[test]
    fn tool_missing_maps_to_engine_feedback() {
        let mut model = FakeModel {
            outcomes: vec![ModelHostOutcome::Completed {
                response: EngineModelResponse {
                    output: vec![Content::ToolUse {
                        id: "call-1".to_string(),
                        name: TEST_TOOL.to_string(),
                        input: json!({}),
                    }],
                    stop_reason: StopReason::ToolUse,
                },
                usage: None,
            }],
        };
        let mut tools = FakeTools {
            outcomes: vec![ToolHostOutcome::MissingTool { name: TEST_TOOL.to_string() }],
        };
        let mut events = FakeEvents::default();
        let mut cancel = FakeCancel::default();
        let report = block_on(run_with(&mut model, &mut tools, &mut events, &mut cancel));
        assert!(report.last_outcome.rejection.is_none());
        assert!(report.last_outcome.terminal_failure.is_none());
        assert_eq!(model.outcomes.len(), 0);
    }

    #[test]
    fn cancellation_before_model_maps_to_cancel_turn() {
        let mut model = FakeModel::default();
        let mut tools = FakeTools::default();
        let mut events = FakeEvents::default();
        let mut cancel = FakeCancel { cancelled: true };
        let report = block_on(run_with(&mut model, &mut tools, &mut events, &mut cancel));
        assert!(report.last_outcome.rejection.is_none());
        assert!(report
            .observed_events
            .iter()
            .any(|event| matches!(event, EngineEvent::TurnFinished { stop_reason } if *stop_reason == StopReason::Stop)));
    }

    #[test]
    fn reducer_rejection_is_reported_without_local_terminalization() {
        let bad_seed = EngineRunSeed::new(
            EngineState::new(),
            EngineOutcome {
                next_state: EngineState::new(),
                effects: Vec::new(),
                rejection: Some(EngineRejection::InvalidPhase),
                terminal_failure: None,
            },
        );
        let mut model = FakeModel::default();
        let mut tools = FakeTools::default();
        let mut sleeper = FakeSleeper::default();
        let mut events = FakeEvents::default();
        let mut cancel = FakeCancel::default();
        let mut usage = FakeUsage::default();
        let report = block_on(run_engine_turn(
            bad_seed,
            HostAdapters {
                model: &mut model,
                tools: &mut tools,
                retry_sleeper: &mut sleeper,
                event_sink: &mut events,
                cancellation: &mut cancel,
                usage_observer: &mut usage,
            },
        ));
        assert_eq!(report.last_outcome.rejection, Some(EngineRejection::InvalidPhase));
        assert!(report.observed_events.is_empty());
    }
}
