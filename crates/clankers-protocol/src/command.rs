//! Client-to-daemon commands.

use serde::Deserialize;
use serde::Serialize;

use crate::types::ImageData;
use crate::types::SerializedMessage;

/// Commands sent from a client (TUI, CLI, etc.) to the daemon session.
// r[impl protocol.serde.command-externally-tagged]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SessionCommand {
    /// Send a prompt to the agent.
    Prompt { text: String, images: Vec<ImageData> },
    /// Cancel the current operation.
    Abort,
    /// Reset cancellation state (allow new prompts after abort).
    ResetCancel,
    /// Switch the active model.
    SetModel { model: String },
    /// Clear conversation history.
    ClearHistory,
    /// Truncate to N messages.
    TruncateMessages { count: usize },
    /// Set thinking level.
    SetThinkingLevel { level: String },
    /// Cycle thinking level.
    CycleThinkingLevel,
    /// Seed initial messages (session restore).
    SeedMessages { messages: Vec<SerializedMessage> },
    /// Replace the system prompt.
    SetSystemPrompt { prompt: String },
    /// Get the current system prompt.
    GetSystemPrompt,
    /// Switch account credentials.
    SwitchAccount { account: String },
    /// Update disabled tools.
    SetDisabledTools { tools: Vec<String> },
    /// Respond to a bash confirmation request.
    ConfirmBash { request_id: String, approved: bool },
    /// Respond to a todo action request.
    TodoResponse {
        request_id: String,
        response: serde_json::Value,
    },
    /// Execute a slash command (agent-side only).
    SlashCommand { command: String, args: String },
    /// Rewrite last user prompt and re-submit.
    RewriteAndPrompt { text: String },
    /// Compact conversation history.
    CompactHistory,
    /// Start a loop.
    StartLoop {
        iterations: u32,
        prompt: String,
        break_condition: Option<String>,
    },
    /// Stop the active loop.
    StopLoop,
    /// Set auto-test command.
    SetAutoTest {
        enabled: bool,
        command: Option<String>,
    },
    /// Request the tool list.
    GetToolList,
    /// Request session history replay (on attach).
    ReplayHistory,
    /// Query the session's active capabilities.
    GetCapabilities,
    /// Request the list of loaded plugins.
    GetPlugins,
    /// Update the session's active tool capabilities.
    ///
    /// `None` = remove user restrictions (restore to ceiling).
    /// `Some(patterns)` = restrict to these tool patterns.
    /// Rejected if the requested capabilities exceed the session's ceiling.
    SetCapabilities {
        capabilities: Option<Vec<String>>,
    },
    /// Graceful disconnect.
    Disconnect,
}
