//! AgentEvent enum (lifecycle, tool, message events)

use serde_json::Value;

use crate::provider::Usage;
use crate::provider::message::AgentMessage;
use crate::provider::message::AssistantMessage;
use crate::provider::message::Content;
use crate::provider::message::MessageId;
use crate::tools::ToolResult;

/// All lifecycle events emitted by the agent during execution.
/// Consumed by TUI, JSON mode, print mode, plugins, etc.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    // Session lifecycle
    SessionStart {
        session_id: String,
    },
    SessionShutdown {
        session_id: String,
    },

    // Agent lifecycle
    AgentStart,
    AgentEnd {
        messages: Vec<AgentMessage>,
    },

    // Turn lifecycle
    TurnStart {
        index: u32,
    },
    TurnEnd {
        index: u32,
        message: AssistantMessage,
        tool_results: Vec<crate::provider::message::ToolResultMessage>,
    },

    // Message streaming
    MessageStart {
        message: AgentMessage,
    },
    /// A new content block has started streaming
    ContentBlockStart {
        index: usize,
        content_block: Content,
    },
    /// Incremental delta for a content block
    MessageUpdate {
        index: usize,
        delta: crate::provider::streaming::StreamDelta,
    },
    /// A content block has finished streaming
    ContentBlockStop {
        index: usize,
    },
    MessageEnd {
        message: AgentMessage,
    },

    // Tool events
    ToolCall {
        tool_name: String,
        call_id: String,
        input: Value,
    },
    ToolExecutionStart {
        call_id: String,
        tool_name: String,
    },
    ToolExecutionUpdate {
        call_id: String,
        partial: ToolResult,
    },
    ToolExecutionEnd {
        call_id: String,
        result: ToolResult,
        is_error: bool,
    },
    ToolResultEvent {
        tool_name: String,
        call_id: String,
        content: Vec<Content>,
        details: Option<Value>,
    },

    // Structured progress and result streaming
    /// Structured progress update from a tool
    ToolProgressUpdate {
        call_id: String,
        progress: crate::tools::progress::ToolProgress,
    },
    /// Result chunk streamed from a tool
    ToolResultChunk {
        call_id: String,
        chunk: crate::tools::progress::ResultChunk,
    },

    // Context
    BeforeAgentStart {
        prompt: String,
        system_prompt: String,
    },
    Context {
        messages: Vec<AgentMessage>,
    },

    // Session events
    SessionBranch {
        from_id: MessageId,
        branch_id: MessageId,
    },
    SessionCompaction {
        compacted_count: usize,
        tokens_saved: usize,
    },

    // Model events
    ModelChange {
        from: String,
        to: String,
        reason: String,
    },

    // Input events
    UserInput {
        text: String,
        /// Number of agent messages *before* this user message was appended
        agent_msg_count: usize,
    },
    UserCancel,

    // Cost tracking
    UsageUpdate {
        turn_usage: Usage,
        cumulative_usage: Usage,
    },

    // Process monitoring events
    ProcessSpawn {
        pid: u32,
        meta: crate::procmon::ProcessMeta,
    },
    ProcessSample {
        pid: u32,
        cpu_percent: f32,
        rss_bytes: u64,
        children: Vec<u32>,
    },
    ProcessExit {
        pid: u32,
        exit_code: Option<i32>,
        wall_time: std::time::Duration,
        peak_rss: u64,
    },
}

impl AgentEvent {
    /// String tag for plugin event matching.
    ///
    /// Returns the snake_case identifier that plugin manifests use
    /// in their `"events"` array to subscribe to this event type.
    pub fn event_kind(&self) -> &'static str {
        match self {
            Self::ToolCall { .. } => "tool_call",
            Self::ToolExecutionEnd { .. } => "tool_result",
            Self::AgentStart => "agent_start",
            Self::AgentEnd { .. } => "agent_end",
            Self::TurnStart { .. } => "turn_start",
            Self::TurnEnd { .. } => "turn_end",
            Self::MessageUpdate { .. } => "message_update",
            Self::UserInput { .. } => "user_input",
            _ => "",
        }
    }
}

/// Convert a `ProcessEvent` from the procmon crate into an `AgentEvent`.
pub fn process_event_to_agent(pe: crate::procmon::ProcessEvent) -> AgentEvent {
    match pe {
        crate::procmon::ProcessEvent::Spawn { pid, meta } => AgentEvent::ProcessSpawn { pid, meta },
        crate::procmon::ProcessEvent::Sample {
            pid,
            cpu_percent,
            rss_bytes,
            children,
        } => AgentEvent::ProcessSample {
            pid,
            cpu_percent,
            rss_bytes,
            children,
        },
        crate::procmon::ProcessEvent::Exit {
            pid,
            exit_code,
            wall_time,
            peak_rss,
        } => AgentEvent::ProcessExit {
            pid,
            exit_code,
            wall_time,
            peak_rss,
        },
    }
}
