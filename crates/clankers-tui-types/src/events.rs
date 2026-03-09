//! TUI event types — display-relevant events the TUI can consume.
//!
//! The main crate translates `AgentEvent` → `TuiEvent` at the boundary
//! so the TUI never imports agent, provider, or tool types.

use crate::DisplayImage;
use crate::ToolProgress;

/// Events the TUI can receive from the application layer.
#[derive(Debug, Clone)]
pub enum TuiEvent {
    // ── Agent lifecycle ──────────────────────────────
    /// Agent started processing a prompt.
    AgentStart,
    /// Agent finished processing.
    AgentEnd,

    // ── Streaming ────────────────────────────────────
    /// A new content block started.
    ContentBlockStart { is_thinking: bool },
    /// Incremental text delta.
    TextDelta(String),
    /// Incremental thinking delta.
    ThinkingDelta(String),
    /// Content block finished.
    ContentBlockStop,

    // ── Tool events ──────────────────────────────────
    /// Tool was called by the model.
    ToolCall {
        tool_name: String,
        call_id: String,
        input: serde_json::Value,
    },
    /// Tool started executing.
    ToolStart { call_id: String, tool_name: String },
    /// Tool produced partial output (streaming).
    ToolOutput {
        call_id: String,
        text: String,
        images: Vec<DisplayImage>,
    },
    /// Tool structured progress update.
    ToolProgressUpdate { call_id: String, progress: ToolProgress },
    /// Tool result chunk (streaming accumulation).
    ToolChunk {
        call_id: String,
        content: String,
        content_type: String,
    },
    /// Tool finished executing.
    ToolDone {
        call_id: String,
        text: String,
        images: Vec<DisplayImage>,
        is_error: bool,
    },

    // ── Session events ───────────────────────────────
    /// User input was submitted.
    UserInput { text: String, agent_msg_count: usize },
    /// Session was auto-compacted.
    SessionCompaction {
        compacted_count: usize,
        tokens_saved: usize,
    },
    /// Usage update from the agent.
    UsageUpdate {
        total_tokens: usize,
        input_tokens: usize,
        output_tokens: usize,
        cache_creation_input_tokens: usize,
        cache_read_input_tokens: usize,
        turn_tokens: usize,
    },
}
