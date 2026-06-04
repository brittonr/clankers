use std::collections::BTreeMap;
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
use clankers_engine::EngineTurnPhase;
use clankers_engine::reduce;
use clankers_engine_host::EngineRunReport;
use clankers_engine_host::EngineRunSeed;
use clankers_engine_host::HostAdapters;
use clankers_engine_host::ModelHost;
use clankers_engine_host::ModelHostOutcome;
use clankers_engine_host::SessionLedgerMessage;
use clankers_engine_host::SessionLedgerRole;
use clankers_engine_host::engine_messages_from_ledger_messages;
use clankers_engine_host::ledger_message_from_engine_message;
use clankers_engine_host::ledger_messages_from_engine_messages;
use clankers_engine_host::run_engine_turn;

const MODEL: &str = "product-session-model";
const SYSTEM_PROMPT: &str = "host-owned persistence recipe";
const SESSION_ID: &str = "product-session-42";
const FIRST_USER_PROMPT: &str = "Remember the launch code name is Orchard.";
const FIRST_ASSISTANT_TEXT: &str = "Stored: launch code name Orchard.";
const FOLLOW_UP_PROMPT: &str = "What launch code name did I give you?";
const SECOND_ASSISTANT_TEXT: &str = "You gave me Orchard as the launch code name.";
const MODEL_REQUEST_SLOT_BUDGET: u32 = 1;
const MAX_TOKENS: usize = 256;

fn main() {
    positive_restore_scenario();
    missing_session_fails_closed();
    println!("embedded-session-store passed");
}

fn positive_restore_scenario() {
    let mut store = InMemoryProductSessionStore::default();
    store.create(ProductSession::new(SESSION_ID));

    let first_turn = run_product_turn(&store, SESSION_ID, FIRST_USER_PROMPT, [ProductModelResponse::Completed {
        text: FIRST_ASSISTANT_TEXT.to_string(),
        usage: usage(8, 6),
    }])
    .expect("first turn should run");
    assert_eq!(first_turn.model.requests.len(), 1);
    assert_eq!(first_turn.model.requests[0].session_id, SESSION_ID);
    assert_eq!(first_turn.model.requests[0].roles_and_text(), vec![
        "user: Remember the launch code name is Orchard."
    ]);

    store
        .replace_messages_from_engine_report(SESSION_ID, &first_turn.report)
        .expect("persist first transcript");
    store
        .append_receipt(SESSION_ID, receipt_from_report(SESSION_ID, 1, &first_turn.report))
        .expect("persist first receipt");

    let reloaded = store.load(SESSION_ID).expect("session should reload");
    assert_eq!(reloaded.session_id, SESSION_ID);
    assert_eq!(reloaded.messages.len(), 2);
    assert_eq!(reloaded.receipts.len(), 1);
    assert_eq!(reloaded.receipts[0].session_id, SESSION_ID);
    assert_eq!(reloaded.receipts[0].output_tokens, 6);

    let second_turn = run_product_turn(&store, SESSION_ID, FOLLOW_UP_PROMPT, [ProductModelResponse::Completed {
        text: SECOND_ASSISTANT_TEXT.to_string(),
        usage: usage(14, 9),
    }])
    .expect("second turn should run");

    assert_eq!(second_turn.report.final_state.phase, EngineTurnPhase::Finished);
    assert!(second_turn.report.last_outcome.terminal_failure.is_none());
    assert!(second_turn.report.adapter_diagnostics.is_empty());
    assert_eq!(second_turn.model.requests.len(), 1);

    let request = &second_turn.model.requests[0];
    assert_eq!(request.session_id, SESSION_ID);
    assert_eq!(request.model, MODEL);
    assert_eq!(request.roles_and_text(), vec![
        "user: Remember the launch code name is Orchard.",
        "assistant: Stored: launch code name Orchard.",
        "user: What launch code name did I give you?",
    ]);
}

fn missing_session_fails_closed() {
    let store = InMemoryProductSessionStore::default();
    let before_count = store.session_count();
    let result = run_product_turn(&store, "missing-session", FOLLOW_UP_PROMPT, [ProductModelResponse::Completed {
        text: "should not run".to_string(),
        usage: usage(1, 1),
    }]);

    assert_eq!(result.unwrap_err(), ProductStoreError::MissingSession {
        session_id: "missing-session".to_string()
    });
    assert_eq!(store.session_count(), before_count);
}

#[derive(Debug, Clone)]
struct ProductSession {
    session_id: String,
    messages: Vec<SessionLedgerMessage>,
    receipts: Vec<ProductTurnReceipt>,
}

impl ProductSession {
    fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            messages: Vec::new(),
            receipts: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProductTurnReceipt {
    session_id: String,
    turn_index: usize,
    input_tokens: usize,
    output_tokens: usize,
    tool_call_summaries: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProductStoreError {
    MissingSession { session_id: String },
}

#[derive(Debug, Default)]
struct InMemoryProductSessionStore {
    sessions: BTreeMap<String, ProductSession>,
}

impl InMemoryProductSessionStore {
    fn create(&mut self, session: ProductSession) {
        self.sessions.insert(session.session_id.clone(), session);
    }

    fn load(&self, session_id: &str) -> Result<ProductSession, ProductStoreError> {
        self.sessions.get(session_id).cloned().ok_or_else(|| ProductStoreError::MissingSession {
            session_id: session_id.to_string(),
        })
    }

    fn session_count(&self) -> usize {
        self.sessions.len()
    }

    fn replace_messages_from_engine_report(
        &mut self,
        session_id: &str,
        report: &EngineRunReport,
    ) -> Result<(), ProductStoreError> {
        let messages = ledger_messages_from_engine_messages(&report.final_state.messages);
        let session = self.sessions.get_mut(session_id).ok_or_else(|| ProductStoreError::MissingSession {
            session_id: session_id.to_string(),
        })?;
        session.messages = messages;
        Ok(())
    }

    fn append_receipt(&mut self, session_id: &str, receipt: ProductTurnReceipt) -> Result<(), ProductStoreError> {
        let session = self.sessions.get_mut(session_id).ok_or_else(|| ProductStoreError::MissingSession {
            session_id: session_id.to_string(),
        })?;
        session.receipts.push(receipt);
        Ok(())
    }
}

#[derive(Debug)]
struct TurnRun {
    report: EngineRunReport,
    model: RecordingProductModelHost,
}

fn run_product_turn(
    store: &InMemoryProductSessionStore,
    session_id: &str,
    user_prompt: &str,
    responses: impl IntoIterator<Item = ProductModelResponse>,
) -> Result<TurnRun, ProductStoreError> {
    let session = store.load(session_id)?;
    let mut history = engine_messages_from_ledger_messages(&session.messages);
    history.push(EngineMessage {
        role: EngineMessageRole::User,
        content: vec![Content::Text {
            text: user_prompt.to_string(),
        }],
    });

    let mut model = RecordingProductModelHost::new(responses);
    let mut tools = ScriptedToolExecutor::default();
    let mut retry = NoopRetrySleeper::default();
    let mut events = MemoryEventSink::default();
    let mut cancellation = AtomicCancellationSource::default();
    let mut usage = CollectingUsageObserver::default();
    let report = block_on(run_engine_turn(seed(session_id, history), HostAdapters {
        model: &mut model,
        tools: &mut tools,
        retry_sleeper: &mut retry,
        event_sink: &mut events,
        cancellation: &mut cancellation,
        usage_observer: &mut usage,
    }));

    Ok(TurnRun { report, model })
}

fn seed(session_id: &str, messages: Vec<EngineMessage>) -> EngineRunSeed {
    let submission = EnginePromptSubmission {
        messages,
        model: MODEL.to_string(),
        system_prompt: SYSTEM_PROMPT.to_string(),
        max_tokens: Some(MAX_TOKENS),
        temperature: None,
        thinking: None,
        tools: Vec::new(),
        no_cache: true,
        cache_ttl: None,
        session_id: session_id.to_string(),
        model_request_slot_budget: MODEL_REQUEST_SLOT_BUDGET,
    };
    let initial_state = EngineState::new();
    let first_outcome = reduce(&initial_state, &EngineInput::submit_user_prompt(submission));
    EngineRunSeed::new(initial_state, first_outcome)
}

fn receipt_from_report(session_id: &str, turn_index: usize, report: &EngineRunReport) -> ProductTurnReceipt {
    let usage = report.usage_observations.last().map(|observation| &observation.usage);
    ProductTurnReceipt {
        session_id: session_id.to_string(),
        turn_index,
        input_tokens: usage.map_or(0, |usage| usage.input_tokens),
        output_tokens: usage.map_or(0, |usage| usage.output_tokens),
        tool_call_summaries: report
            .final_state
            .messages
            .iter()
            .filter(|message| message.role == EngineMessageRole::Tool)
            .flat_map(|message| message.content.iter())
            .map(content_summary)
            .collect(),
    }
}

fn content_summary(content: &Content) -> String {
    match content {
        Content::Text { text } => format!("text:{}", text.len()),
        Content::ToolUse { name, .. } => format!("tool-use:{name}"),
        Content::ToolResult { .. } => "tool-result".to_string(),
        Content::Image { .. } => "image".to_string(),
        Content::Thinking { .. } => "thinking".to_string(),
    }
}

#[derive(Debug, Clone)]
struct ProductModelRequest {
    model: String,
    session_id: String,
    messages: Vec<SessionLedgerMessage>,
}

impl ProductModelRequest {
    fn roles_and_text(&self) -> Vec<String> {
        self.messages
            .iter()
            .map(|message| format!("{}: {}", ledger_role_name(message.role), message.text_summary()))
            .collect()
    }
}

#[derive(Debug, Clone)]
enum ProductModelResponse {
    Completed { text: String, usage: Usage },
}

#[derive(Debug, Default)]
struct RecordingProductModelHost {
    responses: VecDeque<ProductModelResponse>,
    requests: Vec<ProductModelRequest>,
}

impl RecordingProductModelHost {
    fn new(responses: impl IntoIterator<Item = ProductModelResponse>) -> Self {
        Self {
            responses: responses.into_iter().collect(),
            requests: Vec::new(),
        }
    }
}

impl ModelHost for RecordingProductModelHost {
    async fn execute_model(&mut self, request: EngineModelRequest) -> ModelHostOutcome {
        let messages = request.messages.iter().map(ledger_message_from_engine_message).collect::<Vec<_>>();

        self.requests.push(ProductModelRequest {
            model: request.model,
            session_id: request.session_id,
            messages,
        });

        match self.responses.pop_front() {
            Some(ProductModelResponse::Completed { text, usage }) => ModelHostOutcome::Completed {
                response: EngineModelResponse {
                    output: vec![Content::Text { text }],
                    stop_reason: StopReason::Stop,
                },
                usage: Some(usage),
            },
            None => ModelHostOutcome::Failed {
                failure: EngineTerminalFailure {
                    message: "product model host has no scripted response".to_string(),
                    status: None,
                    retryable: false,
                },
            },
        }
    }
}

fn ledger_role_name(role: SessionLedgerRole) -> &'static str {
    match role {
        SessionLedgerRole::User => "user",
        SessionLedgerRole::Assistant => "assistant",
        SessionLedgerRole::Tool => "tool",
    }
}

fn usage(input_tokens: usize, output_tokens: usize) -> Usage {
    Usage {
        input_tokens,
        output_tokens,
        cache_creation_input_tokens: 0,
        cache_read_input_tokens: 0,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_session_store_smoke() {
        positive_restore_scenario();
        missing_session_fails_closed();
    }
}
