use std::collections::VecDeque;
use std::future::Future;
use std::sync::Arc;
use std::task::Context;
use std::task::Poll;
use std::task::Wake;
use std::task::Waker;

use clanker_message::Content;
use clanker_message::StopReason;
use clanker_message::Usage;
use clankers_adapters::AtomicCancellationSource;
use clankers_adapters::CollectingUsageObserver;
use clankers_adapters::MemoryEventSink;
use clankers_adapters::NoopRetrySleeper;
use clankers_adapters::ScriptedToolExecutor;
use clankers_engine::EngineInput;
use clankers_engine::EngineMessage;
use clankers_engine::EngineMessageRole;
use clankers_engine::EngineModelRequest;
use clankers_engine::EngineModelResponse;
use clankers_engine::EnginePromptSubmission;
use clankers_engine::EngineState;
use clankers_engine::EngineTerminalFailure;
use clankers_engine::reduce;
use clankers_engine_host::EngineRunSeed;
use clankers_engine_host::HostAdapters;
use clankers_engine_host::ModelHost;
use clankers_engine_host::ModelHostOutcome;
use clankers_engine_host::run_engine_turn;

const MODEL: &str = "product-owned-model";
const SESSION_ID: &str = "embedded-provider-session";

fn main() {
    completed_provider_path();
    retryable_provider_path();
    terminal_provider_failure_path();
    println!("embedded-provider-adapter passed");
}

fn completed_provider_path() {
    let mut adapter = ProductProviderAdapter::new([ProductProviderResponse::Completed {
        text: "provider adapter completed".to_string(),
        usage: Usage {
            input_tokens: 4,
            output_tokens: 6,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        },
    }]);
    let mut retry = NoopRetrySleeper::default();
    let mut events = MemoryEventSink::default();
    let mut cancellation = AtomicCancellationSource::default();
    let mut usage = CollectingUsageObserver::default();
    let mut tools = ScriptedToolExecutor::default();

    let report = block_on(run_engine_turn(seed("answer directly"), HostAdapters {
        model: &mut adapter,
        tools: &mut tools,
        retry_sleeper: &mut retry,
        event_sink: &mut events,
        cancellation: &mut cancellation,
        usage_observer: &mut usage,
    }));

    assert!(report.last_outcome.terminal_failure.is_none());
    assert_eq!(adapter.requests().len(), 1);
    assert_eq!(adapter.requests()[0].model, MODEL);
    assert_eq!(adapter.requests()[0].session_id, SESSION_ID);
    assert!(adapter.requests()[0].prompt_text.contains("answer directly"));
    assert_eq!(usage.observations().len(), 1);
    assert!(!events.events().is_empty());
}

fn retryable_provider_path() {
    let mut adapter = ProductProviderAdapter::new([
        ProductProviderResponse::RetryableFailure {
            status: 503,
            message: "provider warming".to_string(),
        },
        ProductProviderResponse::Completed {
            text: "retry recovered".to_string(),
            usage: Usage {
                input_tokens: 5,
                output_tokens: 7,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
        },
    ]);
    let mut retry = NoopRetrySleeper::default();
    let mut events = MemoryEventSink::default();
    let mut cancellation = AtomicCancellationSource::default();
    let mut usage = CollectingUsageObserver::default();
    let mut tools = ScriptedToolExecutor::default();

    let report = block_on(run_engine_turn(seed("retry once"), HostAdapters {
        model: &mut adapter,
        tools: &mut tools,
        retry_sleeper: &mut retry,
        event_sink: &mut events,
        cancellation: &mut cancellation,
        usage_observer: &mut usage,
    }));

    assert!(report.last_outcome.terminal_failure.is_none());
    assert_eq!(adapter.requests().len(), 2);
    assert_eq!(retry.sleeps().len(), 1);
    assert_eq!(usage.observations().len(), 1);
}

fn terminal_provider_failure_path() {
    let mut adapter = ProductProviderAdapter::new([ProductProviderResponse::TerminalFailure {
        status: 400,
        message: "bad product request".to_string(),
    }]);
    let mut retry = NoopRetrySleeper::default();
    let mut events = MemoryEventSink::default();
    let mut cancellation = AtomicCancellationSource::default();
    let mut usage = CollectingUsageObserver::default();
    let mut tools = ScriptedToolExecutor::default();

    let report = block_on(run_engine_turn(seed("fail terminally"), HostAdapters {
        model: &mut adapter,
        tools: &mut tools,
        retry_sleeper: &mut retry,
        event_sink: &mut events,
        cancellation: &mut cancellation,
        usage_observer: &mut usage,
    }));

    assert!(report.last_outcome.terminal_failure.is_some());
    assert_eq!(adapter.requests().len(), 1);
    assert!(retry.sleeps().is_empty());
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProductProviderRequest {
    model: String,
    session_id: String,
    system_prompt: String,
    prompt_text: String,
}

#[derive(Debug, Clone)]
enum ProductProviderResponse {
    Completed { text: String, usage: Usage },
    RetryableFailure { status: u16, message: String },
    TerminalFailure { status: u16, message: String },
}

#[derive(Debug, Default)]
struct ProductProviderAdapter {
    responses: VecDeque<ProductProviderResponse>,
    requests: Vec<ProductProviderRequest>,
}

impl ProductProviderAdapter {
    fn new(responses: impl IntoIterator<Item = ProductProviderResponse>) -> Self {
        Self {
            responses: responses.into_iter().collect(),
            requests: Vec::new(),
        }
    }

    fn requests(&self) -> &[ProductProviderRequest] {
        &self.requests
    }
}

impl ModelHost for ProductProviderAdapter {
    async fn execute_model(&mut self, request: EngineModelRequest) -> ModelHostOutcome {
        self.requests.push(ProductProviderRequest {
            model: request.model,
            session_id: request.session_id,
            system_prompt: request.system_prompt,
            prompt_text: request
                .messages
                .iter()
                .flat_map(|message| &message.content)
                .filter_map(|content| match content {
                    Content::Text { text } => Some(text.as_str()),
                    Content::Image { .. }
                    | Content::Thinking { .. }
                    | Content::ToolUse { .. }
                    | Content::ToolResult { .. } => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
        });
        match self.responses.pop_front() {
            Some(ProductProviderResponse::Completed { text, usage }) => ModelHostOutcome::Completed {
                response: EngineModelResponse {
                    output: vec![Content::Text { text }],
                    stop_reason: StopReason::Stop,
                },
                usage: Some(usage),
            },
            Some(ProductProviderResponse::RetryableFailure { status, message }) => ModelHostOutcome::Failed {
                failure: EngineTerminalFailure {
                    message,
                    status: Some(status),
                    retryable: true,
                },
            },
            Some(ProductProviderResponse::TerminalFailure { status, message }) => ModelHostOutcome::Failed {
                failure: EngineTerminalFailure {
                    message,
                    status: Some(status),
                    retryable: false,
                },
            },
            None => ModelHostOutcome::Failed {
                failure: EngineTerminalFailure {
                    message: "product provider adapter has no response".to_string(),
                    status: None,
                    retryable: false,
                },
            },
        }
    }
}

fn seed(prompt: &str) -> EngineRunSeed {
    let submission = EnginePromptSubmission {
        messages: vec![EngineMessage {
            role: EngineMessageRole::User,
            content: vec![Content::Text {
                text: prompt.to_string(),
            }],
        }],
        model: MODEL.to_string(),
        system_prompt: "product-owned provider adapter".to_string(),
        max_tokens: Some(256),
        temperature: None,
        thinking: None,
        tools: Vec::new(),
        no_cache: true,
        cache_ttl: None,
        session_id: SESSION_ID.to_string(),
        model_request_slot_budget: 3,
    };
    let initial_state = EngineState::new();
    let first_outcome = reduce(&initial_state, &EngineInput::submit_user_prompt(submission));
    EngineRunSeed::new(initial_state, first_outcome)
}

fn block_on<F: Future>(future: F) -> F::Output {
    struct NoopWaker;
    impl Wake for NoopWaker {
        fn wake(self: Arc<Self>) {}
    }
    let waker = Waker::from(Arc::new(NoopWaker));
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}
