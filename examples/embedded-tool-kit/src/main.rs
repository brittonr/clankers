use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

use clanker_message::{Content, ToolDefinition};
use clankers_adapters::{ApprovalPolicy, AtomicCancellationSource, CatalogToolExecutor, CollectingUsageObserver, EmbeddedCapability, EmbeddedToolCatalog, EmbeddedToolMetadata, EmbeddedToolRuntime, MemoryEventSink, NoopRetrySleeper, RedactionPolicy, ScriptedModelHost, ScriptedToolExecutor};
use clankers_engine::{EngineInput, EngineMessage, EngineMessageRole, EnginePromptSubmission, EngineState, reduce};
use clankers_engine_host::{EngineRunSeed, HostAdapters, run_engine_turn};
use clankers_tool_host::{ToolHostOutcome, ToolTruncationLimits};

const MODEL: &str = "embedded-tool-kit";
const SESSION_ID: &str = "embedded-tool-session";

fn main() {
    success_path();
    missing_tool_path();
    tool_error_path();
    capability_denial_path();
    truncation_path();
    println!("embedded-tool-kit passed");
}

fn success_path() {
    let report = run_with_tools(
        "lookup",
        CatalogToolExecutor::new(catalog(vec![safe_tool("lookup")]))
            .with_outcome("lookup", ScriptedToolExecutor::text_success("found product")),
    );
    assert!(report.last_outcome.terminal_failure.is_none());
}

fn missing_tool_path() {
    let mut executor = CatalogToolExecutor::new(catalog(vec![safe_tool("lookup")]));
    let outcome = block_on(clankers_tool_host::ToolExecutor::execute_tool(&mut executor, tool_call("missing")));
    assert!(matches!(outcome, ToolHostOutcome::MissingTool { .. }));
}

fn tool_error_path() {
    let mut executor = CatalogToolExecutor::new(catalog(vec![safe_tool("lookup")]))
        .with_outcome("lookup", ScriptedToolExecutor::text_error("tool exploded"));
    let outcome = block_on(clankers_tool_host::ToolExecutor::execute_tool(&mut executor, tool_call("lookup")));
    assert!(matches!(outcome, ToolHostOutcome::ToolError { .. }));
}

fn capability_denial_path() {
    let mut tool = safe_tool("danger");
    tool.capabilities = vec![EmbeddedCapability::Shell];
    tool.approval = ApprovalPolicy::Never;
    let mut executor = CatalogToolExecutor::new(catalog(vec![tool]));
    let outcome = block_on(clankers_tool_host::ToolExecutor::execute_tool(&mut executor, tool_call("danger")));
    assert!(matches!(outcome, ToolHostOutcome::ToolError { .. }));
}

fn truncation_path() {
    let mut executor = CatalogToolExecutor::new(catalog(vec![safe_tool("lookup")]))
        .with_limits(ToolTruncationLimits { max_bytes: 4, max_lines: 1 })
        .with_outcome("lookup", ScriptedToolExecutor::text_success("abcdef"));
    let outcome = block_on(clankers_tool_host::ToolExecutor::execute_tool(&mut executor, clankers_engine::EngineToolCall {
        call_id: clankers_engine::EngineCorrelationId("manual".to_string()),
        tool_name: "lookup".to_string(),
        input: serde_json::json!({}),
    }));
    assert!(matches!(outcome, ToolHostOutcome::Truncated { .. }));
}

fn run_with_tools(tool_name: &str, mut tools: CatalogToolExecutor) -> clankers_engine_host::EngineRunReport {
    let seed = seed(tool_name);
    let mut model = ScriptedModelHost::new([
        ScriptedModelHost::tool_request("tool-call-1", tool_name, serde_json::json!({"query":"kit"})),
        ScriptedModelHost::completed_text("done"),
    ]);
    let mut retry = NoopRetrySleeper::default();
    let mut events = MemoryEventSink::default();
    let mut cancellation = AtomicCancellationSource::default();
    let mut usage = CollectingUsageObserver::default();
    block_on(run_engine_turn(seed, HostAdapters {
        model: &mut model,
        tools: &mut tools,
        retry_sleeper: &mut retry,
        event_sink: &mut events,
        cancellation: &mut cancellation,
        usage_observer: &mut usage,
    }))
}

fn seed(tool_name: &str) -> EngineRunSeed {
    let submission = EnginePromptSubmission {
        messages: vec![EngineMessage { role: EngineMessageRole::User, content: vec![Content::Text { text: format!("use {tool_name}") }] }],
        model: MODEL.to_string(),
        system_prompt: "tool embedded kit".to_string(),
        max_tokens: None,
        temperature: None,
        thinking: None,
        tools: vec![ToolDefinition { name: tool_name.to_string(), description: "example tool".to_string(), input_schema: serde_json::json!({}) }],
        no_cache: true,
        cache_ttl: None,
        session_id: SESSION_ID.to_string(),
        model_request_slot_budget: 2,
    };
    let initial_state = EngineState::new();
    let first_outcome = reduce(&initial_state, &EngineInput::submit_user_prompt(submission));
    EngineRunSeed::new(initial_state, first_outcome)
}

fn catalog(tools: Vec<EmbeddedToolMetadata>) -> EmbeddedToolCatalog { EmbeddedToolCatalog { tools } }

fn tool_call(name: &str) -> clankers_engine::EngineToolCall {
    clankers_engine::EngineToolCall {
        call_id: clankers_engine::EngineCorrelationId("manual".to_string()),
        tool_name: name.to_string(),
        input: serde_json::json!({}),
    }
}

fn safe_tool(name: &str) -> EmbeddedToolMetadata {
    EmbeddedToolMetadata {
        name: name.to_string(),
        description: format!("{name} tool"),
        runtime: EmbeddedToolRuntime::ProductOwned,
        capabilities: vec![EmbeddedCapability::Read],
        approval: ApprovalPolicy::Never,
        redaction: RedactionPolicy::None,
        input_schema: serde_json::json!({}),
    }
}

fn block_on<F: Future>(future: F) -> F::Output {
    struct NoopWaker;
    impl Wake for NoopWaker { fn wake(self: Arc<Self>) {} }
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
