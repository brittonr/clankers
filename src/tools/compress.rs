//! Context compression tool — LLM-based conversation summarization
//!
//! Summarizes older messages to free context window space while preserving
//! semantic content. Keeps recent messages intact.

use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::Mutex;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolDefinition, ToolResult};

/// The compression prompt sent to the summarization model.
pub const COMPRESSION_PROMPT: &str = "\
Summarize the following conversation concisely. Preserve all important information.

Output exactly these sections:
## Topics Covered
- (bullet list of topics discussed)

## Decisions Made
- (bullet list of decisions and conclusions reached)

## Files Touched
- (bullet list of files read, written, or modified)

## Open Threads
- (bullet list of unresolved items or next steps)

Keep the summary under 2000 characters. Be precise and factual.";

/// Shared slot the turn loop reads after each tool execution round.
/// When `Some(summary)`, the loop replaces older messages with the summary.
pub type CompressionSlot = Arc<Mutex<Option<CompressionResult>>>;

pub fn compression_slot() -> CompressionSlot {
    Arc::new(Mutex::new(None))
}

/// Result of a compression operation.
#[derive(Debug, Clone)]
pub struct CompressionResult {
    /// The summary text to inject.
    pub summary: String,
    /// Number of messages to keep from the end (the rest are replaced).
    pub keep_recent: usize,
    /// Token count before compression.
    pub before_tokens: usize,
}

pub struct CompressTool {
    definition: ToolDefinition,
    slot: CompressionSlot,
    keep_recent: usize,
    #[allow(dead_code)] // Used when turn loop validates compression preconditions
    min_messages: usize,
}

impl CompressTool {
    pub fn new(slot: CompressionSlot, keep_recent: usize, min_messages: usize) -> Self {
        Self {
            slot,
            keep_recent,
            min_messages,
            definition: ToolDefinition {
                name: "compress".to_string(),
                description: "Compress conversation context by summarizing older messages. \
                    Frees context window space while preserving key information. \
                    Recent messages are kept intact. Use when the context is getting large."
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
        }
    }
}

#[async_trait]
impl Tool for CompressTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, _params: Value) -> ToolResult {
        // The actual compression happens in the turn loop when it reads the slot.
        // Here we just validate preconditions and signal the intent.

        // Check message count (we can't access messages directly, but the turn
        // loop will validate this when processing the slot).
        // For now, just signal the compression request.

        let result = CompressionResult {
            summary: String::new(), // Turn loop fills this in
            keep_recent: self.keep_recent,
            before_tokens: 0, // Turn loop fills this in
        };

        {
            let mut slot = self.slot.lock();
            *slot = Some(result);
        }

        ctx.emit_progress("Compression requested. Context will be summarized on the next turn.");

        ToolResult::text(format!(
            "Context compression requested. The {} most recent messages will be preserved, \
             older messages will be replaced with a structured summary.",
            self.keep_recent
        ))
    }
}

#[cfg(test)]
mod tests {
    use tokio_util::sync::CancellationToken;

    use super::*;

    fn make_ctx() -> ToolContext {
        ToolContext::new("test".to_string(), CancellationToken::new(), None)
    }

    fn result_text(result: &ToolResult) -> String {
        result
            .content
            .iter()
            .filter_map(|c| match c {
                crate::tools::ToolResultContent::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    #[tokio::test]
    async fn test_compress_signals_slot() {
        let slot = compression_slot();
        let tool = CompressTool::new(slot.clone(), 4, 5);
        let ctx = make_ctx();

        let result = tool.execute(&ctx, json!({})).await;
        assert!(!result.is_error);
        assert!(result_text(&result).contains("compression requested"));

        let pending = slot.lock();
        assert!(pending.is_some());
        assert_eq!(pending.as_ref().unwrap().keep_recent, 4);
    }

    #[tokio::test]
    async fn test_compress_preserves_keep_recent() {
        let slot = compression_slot();
        let tool = CompressTool::new(slot.clone(), 8, 5);

        let result = tool.execute(&make_ctx(), json!({})).await;
        let text = result_text(&result);
        assert!(text.contains("8 most recent"));
    }
}
