//! Daemon-to-client events.

use serde::Deserialize;
use serde::Serialize;

use crate::types::ImageData;

/// Events sent from the daemon to connected clients.
// r[impl protocol.serde.event-externally-tagged]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DaemonEvent {
    // ── Agent lifecycle ─────────────────────────
    /// Agent started processing a prompt.
    AgentStart,
    /// Agent finished processing.
    AgentEnd,

    // ── Streaming ───────────────────────────────
    /// A new content block started.
    ContentBlockStart { is_thinking: bool },
    /// Incremental text delta.
    TextDelta { text: String },
    /// Incremental thinking delta.
    ThinkingDelta { text: String },
    /// Content block finished.
    ContentBlockStop,

    // ── Tool events ─────────────────────────────
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
        images: Vec<ImageData>,
    },
    /// Tool structured progress update.
    ToolProgressUpdate {
        call_id: String,
        progress: serde_json::Value,
    },
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
        images: Vec<ImageData>,
        is_error: bool,
    },

    // ── Session events ──────────────────────────
    /// User input was submitted.
    UserInput { text: String, agent_msg_count: usize },
    /// Session was auto-compacted.
    SessionCompaction {
        compacted_count: usize,
        tokens_saved: usize,
    },
    /// Usage update from the agent.
    UsageUpdate {
        input_tokens: u64,
        output_tokens: u64,
        cache_read: u64,
        model: String,
    },
    /// Model was changed.
    ModelChanged { from: String, to: String, reason: String },

    // ── Confirmation requests ───────────────────
    /// Bash tool needs confirmation before executing.
    ConfirmRequest {
        request_id: String,
        command: String,
        working_dir: String,
    },
    /// Todo tool needs a response.
    TodoRequest {
        request_id: String,
        action: serde_json::Value,
    },

    // ── Session metadata ────────────────────────
    /// Session info sent after handshake.
    SessionInfo {
        session_id: String,
        model: String,
        system_prompt_hash: String,
        #[serde(default)]
        available_models: Vec<String>,
        #[serde(default)]
        active_account: String,
        #[serde(default)]
        disabled_tools: Vec<String>,
        #[serde(default)]
        auto_test_command: Option<String>,
    },
    /// Response to GetSystemPrompt.
    SystemPromptResponse { prompt: String },

    // ── Subagent events ─────────────────────────
    /// A subagent was spawned.
    SubagentStarted {
        id: String,
        name: String,
        task: String,
        pid: Option<u32>,
    },
    /// Output line from a subagent.
    SubagentOutput { id: String, line: String },
    /// Subagent completed successfully.
    SubagentDone { id: String },
    /// Subagent failed.
    SubagentError { id: String, message: String },

    // ── Capability events ───────────────────────
    /// Response to GetCapabilities — None means full access.
    Capabilities { capabilities: Option<Vec<String>> },
    /// Tool call was blocked by capability enforcement.
    ToolBlocked {
        call_id: String,
        tool_name: String,
        reason: String,
    },

    // ── Tool metadata ────────────────────────────
    /// Full tool list available in this session.
    ToolList {
        tools: Vec<ToolInfo>,
    },
    /// Disabled tools changed.
    DisabledToolsChanged {
        tools: Vec<String>,
    },

    // ── Thinking / loop / auto-test state ───────
    /// Thinking level changed.
    ThinkingLevelChanged {
        from: String,
        to: String,
    },
    /// Loop status update.
    LoopStatus {
        active: bool,
        iteration: Option<u32>,
        max_iterations: Option<u32>,
        break_condition: Option<String>,
    },
    /// Auto-test state changed.
    AutoTestChanged {
        enabled: bool,
        command: Option<String>,
    },

    // ── Cost / accounting ───────────────────────
    /// Cumulative cost update for the session.
    CostUpdate {
        total_cost_usd: f64,
        total_input_tokens: u64,
        total_output_tokens: u64,
    },

    // ── System messages ─────────────────────────
    /// System message for display.
    SystemMessage { text: String, is_error: bool },
    /// Prompt processing finished.
    PromptDone { error: Option<String> },

    // ── History replay ──────────────────────────
    /// One block of conversation history.
    HistoryBlock { block: serde_json::Value },
    /// History replay is complete.
    HistoryEnd,
}

/// Metadata about a single tool.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
}
