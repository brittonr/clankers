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
use clanker_message::ToolDefinition;
use clanker_message::Usage;
use clankers_adapters::ApprovalPolicy;
use clankers_adapters::AtomicCancellationSource;
use clankers_adapters::CatalogToolExecutor;
use clankers_adapters::CollectingUsageObserver;
use clankers_adapters::EmbeddedCapability;
use clankers_adapters::EmbeddedToolCatalog;
use clankers_adapters::EmbeddedToolMetadata;
use clankers_adapters::EmbeddedToolRuntime;
use clankers_adapters::MemoryEventSink;
use clankers_adapters::NoopRetrySleeper;
use clankers_adapters::RedactionPolicy;
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
use clankers_engine_host::run_engine_turn;
use clankers_tool_host::ToolHostOutcome;

const MODEL: &str = "product-workbench-model";
const SYSTEM_PROMPT: &str = "product-owned workbench";
const SESSION_ID: &str = "product-workbench-session";
const TOOL_NAME: &str = "lookup_project_fact";
const FIRST_PROMPT: &str = "Look up the launch code name, then remember it.";
const FOLLOW_UP_PROMPT: &str = "What launch code did the tool report?";
const TOOL_TEXT: &str = "launch code name: Orchard";
const FIRST_ASSISTANT_TEXT: &str = "Stored the product fact: Orchard.";
const SECOND_ASSISTANT_TEXT: &str = "The tool reported Orchard.";
const MODEL_REQUEST_SLOT_BUDGET: u32 = 2;
const MAX_TOKENS: usize = 256;

fn main() {
    combined_product_workbench_preserves_context();
    missing_session_fails_closed_before_model_or_tool_execution();
    dangerous_tool_catalog_denies_before_product_tool_runs();
    println!("embedded-product-workbench passed");
}

fn combined_product_workbench_preserves_context() {
    let mut store = ProductSessionStore::default();
    store.create(ProductSession::new(SESSION_ID));

    let first_turn = run_product_turn(
        &store,
        SESSION_ID,
        FIRST_PROMPT,
        product_catalog(vec![read_only_tool(TOOL_NAME)]),
        [(TOOL_NAME, ScriptedToolExecutor::text_success(TOOL_TEXT))],
        [
            ProductModelResponse::ToolRequest {
                id: "lookup-1".to_string(),
                name: TOOL_NAME.to_string(),
                input: serde_json::json!({"key":"launch-code"}),
            },
            ProductModelResponse::Completed {
                text: FIRST_ASSISTANT_TEXT.to_string(),
                usage: usage(20, 9),
            },
        ],
    )
    .expect("first turn should run");

    assert_eq!(first_turn.report.final_state.phase, EngineTurnPhase::Finished);
    assert_eq!(first_turn.model.requests.len(), 2);
    assert_eq!(first_turn.model.requests[0].session_id, SESSION_ID);
    assert_eq!(first_turn.tools.catalog().tools[0].runtime, EmbeddedToolRuntime::ProductOwned);
    assert!(first_turn.report.last_outcome.terminal_failure.is_none());

    store
        .replace_messages_from_report(SESSION_ID, &first_turn.report)
        .expect("persist first transcript");
    store
        .append_receipt(
            SESSION_ID,
            receipt_from_report(SESSION_ID, 1, first_turn.model.requests.len(), &first_turn.report),
        )
        .expect("persist first receipt");

    let persisted = store.load(SESSION_ID).expect("session should reload");
    assert_eq!(persisted.messages.len(), 4);
    assert_eq!(persisted.receipts, vec![ProductTurnReceipt {
        session_id: SESSION_ID.to_string(),
        turn_index: 1,
        model_request_count: 2,
        tool_call_summaries: vec!["tool: lookup_project_fact -> tool-result:launch code name: Orchard".to_string()],
        input_tokens: 20,
        output_tokens: 9,
    }]);

    let second_turn =
        run_product_turn(&store, SESSION_ID, FOLLOW_UP_PROMPT, product_catalog(vec![read_only_tool(TOOL_NAME)]), [], [
            ProductModelResponse::Completed {
                text: SECOND_ASSISTANT_TEXT.to_string(),
                usage: usage(30, 7),
            },
        ])
        .expect("second turn should run");

    assert_eq!(second_turn.report.final_state.phase, EngineTurnPhase::Finished);
    assert_eq!(second_turn.model.requests.len(), 1);
    assert_eq!(second_turn.model.requests[0].roles_and_text(), vec![
        "user: Look up the launch code name, then remember it.",
        "assistant: tool-use:lookup_project_fact",
        "tool: tool-result:launch code name: Orchard",
        "assistant: Stored the product fact: Orchard.",
        "user: What launch code did the tool report?",
    ]);
}

fn missing_session_fails_closed_before_model_or_tool_execution() {
    let store = ProductSessionStore::default();
    let result = run_product_turn(
        &store,
        "missing-session",
        FOLLOW_UP_PROMPT,
        product_catalog(vec![read_only_tool(TOOL_NAME)]),
        [(TOOL_NAME, ScriptedToolExecutor::text_success("must not run"))],
        [ProductModelResponse::Completed {
            text: "must not run".to_string(),
            usage: usage(1, 1),
        }],
    );

    assert_eq!(result.unwrap_err(), ProductError::MissingSession {
        session_id: "missing-session".to_string(),
    });
    assert_eq!(store.session_count(), 0);
}

fn dangerous_tool_catalog_denies_before_product_tool_runs() {
    let mut tool = read_only_tool("dangerous_shell");
    tool.capabilities = vec![EmbeddedCapability::Shell];
    tool.approval = ApprovalPolicy::Never;
    let mut executor = CatalogToolExecutor::new(product_catalog(vec![tool]))
        .with_outcome("dangerous_shell", ScriptedToolExecutor::text_success("must not execute"));

    let outcome = block_on(clankers_tool_host::ToolExecutor::execute_tool(&mut executor, tool_call("dangerous_shell")));
    assert!(matches!(outcome, ToolHostOutcome::ToolError { .. }));
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProductSession {
    session_id: String,
    messages: Vec<ProductMessage>,
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
struct ProductMessage {
    role: ProductRole,
    content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProductRole {
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProductTurnReceipt {
    session_id: String,
    turn_index: usize,
    model_request_count: usize,
    tool_call_summaries: Vec<String>,
    input_tokens: usize,
    output_tokens: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProductError {
    MissingSession { session_id: String },
    UnsupportedContent { role: ProductRole, kind: &'static str },
}

#[derive(Debug, Default)]
struct ProductSessionStore {
    sessions: BTreeMap<String, ProductSession>,
}

impl ProductSessionStore {
    fn create(&mut self, session: ProductSession) {
        self.sessions.insert(session.session_id.clone(), session);
    }

    fn load(&self, session_id: &str) -> Result<ProductSession, ProductError> {
        self.sessions.get(session_id).cloned().ok_or_else(|| ProductError::MissingSession {
            session_id: session_id.to_string(),
        })
    }

    fn session_count(&self) -> usize {
        self.sessions.len()
    }

    fn replace_messages_from_report(&mut self, session_id: &str, report: &EngineRunReport) -> Result<(), ProductError> {
        let messages =
            report.final_state.messages.iter().map(product_message_from_engine).collect::<Result<Vec<_>, _>>()?;
        let session = self.sessions.get_mut(session_id).ok_or_else(|| ProductError::MissingSession {
            session_id: session_id.to_string(),
        })?;
        session.messages = messages;
        Ok(())
    }

    fn append_receipt(&mut self, session_id: &str, receipt: ProductTurnReceipt) -> Result<(), ProductError> {
        let session = self.sessions.get_mut(session_id).ok_or_else(|| ProductError::MissingSession {
            session_id: session_id.to_string(),
        })?;
        session.receipts.push(receipt);
        Ok(())
    }
}

#[derive(Debug)]
struct ProductTurnRun {
    report: EngineRunReport,
    model: ProductModelAdapter,
    tools: CatalogToolExecutor,
}

fn run_product_turn(
    store: &ProductSessionStore,
    session_id: &str,
    user_prompt: &str,
    catalog: EmbeddedToolCatalog,
    tool_outcomes: impl IntoIterator<Item = (&'static str, ToolHostOutcome)>,
    model_responses: impl IntoIterator<Item = ProductModelResponse>,
) -> Result<ProductTurnRun, ProductError> {
    let session = store.load(session_id)?;
    let mut history = session.messages.iter().map(engine_message_from_product).collect::<Result<Vec<_>, _>>()?;
    history.push(EngineMessage {
        role: EngineMessageRole::User,
        content: vec![Content::Text {
            text: user_prompt.to_string(),
        }],
    });

    let mut model = ProductModelAdapter::new(model_responses);
    let mut tools = CatalogToolExecutor::new(catalog);
    for (tool_name, outcome) in tool_outcomes {
        tools = tools.with_outcome(tool_name, outcome);
    }
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

    Ok(ProductTurnRun { report, model, tools })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProductModelRequest {
    model: String,
    session_id: String,
    messages: Vec<ProductMessage>,
}

impl ProductModelRequest {
    fn roles_and_text(&self) -> Vec<String> {
        self.messages
            .iter()
            .map(|message| format!("{}: {}", role_name(message.role), message.content))
            .collect()
    }
}

#[derive(Debug, Clone)]
enum ProductModelResponse {
    ToolRequest {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    Completed {
        text: String,
        usage: Usage,
    },
}

#[derive(Debug, Default)]
struct ProductModelAdapter {
    responses: VecDeque<ProductModelResponse>,
    requests: Vec<ProductModelRequest>,
}

impl ProductModelAdapter {
    fn new(responses: impl IntoIterator<Item = ProductModelResponse>) -> Self {
        Self {
            responses: responses.into_iter().collect(),
            requests: Vec::new(),
        }
    }
}

impl ModelHost for ProductModelAdapter {
    async fn execute_model(&mut self, request: EngineModelRequest) -> ModelHostOutcome {
        let messages = request.messages.iter().map(product_message_from_engine).collect::<Result<Vec<_>, _>>();
        let Ok(messages) = messages else {
            return terminal_model_failure("product model received unsupported content");
        };
        self.requests.push(ProductModelRequest {
            model: request.model,
            session_id: request.session_id,
            messages,
        });

        match self.responses.pop_front() {
            Some(ProductModelResponse::ToolRequest { id, name, input }) => ModelHostOutcome::Completed {
                response: EngineModelResponse {
                    output: vec![Content::ToolUse { id, name, input }],
                    stop_reason: StopReason::ToolUse,
                },
                usage: None,
            },
            Some(ProductModelResponse::Completed { text, usage }) => ModelHostOutcome::Completed {
                response: EngineModelResponse {
                    output: vec![Content::Text { text }],
                    stop_reason: StopReason::Stop,
                },
                usage: Some(usage),
            },
            None => terminal_model_failure("product model adapter has no scripted response"),
        }
    }
}

fn seed(session_id: &str, messages: Vec<EngineMessage>) -> EngineRunSeed {
    let submission = EnginePromptSubmission {
        messages,
        model: MODEL.to_string(),
        system_prompt: SYSTEM_PROMPT.to_string(),
        max_tokens: Some(MAX_TOKENS),
        temperature: None,
        thinking: None,
        tools: vec![ToolDefinition {
            name: TOOL_NAME.to_string(),
            description: "lookup product workbench facts".to_string(),
            input_schema: serde_json::json!({"type":"object"}),
        }],
        no_cache: true,
        cache_ttl: None,
        session_id: session_id.to_string(),
        model_request_slot_budget: MODEL_REQUEST_SLOT_BUDGET,
    };
    let initial_state = EngineState::new();
    let first_outcome = reduce(&initial_state, &EngineInput::submit_user_prompt(submission));
    EngineRunSeed::new(initial_state, first_outcome)
}

fn product_catalog(tools: Vec<EmbeddedToolMetadata>) -> EmbeddedToolCatalog {
    EmbeddedToolCatalog { tools }
}

fn read_only_tool(name: &str) -> EmbeddedToolMetadata {
    EmbeddedToolMetadata {
        name: name.to_string(),
        description: format!("{name} product-owned lookup"),
        runtime: EmbeddedToolRuntime::ProductOwned,
        capabilities: vec![EmbeddedCapability::Read],
        approval: ApprovalPolicy::Never,
        redaction: RedactionPolicy::None,
        input_schema: serde_json::json!({"type":"object"}),
    }
}

fn tool_call(name: &str) -> clankers_engine::EngineToolCall {
    clankers_engine::EngineToolCall {
        call_id: clankers_engine::EngineCorrelationId("manual".to_string()),
        tool_name: name.to_string(),
        input: serde_json::json!({}),
    }
}

fn engine_message_from_product(message: &ProductMessage) -> Result<EngineMessage, ProductError> {
    Ok(EngineMessage {
        role: match message.role {
            ProductRole::User => EngineMessageRole::User,
            ProductRole::Assistant => EngineMessageRole::Assistant,
            ProductRole::Tool => EngineMessageRole::Tool,
        },
        content: vec![Content::Text {
            text: message.content.clone(),
        }],
    })
}

fn product_message_from_engine(message: &EngineMessage) -> Result<ProductMessage, ProductError> {
    let role = match message.role {
        EngineMessageRole::User => ProductRole::User,
        EngineMessageRole::Assistant => ProductRole::Assistant,
        EngineMessageRole::Tool => ProductRole::Tool,
    };
    Ok(ProductMessage {
        role,
        content: text_summary(role, &message.content)?,
    })
}

fn text_summary(role: ProductRole, content: &[Content]) -> Result<String, ProductError> {
    let mut text = Vec::new();
    for block in content {
        match block {
            Content::Text { text: block_text } => text.push(block_text.clone()),
            Content::ToolUse { name, .. } => text.push(format!("tool-use:{name}")),
            Content::ToolResult { content, .. } => text.push(tool_result_summary(content)),
            Content::Image { .. } => return Err(ProductError::UnsupportedContent { role, kind: "image" }),
            Content::Thinking { .. } => {
                return Err(ProductError::UnsupportedContent { role, kind: "thinking" });
            }
        }
    }
    Ok(text.join("\n"))
}

fn tool_result_summary(content: &[Content]) -> String {
    let text = content
        .iter()
        .filter_map(|block| match block {
            Content::Text { text } => Some(text.as_str()),
            Content::Image { .. } | Content::Thinking { .. } | Content::ToolUse { .. } | Content::ToolResult { .. } => {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!("tool-result:{text}")
}

fn receipt_from_report(
    session_id: &str,
    turn_index: usize,
    model_request_count: usize,
    report: &EngineRunReport,
) -> ProductTurnReceipt {
    let usage = report.usage_observations.last().map(|observation| &observation.usage);
    ProductTurnReceipt {
        session_id: session_id.to_string(),
        turn_index,
        model_request_count,
        tool_call_summaries: report
            .final_state
            .messages
            .iter()
            .filter(|message| message.role == EngineMessageRole::Tool)
            .map(tool_message_summary)
            .collect(),
        input_tokens: usage.map_or(0, |usage| usage.input_tokens),
        output_tokens: usage.map_or(0, |usage| usage.output_tokens),
    }
}

fn tool_message_summary(message: &EngineMessage) -> String {
    let summary = message
        .content
        .iter()
        .map(|content| match content {
            Content::Text { text } => format!("text: {}", text.len()),
            Content::ToolResult { content, .. } => tool_result_summary(content),
            Content::ToolUse { name, .. } => format!("tool-use:{name}"),
            Content::Image { .. } => "image".to_string(),
            Content::Thinking { .. } => "thinking".to_string(),
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("tool: {TOOL_NAME} -> {summary}")
}

fn terminal_model_failure(message: &str) -> ModelHostOutcome {
    ModelHostOutcome::Failed {
        failure: EngineTerminalFailure {
            message: message.to_string(),
            status: None,
            retryable: false,
        },
    }
}

fn role_name(role: ProductRole) -> &'static str {
    match role {
        ProductRole::User => "user",
        ProductRole::Assistant => "assistant",
        ProductRole::Tool => "tool",
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
