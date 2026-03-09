//! Conversation blocks — Warp-style grouped prompt/response units

use chrono::DateTime;
use chrono::Local;

use crate::tui::app::DisplayMessage;
use crate::tui::app::MessageRole;

/// A single conversation block: one user turn + the full agent response
#[derive(Debug, Clone)]
pub struct ConversationBlock {
    /// Unique block ID (monotonic counter)
    pub id: usize,
    /// Timestamp when the block was created
    pub timestamp: DateTime<Local>,
    /// The user's input prompt
    pub prompt: String,
    /// All response messages (thinking, assistant text, tool calls/results)
    pub responses: Vec<DisplayMessage>,
    /// Whether the block is collapsed (shows only a summary line)
    pub collapsed: bool,
    /// Whether this block is still streaming (not yet complete)
    pub streaming: bool,
    /// Optional error that terminated this block
    pub error: Option<String>,
    /// Token usage for this block
    pub tokens: usize,

    // ── Branching ────────────────────────────────────
    /// ID of the parent block (the block before this one in the conversation).
    /// `None` means this is a root block (first in a conversation).
    pub parent_block_id: Option<usize>,
    /// The number of agent messages at the point just before this block started.
    /// Used to truncate the agent's history when branching.
    pub agent_msg_checkpoint: usize,
}

impl ConversationBlock {
    pub fn new(id: usize, prompt: String) -> Self {
        Self {
            id,
            timestamp: Local::now(),
            prompt,
            responses: Vec::new(),
            collapsed: false,
            streaming: true,
            error: None,
            tokens: 0,
            parent_block_id: None,
            agent_msg_checkpoint: 0,
        }
    }

    /// One-line summary for collapsed view
    pub fn summary(&self) -> String {
        let status = if self.streaming {
            "…"
        } else if self.error.is_some() {
            "✗"
        } else {
            "✓"
        };
        let tool_count = self.responses.iter().filter(|m| m.role == MessageRole::ToolCall).count();
        let text_preview: String = self
            .responses
            .iter()
            .filter(|m| m.role == MessageRole::Assistant)
            .find_map(|m| m.content.lines().next())
            .unwrap_or("...")
            .chars()
            .take(60)
            .collect();

        if tool_count > 0 {
            format!("{} {} ({} tools) — {}", status, self.prompt_preview(), tool_count, text_preview)
        } else {
            format!("{} {} — {}", status, self.prompt_preview(), text_preview)
        }
    }

    fn prompt_preview(&self) -> String {
        self.prompt.lines().next().unwrap_or("").chars().take(40).collect()
    }

    /// Toggle collapsed state
    pub fn toggle_collapse(&mut self) {
        self.collapsed = !self.collapsed;
    }
}

/// Top-level entry in the conversation: either a block or a standalone system message
#[derive(Debug, Clone)]
pub enum BlockEntry {
    /// A full prompt→response block
    Conversation(ConversationBlock),
    /// A standalone system message (not part of a prompt)
    System(DisplayMessage),
}
