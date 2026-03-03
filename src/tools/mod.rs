//! Built-in tools

use async_trait::async_trait;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

use crate::agent::events::AgentEvent;

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
}

impl ToolContext {
    /// Create a new context with all fields.
    pub fn new(call_id: String, signal: CancellationToken, event_tx: Option<broadcast::Sender<AgentEvent>>) -> Self {
        Self {
            call_id,
            signal,
            event_tx,
        }
    }

    /// Emit a streaming progress line to the TUI.
    ///
    /// No-op if there is no event channel (e.g. headless / test mode).
    pub fn emit_progress(&self, text: &str) {
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(AgentEvent::ToolExecutionUpdate {
                call_id: self.call_id.clone(),
                partial: ToolResult::text(text),
            });
        }
    }
}

#[async_trait]
pub trait Tool: Send + Sync {
    /// Returns the tool's definition (name, description, parameters schema)
    fn definition(&self) -> &ToolDefinition;

    /// Execute the tool with the given parameters
    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value, // JSON Schema
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: Vec<ToolResultContent>,
    pub is_error: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
    /// If output was truncated, path to the full output file
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_output_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ToolResultContent {
    Text { text: String },
    Image { media_type: String, data: String },
}

impl ToolResult {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolResultContent::Text { text: text.into() }],
            is_error: false,
            details: None,
            full_output_path: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![ToolResultContent::Text { text: message.into() }],
            is_error: true,
            details: None,
            full_output_path: None,
        }
    }
}

pub mod ask;
pub mod bash;
pub mod commit;
pub mod sandbox;
pub mod delegate;
pub mod diff;
pub mod edit;
pub mod find;
pub mod grep;
pub mod image_gen;
pub mod ls;
pub mod matrix;
pub mod plugin_tool;
pub mod read;
pub mod review;
pub mod screenshot;
pub mod subagent;
pub mod todo;
pub mod watchdog;
pub mod nix;
pub mod truncation;
#[cfg(feature = "tui-validate")]
pub mod validate_tui;
pub mod validator_tool;
pub mod web;
pub mod write;

#[cfg(test)]
mod tests {
    use tokio_util::sync::CancellationToken;

    use super::*;

    #[test]
    fn context_emit_progress_no_channel_is_noop() {
        let ctx = ToolContext::new("call-1".to_string(), CancellationToken::new(), None);
        // Should not panic even without an event channel
        ctx.emit_progress("hello");
    }

    #[test]
    fn context_emit_progress_sends_event() {
        let (tx, mut rx) = broadcast::channel(16);
        let ctx = ToolContext::new("call-42".to_string(), CancellationToken::new(), Some(tx));

        ctx.emit_progress("step 1");
        ctx.emit_progress("step 2");

        let event1 = rx.try_recv().unwrap();
        let event2 = rx.try_recv().unwrap();

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

        let e1 = rx.try_recv().unwrap();
        let e2 = rx.try_recv().unwrap();

        // Both should arrive on the same channel
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
}
