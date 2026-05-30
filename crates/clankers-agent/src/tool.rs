//! Tool trait and execution context

use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use async_trait::async_trait;
// ToolResult and ToolResultContent — canonical definitions in clanker-message.
pub use clanker_message::ToolResult;
pub use clanker_message::ToolResultContent;
// Re-export ToolDefinition from clanker-router (canonical definition)
pub use clanker_router::provider::ToolDefinition;
use parking_lot::Mutex;
use serde_json::Value;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use crate::events::AgentEvent;

/// Re-export progress types from their canonical crates.
pub mod progress {
    // ProgressKind and ToolProgress — canonical definitions in clanker-tui-types.
    // ResultChunk, TruncationConfig, ToolResultAccumulator — canonical definitions in clanker-message.
    pub use clanker_message::ResultChunk;
    pub use clanker_message::ToolResultAccumulator;
    pub use clanker_message::TruncationConfig;
    pub use clanker_tui_types::ProgressKind;
    pub use clanker_tui_types::ToolProgress;
}

/// Shared slot the turn loop reads after each tool execution round.
/// When `Some(model_id)`, the loop switches to that model for the next
/// LLM call, then clears the slot.
pub type ModelSwitchSlot = Arc<Mutex<Option<String>>>;

/// Create a new empty model switch slot.
pub fn model_switch_slot() -> ModelSwitchSlot {
    Arc::new(Mutex::new(None))
}

/// Throttle state for progress updates
struct ThrottleState {
    /// Last time we emitted a structured progress event
    last_progress_emit: Option<Instant>,
    /// Minimum interval between progress emissions (default: 100ms)
    min_interval: Duration,
}

impl Default for ThrottleState {
    fn default() -> Self {
        Self {
            last_progress_emit: None,
            min_interval: Duration::from_millis(100),
        }
    }
}

/// Execution context passed to every tool invocation.
///
/// Bundles the call identity, cancellation signal, and an optional event
/// channel so that any tool can stream partial progress updates to the TUI
/// without needing per-tool wiring.
#[derive(Clone)]
pub struct ToolContext {
    /// Unique identifier for this tool call (matches `ToolCall.call_id`)
    pub call_id: String,
    /// Cancellation token — tools should check this periodically
    pub signal: CancellationToken,
    /// Optional event bus for streaming partial results to the TUI
    event_tx: Option<broadcast::Sender<AgentEvent>>,
    /// Throttle state for structured progress updates
    throttle_state: Arc<Mutex<ThrottleState>>,
    /// Optional hook pipeline for pre/post tool hooks
    hook_pipeline: Option<Arc<clankers_hooks::HookPipeline>>,
    /// Session ID for hook payloads
    session_id: String,
    /// Optional database handle for tools that need persistent storage
    db: Option<clankers_db::Db>,
    /// Optional full-text search index for session content
    search_index: Option<Arc<clankers_db::search_index::SearchIndex>>,
}

impl ToolContext {
    /// Create a new context with all fields.
    pub fn new(call_id: String, signal: CancellationToken, event_tx: Option<broadcast::Sender<AgentEvent>>) -> Self {
        Self {
            call_id,
            signal,
            event_tx,
            throttle_state: Arc::new(Mutex::new(ThrottleState::default())),
            hook_pipeline: None,
            session_id: String::new(),
            db: None,
            search_index: None,
        }
    }

    /// Attach a session ID to this context.
    pub fn with_session_id(mut self, session_id: String) -> Self {
        self.session_id = session_id;
        self
    }

    /// Attach a hook pipeline to this context.
    pub fn with_hooks(mut self, pipeline: Arc<clankers_hooks::HookPipeline>, session_id: String) -> Self {
        self.hook_pipeline = Some(pipeline);
        self.session_id = session_id;
        self
    }

    /// Attach a database handle to this context.
    pub fn with_db(mut self, db: clankers_db::Db) -> Self {
        self.db = Some(db);
        self
    }

    /// Access the database handle (if set).
    pub fn db(&self) -> Option<&clankers_db::Db> {
        self.db.as_ref()
    }

    /// Attach a search index to this context.
    pub fn with_search_index(mut self, index: Arc<clankers_db::search_index::SearchIndex>) -> Self {
        self.search_index = Some(index);
        self
    }

    /// Access the search index (if set).
    pub fn search_index(&self) -> Option<&clankers_db::search_index::SearchIndex> {
        self.search_index.as_deref()
    }

    /// Access the hook pipeline (if set).
    pub fn hook_pipeline(&self) -> Option<&Arc<clankers_hooks::HookPipeline>> {
        self.hook_pipeline.as_ref()
    }

    /// Session ID for hook payloads.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Emit an arbitrary agent event on the event bus.
    pub fn emit_event(&self, event: AgentEvent) {
        if let Some(ref tx) = self.event_tx {
            tx.send(event).ok();
        }
    }

    /// Emit a streaming progress line to the TUI.
    ///
    /// No-op if there is no event channel (e.g. headless / test mode).
    pub fn emit_progress(&self, text: &str) {
        if let Some(ref tx) = self.event_tx {
            tx.send(AgentEvent::ToolExecutionUpdate {
                call_id: self.call_id.clone(),
                partial: ToolResult::text(text),
            })
            .ok();
        }
    }

    /// Emit structured progress update
    ///
    /// Throttled to max 10 updates/sec (100ms interval) per `call_id`.
    /// If called more frequently, the event is silently dropped.
    pub fn emit_structured_progress(&self, progress: progress::ToolProgress) {
        let mut state = self.throttle_state.lock();

        // Check throttle
        if let Some(last) = state.last_progress_emit
            && last.elapsed() < state.min_interval
        {
            // Drop this event (throttled)
            return;
        }

        // Update throttle state
        state.last_progress_emit = Some(Instant::now());
        drop(state);

        // Emit event
        if let Some(ref tx) = self.event_tx {
            tx.send(AgentEvent::ToolProgressUpdate {
                call_id: self.call_id.clone(),
                progress,
            })
            .ok();
        }
    }

    /// Emit a result chunk
    ///
    /// NOT throttled — result chunks are streamed as fast as produced.
    /// Back-pressure is handled by the event bus ring buffer (drop-oldest).
    pub fn emit_result_chunk(&self, chunk: progress::ResultChunk) {
        if let Some(ref tx) = self.event_tx {
            tx.send(AgentEvent::ToolResultChunk {
                call_id: self.call_id.clone(),
                chunk,
            })
            .ok();
        }
    }

    /// Configure throttle interval (for tests or special cases)
    ///
    /// Default is 100ms. Lower values = more events (higher TUI load).
    pub fn set_throttle_interval(&self, interval: Duration) {
        let mut state = self.throttle_state.lock();
        state.min_interval = interval;
    }
}

/// Gate for checking if a session operation is allowed by the session's capabilities.
///
/// Implementations inspect prompts, model switches, session-management actions,
/// and tool calls to decide whether to allow or block execution.
/// Returning `Err(reason)` blocks the operation and reports the reason to the caller.
pub trait CapabilityGate: Send + Sync {
    /// Check if a user prompt is allowed in this session.
    ///
    /// Returns `Ok(())` if the prompt should proceed, `Err(reason)` if blocked.
    fn check_prompt(&self, _session_id: &str, _text: &str) -> std::result::Result<(), String> {
        Ok(())
    }

    /// Check if a session-management action is allowed.
    ///
    /// Returns `Ok(())` if the action should proceed, `Err(reason)` if blocked.
    fn check_session_manage(&self, _session_id: &str, _action: &str) -> std::result::Result<(), String> {
        Ok(())
    }

    /// Check if switching to or selecting a model is allowed.
    ///
    /// Returns `Ok(())` if the model change should proceed, `Err(reason)` if blocked.
    fn check_model_switch(&self, _model: &str) -> std::result::Result<(), String> {
        Ok(())
    }

    /// Check if a tool call is allowed.
    ///
    /// Returns `Ok(())` if the call should proceed, `Err(reason)` if blocked.
    fn check_tool_call(&self, tool_name: &str, input: &Value) -> std::result::Result<(), String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolExecutionBackend {
    RustBuiltin,
    WasmPlugin,
    StdioPlugin,
    Subagent,
}

#[async_trait]
pub trait Tool: Send + Sync {
    /// Returns the tool's definition (name, description, parameters schema)
    fn definition(&self) -> &ToolDefinition;

    /// Execute the tool with the given parameters
    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult;

    /// Source label: "built-in" for core tools, plugin name for plugin tools.
    fn source(&self) -> &str {
        "built-in"
    }

    /// Backend kind used by the Steel tool/plugin/subagent substrate.
    fn execution_backend(&self) -> ToolExecutionBackend {
        ToolExecutionBackend::RustBuiltin
    }
}

/// Compatibility adapter from legacy agent tools to neutral tool-host execution.
pub struct LegacyAgentToolExecutor {
    tool: Arc<dyn Tool>,
}

impl LegacyAgentToolExecutor {
    #[must_use]
    pub fn new(tool: Arc<dyn Tool>) -> Self {
        Self { tool }
    }
}

impl clankers_tool_host::NeutralToolExecutor for LegacyAgentToolExecutor {
    async fn execute_tool_with_context(
        &mut self,
        call: clankers_engine::EngineToolCall,
        context: clankers_tool_host::ToolInvocationContext,
    ) -> clankers_tool_host::ToolHostOutcome {
        if let Err(outcome) = context.ensure_not_cancelled(&call.tool_name) {
            return outcome;
        }
        if let Err(outcome) = context.ensure_allowed(&call.tool_name) {
            return outcome;
        }
        let token = CancellationToken::new();
        let ctx = ToolContext::new(context.call_id.clone(), token, None);
        let result = self.tool.execute(&ctx, call.input).await;
        legacy_tool_result_to_host_outcome(&call.tool_name, result)
    }
}

fn legacy_tool_result_to_host_outcome(tool_name: &str, result: ToolResult) -> clankers_tool_host::ToolHostOutcome {
    let content = result.content.iter().map(tool_result_content_to_content).collect::<Vec<_>>();
    let details = result.details.unwrap_or_else(|| serde_json::json!({}));
    if result.is_error {
        return clankers_tool_host::ToolHostOutcome::ToolError {
            message: first_tool_result_text(&content).unwrap_or_else(|| format!("{tool_name} failed")),
            content,
            details,
        };
    }
    clankers_tool_host::ToolHostOutcome::Succeeded { content, details }
}

fn tool_result_content_to_content(content: &ToolResultContent) -> clanker_message::Content {
    match content {
        ToolResultContent::Text { text } => clanker_message::Content::Text { text: text.clone() },
        ToolResultContent::Image { media_type, data } => clanker_message::Content::Image {
            source: clanker_message::ImageSource::Base64 {
                media_type: media_type.clone(),
                data: data.clone(),
            },
        },
    }
}

fn first_tool_result_text(content: &[clanker_message::Content]) -> Option<String> {
    content.iter().find_map(|block| match block {
        clanker_message::Content::Text { text } => Some(text.clone()),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use clankers_tool_host::NeutralToolExecutor;
    use tokio_util::sync::CancellationToken;

    use super::*;

    struct FakeLegacyTool {
        definition: ToolDefinition,
    }

    #[async_trait]
    impl Tool for FakeLegacyTool {
        fn definition(&self) -> &ToolDefinition {
            &self.definition
        }

        async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
            ToolResult::text(format!("{}:{}", ctx.call_id, params["value"].as_str().unwrap_or("missing")))
        }
    }

    #[tokio::test]
    async fn legacy_tool_executor_bridges_neutral_context_to_agent_tool() {
        let tool = Arc::new(FakeLegacyTool {
            definition: ToolDefinition {
                name: "fake_legacy".to_string(),
                description: "fake legacy tool".to_string(),
                input_schema: serde_json::json!({"type":"object"}),
            },
        });
        let mut executor = LegacyAgentToolExecutor::new(tool);
        let call = clankers_engine::EngineToolCall {
            call_id: clankers_engine::EngineCorrelationId("call-neutral".to_string()),
            tool_name: "fake_legacy".to_string(),
            input: serde_json::json!({"value":"ok"}),
        };

        let outcome = executor
            .execute_tool_with_context(call.clone(), clankers_tool_host::ToolInvocationContext::new("call-neutral"))
            .await;
        let clankers_tool_host::ToolHostOutcome::Succeeded { content, .. } = outcome else {
            panic!("expected successful legacy bridge");
        };
        assert!(matches!(&content[0], clanker_message::Content::Text { text } if text == "call-neutral:ok"));

        let denied = executor
            .execute_tool_with_context(
                call,
                clankers_tool_host::ToolInvocationContext::new("call-neutral").with_capability(
                    clankers_tool_host::CapabilityDecision::Denied {
                        reason: "blocked".to_string(),
                    },
                ),
            )
            .await;
        assert!(
            matches!(denied, clankers_tool_host::ToolHostOutcome::CapabilityDenied { reason, .. } if reason == "blocked")
        );
    }

    #[test]
    fn context_emit_progress_no_channel_is_noop() {
        let ctx = ToolContext::new("call-1".to_string(), CancellationToken::new(), None);
        ctx.emit_progress("hello");
    }

    #[test]
    fn context_emit_progress_sends_event() {
        let (tx, mut rx) = broadcast::channel(16);
        let ctx = ToolContext::new("call-42".to_string(), CancellationToken::new(), Some(tx));

        ctx.emit_progress("step 1");
        ctx.emit_progress("step 2");

        let event1 = rx.try_recv().expect("should receive first event");
        let event2 = rx.try_recv().expect("should receive second event");

        match event1 {
            AgentEvent::ToolExecutionUpdate { call_id, partial } => {
                assert_eq!(call_id, "call-42");
                assert_eq!(partial.content.len(), 1);
                match &partial.content[0] {
                    ToolResultContent::Text { text } => assert_eq!(text, "step 1"),
                    _ => panic!("expected text"),
                }
            }
            _ => panic!("expected ToolExecutionUpdate, got {:?}", event1),
        }

        match event2 {
            AgentEvent::ToolExecutionUpdate { call_id, partial } => {
                assert_eq!(call_id, "call-42");
                match &partial.content[0] {
                    ToolResultContent::Text { text } => assert_eq!(text, "step 2"),
                    _ => panic!("expected text"),
                }
            }
            _ => panic!("expected ToolExecutionUpdate"),
        }
    }

    #[test]
    fn context_clone_shares_channel() {
        let (tx, mut rx) = broadcast::channel(16);
        let ctx1 = ToolContext::new("call-a".to_string(), CancellationToken::new(), Some(tx));
        let ctx2 = ctx1.clone();

        ctx1.emit_progress("from ctx1");
        ctx2.emit_progress("from ctx2");

        let e1 = rx.try_recv().expect("should receive e1");
        let e2 = rx.try_recv().expect("should receive e2");

        match (e1, e2) {
            (
                AgentEvent::ToolExecutionUpdate {
                    call_id: id1,
                    partial: p1,
                },
                AgentEvent::ToolExecutionUpdate {
                    call_id: id2,
                    partial: p2,
                },
            ) => {
                assert_eq!(id1, "call-a");
                assert_eq!(id2, "call-a");
                match (&p1.content[0], &p2.content[0]) {
                    (ToolResultContent::Text { text: t1 }, ToolResultContent::Text { text: t2 }) => {
                        assert_eq!(t1, "from ctx1");
                        assert_eq!(t2, "from ctx2");
                    }
                    _ => panic!("expected text"),
                }
            }
            _ => panic!("expected ToolExecutionUpdate events"),
        }
    }

    #[test]
    fn emit_structured_progress_throttles_rapid_calls() {
        let (tx, mut rx) = broadcast::channel(16);
        let ctx = ToolContext::new("call-1".to_string(), CancellationToken::new(), Some(tx));

        ctx.set_throttle_interval(Duration::from_millis(50));

        ctx.emit_structured_progress(progress::ToolProgress::lines(1, Some(100)));
        ctx.emit_structured_progress(progress::ToolProgress::lines(2, Some(100)));

        let event1 = rx.try_recv().expect("should receive first progress event");
        assert!(matches!(event1, AgentEvent::ToolProgressUpdate { .. }));
        assert!(rx.try_recv().is_err());

        std::thread::sleep(Duration::from_millis(60));

        ctx.emit_structured_progress(progress::ToolProgress::lines(3, Some(100)));
        let event2 = rx.try_recv().expect("should receive second progress event after throttle");
        assert!(matches!(event2, AgentEvent::ToolProgressUpdate { .. }));
    }

    #[test]
    fn emit_result_chunk_not_throttled() {
        let (tx, mut rx) = broadcast::channel(16);
        let ctx = ToolContext::new("call-1".to_string(), CancellationToken::new(), Some(tx));

        ctx.emit_result_chunk(progress::ResultChunk::text("chunk 1"));
        ctx.emit_result_chunk(progress::ResultChunk::text("chunk 2"));
        ctx.emit_result_chunk(progress::ResultChunk::text("chunk 3"));

        let e1 = rx.try_recv().expect("should receive chunk 1");
        let e2 = rx.try_recv().expect("should receive chunk 2");
        let e3 = rx.try_recv().expect("should receive chunk 3");

        assert!(matches!(e1, AgentEvent::ToolResultChunk { .. }));
        assert!(matches!(e2, AgentEvent::ToolResultChunk { .. }));
        assert!(matches!(e3, AgentEvent::ToolResultChunk { .. }));
    }

    #[test]
    fn emit_structured_progress_no_channel_is_noop() {
        let ctx = ToolContext::new("call-1".to_string(), CancellationToken::new(), None);
        ctx.emit_structured_progress(progress::ToolProgress::lines(42, None));
    }

    #[test]
    fn emit_result_chunk_no_channel_is_noop() {
        let ctx = ToolContext::new("call-1".to_string(), CancellationToken::new(), None);
        ctx.emit_result_chunk(progress::ResultChunk::text("test"));
    }
}
