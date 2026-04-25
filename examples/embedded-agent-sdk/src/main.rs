use std::collections::VecDeque;
use std::future::Future;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use std::task::Wake;
use std::task::Waker;
use std::time::Duration;

use clanker_message::Content;
use clanker_message::StopReason;
use clanker_message::Usage;
use clankers_engine::EngineCorrelationId;
use clankers_engine::EngineInput;
use clankers_engine::EngineMessage;
use clankers_engine::EngineMessageRole;
use clankers_engine::EngineModelRequest;
use clankers_engine::EngineModelResponse;
use clankers_engine::EnginePromptSubmission;
use clankers_engine::EngineState;
use clankers_engine::EngineTerminalFailure;
use clankers_engine::EngineToolCall;
use clankers_engine::reduce;
use clankers_engine_host::AdapterDiagnostic;
use clankers_engine_host::CancellationSource;
use clankers_engine_host::EngineEventSink;
use clankers_engine_host::EngineRunReport;
use clankers_engine_host::EngineRunSeed;
use clankers_engine_host::HostAdapterComponent;
use clankers_engine_host::HostAdapterError;
use clankers_engine_host::HostAdapters;
use clankers_engine_host::ModelHost;
use clankers_engine_host::ModelHostOutcome;
use clankers_engine_host::RetrySleeper;
use clankers_engine_host::UsageObservation;
use clankers_engine_host::UsageObserver;
use clankers_engine_host::run_engine_turn;
use clankers_tool_host::ToolExecutor;
use clankers_tool_host::ToolHostOutcome;
use serde_json::json;

const EXAMPLE_MODEL: &str = "embedded-fake-model";
const EXAMPLE_SYSTEM_PROMPT: &str = "You are an embedded SDK example.";
const EXAMPLE_SESSION_ID: &str = "embedded-session";
const USER_TEXT: &str = "say hello";
const TOOL_NAME: &str = "lookup";
const SUCCESS_TEXT: &str = "hello from embedded model";
const TOOL_RESULT_TEXT: &str = "tool result";
const RECOVERED_TEXT: &str = "recovered";
const TERMINAL_FAILURE_TEXT: &str = "terminal failure";
const RETRYABLE_FAILURE_TEXT: &str = "retryable failure";
const EVENT_SINK_FAILURE_TEXT: &str = "event sink failed";
const RETRY_SLEEPER_FAILURE_TEXT: &str = "retry sleeper failed";
const USAGE_FAILURE_TEXT: &str = "usage observer failed";
const TRANSCRIPT_ERROR_TEXT: &str = "system notes are shell-owned in this example";
const CANCELLED_TEXT: &str = "host cancelled before work completed";
const DEFAULT_MODEL_REQUEST_BUDGET: u32 = 1;
const RETRY_MODEL_REQUEST_BUDGET: u32 = 2;
const SAMPLE_INPUT_TOKENS: usize = 7;
const SAMPLE_OUTPUT_TOKENS: usize = 11;
const RETRYABLE_STATUS: u16 = 429;

#[derive(Debug, Clone)]
enum HostTranscriptMessage {
    UserText(String),
    AssistantText(String),
    ToolText(String),
    SystemNote(String),
}

#[derive(Default)]
struct FakeModel {
    outcomes: VecDeque<ModelHostOutcome>,
    requests: Vec<EngineModelRequest>,
}

impl FakeModel {
    fn new(outcomes: Vec<ModelHostOutcome>) -> Self {
        Self {
            outcomes: VecDeque::from(outcomes),
            requests: Vec::new(),
        }
    }
}

impl ModelHost for FakeModel {
    async fn execute_model(&mut self, request: EngineModelRequest) -> ModelHostOutcome {
        self.requests.push(request);
        self.outcomes.pop_front().unwrap_or_else(|| ModelHostOutcome::Failed {
            failure: EngineTerminalFailure {
                message: "fake model has no queued outcome".to_string(),
                status: None,
                retryable: false,
            },
        })
    }
}

#[derive(Default)]
struct FakeTools {
    outcomes: VecDeque<ToolHostOutcome>,
    calls: Vec<EngineToolCall>,
}

impl FakeTools {
    fn new(outcomes: Vec<ToolHostOutcome>) -> Self {
        Self {
            outcomes: VecDeque::from(outcomes),
            calls: Vec::new(),
        }
    }
}

impl ToolExecutor for FakeTools {
    async fn execute_tool(&mut self, call: EngineToolCall) -> ToolHostOutcome {
        self.calls.push(call.clone());
        self.outcomes.pop_front().unwrap_or_else(|| ToolHostOutcome::MissingTool { name: call.tool_name })
    }
}

#[derive(Default)]
struct FakeRetrySleeper {
    fail: bool,
    sleeps: Vec<(EngineCorrelationId, Duration)>,
}

impl RetrySleeper for FakeRetrySleeper {
    async fn sleep_for_retry(
        &mut self,
        request_id: EngineCorrelationId,
        delay: Duration,
    ) -> Result<(), HostAdapterError> {
        self.sleeps.push((request_id, delay));
        if self.fail {
            return Err(HostAdapterError::failed(RETRY_SLEEPER_FAILURE_TEXT));
        }
        Ok(())
    }
}

#[derive(Default)]
struct FakeEvents {
    fail: bool,
    events: Vec<clankers_engine::EngineEvent>,
}

impl EngineEventSink for FakeEvents {
    fn emit_engine_event(&mut self, event: &clankers_engine::EngineEvent) -> Result<(), HostAdapterError> {
        self.events.push(event.clone());
        if self.fail {
            return Err(HostAdapterError::failed(EVENT_SINK_FAILURE_TEXT));
        }
        Ok(())
    }
}

#[derive(Default)]
struct FakeCancellation {
    cancelled: bool,
}

impl CancellationSource for FakeCancellation {
    fn is_cancelled(&mut self) -> bool {
        self.cancelled
    }

    fn cancellation_reason(&mut self) -> String {
        CANCELLED_TEXT.to_string()
    }
}

#[derive(Default)]
struct FakeUsage {
    fail: bool,
    observations: Vec<UsageObservation>,
}

impl UsageObserver for FakeUsage {
    fn observe_usage(&mut self, observation: &UsageObservation) -> Result<(), HostAdapterError> {
        self.observations.push(observation.clone());
        if self.fail {
            return Err(HostAdapterError::failed(USAGE_FAILURE_TEXT));
        }
        Ok(())
    }
}

struct ScenarioAdapters {
    model: FakeModel,
    tools: FakeTools,
    retry: FakeRetrySleeper,
    events: FakeEvents,
    cancellation: FakeCancellation,
    usage: FakeUsage,
}

impl ScenarioAdapters {
    fn new(model: FakeModel, tools: FakeTools) -> Self {
        Self {
            model,
            tools,
            retry: FakeRetrySleeper::default(),
            events: FakeEvents::default(),
            cancellation: FakeCancellation::default(),
            usage: FakeUsage::default(),
        }
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
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

fn host_transcript_to_engine(messages: &[HostTranscriptMessage]) -> Result<Vec<EngineMessage>, String> {
    let mut engine_messages = Vec::with_capacity(messages.len());
    for message in messages {
        match message {
            HostTranscriptMessage::UserText(text) => {
                engine_messages.push(engine_message(EngineMessageRole::User, text))
            }
            HostTranscriptMessage::AssistantText(text) => {
                engine_messages.push(engine_message(EngineMessageRole::Assistant, text))
            }
            HostTranscriptMessage::ToolText(text) => {
                engine_messages.push(engine_message(EngineMessageRole::Tool, text))
            }
            HostTranscriptMessage::SystemNote(note) => {
                assert!(!note.is_empty(), "system note must explain why conversion failed");
                return Err(TRANSCRIPT_ERROR_TEXT.to_string());
            }
        }
    }
    Ok(engine_messages)
}

fn engine_message(role: EngineMessageRole, text: &str) -> EngineMessage {
    EngineMessage {
        role,
        content: vec![Content::Text { text: text.to_string() }],
    }
}

fn seed(messages: Vec<EngineMessage>, model_request_slot_budget: u32) -> EngineRunSeed {
    let submission = EnginePromptSubmission {
        messages,
        model: EXAMPLE_MODEL.to_string(),
        system_prompt: EXAMPLE_SYSTEM_PROMPT.to_string(),
        max_tokens: None,
        temperature: None,
        thinking: None,
        tools: Vec::new(),
        no_cache: true,
        cache_ttl: None,
        session_id: EXAMPLE_SESSION_ID.to_string(),
        model_request_slot_budget,
    };
    let state = EngineState::new();
    let outcome = reduce(&state, &EngineInput::submit_user_prompt(submission));
    EngineRunSeed::new(state, outcome)
}

fn sample_usage() -> Usage {
    Usage {
        input_tokens: SAMPLE_INPUT_TOKENS,
        output_tokens: SAMPLE_OUTPUT_TOKENS,
        cache_creation_input_tokens: 0,
        cache_read_input_tokens: 0,
    }
}

fn completed_text(text: &str) -> ModelHostOutcome {
    ModelHostOutcome::Completed {
        response: EngineModelResponse {
            output: vec![Content::Text { text: text.to_string() }],
            stop_reason: StopReason::Stop,
        },
        usage: Some(sample_usage()),
    }
}

fn tool_request() -> ModelHostOutcome {
    ModelHostOutcome::Completed {
        response: EngineModelResponse {
            output: vec![Content::ToolUse {
                id: "tool-call-1".to_string(),
                name: TOOL_NAME.to_string(),
                input: json!({ "query": "embedded" }),
            }],
            stop_reason: StopReason::ToolUse,
        },
        usage: None,
    }
}

fn retryable_failure() -> ModelHostOutcome {
    ModelHostOutcome::Failed {
        failure: EngineTerminalFailure {
            message: RETRYABLE_FAILURE_TEXT.to_string(),
            status: Some(RETRYABLE_STATUS),
            retryable: true,
        },
    }
}

fn terminal_failure() -> ModelHostOutcome {
    ModelHostOutcome::Failed {
        failure: EngineTerminalFailure {
            message: TERMINAL_FAILURE_TEXT.to_string(),
            status: None,
            retryable: false,
        },
    }
}

fn successful_tool() -> ToolHostOutcome {
    ToolHostOutcome::Succeeded {
        content: vec![Content::Text {
            text: TOOL_RESULT_TEXT.to_string(),
        }],
        details: json!({ "source": "fake" }),
    }
}

fn missing_tool() -> ToolHostOutcome {
    ToolHostOutcome::MissingTool {
        name: TOOL_NAME.to_string(),
    }
}

fn capability_denied_tool() -> ToolHostOutcome {
    ToolHostOutcome::CapabilityDenied {
        name: TOOL_NAME.to_string(),
        reason: "fake policy denied".to_string(),
    }
}

async fn run_with(seed: EngineRunSeed, adapters: &mut ScenarioAdapters) -> EngineRunReport {
    run_engine_turn(seed, HostAdapters {
        model: &mut adapters.model,
        tools: &mut adapters.tools,
        retry_sleeper: &mut adapters.retry,
        event_sink: &mut adapters.events,
        cancellation: &mut adapters.cancellation,
        usage_observer: &mut adapters.usage,
    })
    .await
}

fn initial_messages() -> Vec<EngineMessage> {
    host_transcript_to_engine(&[HostTranscriptMessage::UserText(USER_TEXT.to_string())])
        .expect("host transcript conversion should accept user text")
}

fn assert_success(report: &EngineRunReport) {
    assert!(report.last_outcome.rejection.is_none(), "engine rejected accepted prompt");
    assert!(report.last_outcome.terminal_failure.is_none(), "engine terminalized unexpectedly");
}

fn has_diagnostic(report: &EngineRunReport, component: HostAdapterComponent) -> bool {
    report
        .adapter_diagnostics
        .iter()
        .any(|diagnostic: &AdapterDiagnostic| diagnostic.component == component)
}

fn run_text_success() {
    let mut adapters = ScenarioAdapters::new(FakeModel::new(vec![completed_text(SUCCESS_TEXT)]), FakeTools::default());
    let report = block_on(run_with(seed(initial_messages(), DEFAULT_MODEL_REQUEST_BUDGET), &mut adapters));
    assert_success(&report);
    assert_eq!(adapters.model.requests.len(), 1);
    assert_eq!(adapters.usage.observations.len(), 1);
    assert!(!adapters.events.events.is_empty(), "event adapter should see engine events");
}

fn run_tool_success() {
    let mut adapters = ScenarioAdapters::new(
        FakeModel::new(vec![tool_request(), completed_text(SUCCESS_TEXT)]),
        FakeTools::new(vec![successful_tool()]),
    );
    let report = block_on(run_with(seed(initial_messages(), RETRY_MODEL_REQUEST_BUDGET), &mut adapters));
    assert_success(&report);
    assert_eq!(adapters.tools.calls.len(), 1);
    assert_eq!(adapters.model.requests.len(), RETRY_MODEL_REQUEST_BUDGET as usize);
}

fn run_retry_success() {
    let mut adapters = ScenarioAdapters::new(
        FakeModel::new(vec![retryable_failure(), completed_text(RECOVERED_TEXT)]),
        FakeTools::default(),
    );
    let report = block_on(run_with(seed(initial_messages(), RETRY_MODEL_REQUEST_BUDGET), &mut adapters));
    assert_success(&report);
    assert_eq!(adapters.retry.sleeps.len(), 1);
    assert_eq!(adapters.model.requests.len(), RETRY_MODEL_REQUEST_BUDGET as usize);
}

fn run_model_failure_negative() {
    let mut adapters = ScenarioAdapters::new(FakeModel::new(vec![terminal_failure()]), FakeTools::default());
    let report = block_on(run_with(seed(initial_messages(), DEFAULT_MODEL_REQUEST_BUDGET), &mut adapters));
    assert!(report.last_outcome.terminal_failure.is_some());
}

fn run_tool_negative(outcome: ToolHostOutcome) {
    let mut adapters = ScenarioAdapters::new(FakeModel::new(vec![tool_request()]), FakeTools::new(vec![outcome]));
    let report = block_on(run_with(seed(initial_messages(), DEFAULT_MODEL_REQUEST_BUDGET), &mut adapters));
    assert_success(&report);
    assert_eq!(adapters.tools.calls.len(), 1);
}

fn run_retry_sleeper_negative() {
    let mut adapters = ScenarioAdapters::new(
        FakeModel::new(vec![retryable_failure(), completed_text(RECOVERED_TEXT)]),
        FakeTools::default(),
    );
    adapters.retry.fail = true;
    let report = block_on(run_with(seed(initial_messages(), RETRY_MODEL_REQUEST_BUDGET), &mut adapters));
    assert_success(&report);
    assert!(has_diagnostic(&report, HostAdapterComponent::RetrySleeper));
}

fn run_event_sink_negative() {
    let mut adapters = ScenarioAdapters::new(FakeModel::new(vec![completed_text(SUCCESS_TEXT)]), FakeTools::default());
    adapters.events.fail = true;
    let report = block_on(run_with(seed(initial_messages(), DEFAULT_MODEL_REQUEST_BUDGET), &mut adapters));
    assert_success(&report);
    assert!(has_diagnostic(&report, HostAdapterComponent::EventSink));
}

fn run_cancellation_negative() {
    let mut adapters = ScenarioAdapters::new(FakeModel::new(vec![completed_text(SUCCESS_TEXT)]), FakeTools::default());
    adapters.cancellation.cancelled = true;
    let report = block_on(run_with(seed(initial_messages(), DEFAULT_MODEL_REQUEST_BUDGET), &mut adapters));
    assert!(report.final_state.messages.len() <= 1);
    assert!(adapters.model.requests.is_empty());
}

fn run_usage_negative() {
    let mut adapters = ScenarioAdapters::new(FakeModel::new(vec![completed_text(SUCCESS_TEXT)]), FakeTools::default());
    adapters.usage.fail = true;
    let report = block_on(run_with(seed(initial_messages(), DEFAULT_MODEL_REQUEST_BUDGET), &mut adapters));
    assert_success(&report);
    assert!(has_diagnostic(&report, HostAdapterComponent::UsageObserver));
}

fn run_transcript_conversion_positive_and_negative() {
    let accepted = host_transcript_to_engine(&[
        HostTranscriptMessage::UserText(USER_TEXT.to_string()),
        HostTranscriptMessage::AssistantText(SUCCESS_TEXT.to_string()),
        HostTranscriptMessage::ToolText(TOOL_RESULT_TEXT.to_string()),
    ]);
    assert!(accepted.is_ok());

    let rejected = host_transcript_to_engine(&[HostTranscriptMessage::SystemNote(
        "host-owned prompt assembly".to_string(),
    )]);
    assert!(matches!(rejected, Err(error) if error == TRANSCRIPT_ERROR_TEXT));
}

fn run_scenarios() {
    run_transcript_conversion_positive_and_negative();
    run_text_success();
    run_tool_success();
    run_retry_success();
    run_model_failure_negative();
    run_tool_negative(missing_tool());
    run_tool_negative(capability_denied_tool());
    run_retry_sleeper_negative();
    run_event_sink_negative();
    run_cancellation_negative();
    run_usage_negative();
}

fn main() {
    run_scenarios();
    println!("embedded-agent-sdk example passed");
}
