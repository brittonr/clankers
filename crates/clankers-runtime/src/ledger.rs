//! Neutral session ledger DTOs and replay projection for embedded resume.

use chrono::DateTime;
use chrono::Utc;
use clanker_message::Content;
use clanker_message::Usage;
use clankers_engine::EngineMessage;
use clankers_engine::EngineMessageRole;
use serde::Deserialize;
use serde::Serialize;

use crate::EventMetadata;
use crate::PromptId;
use crate::RuntimeError;
use crate::SessionId;

/// Neutral conversation role stored by the reusable session ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionLedgerRole {
    User,
    Assistant,
    Tool,
}

/// A model-visible ledger message. This is intentionally engine/session neutral:
/// hosts can persist it without importing agent, daemon, TUI, JSONL, or database DTOs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLedgerMessage {
    pub prompt_id: Option<PromptId>,
    pub role: SessionLedgerRole,
    pub content: Vec<Content>,
}

impl SessionLedgerMessage {
    #[must_use]
    pub fn text(role: SessionLedgerRole, text: impl Into<String>) -> Self {
        Self {
            prompt_id: None,
            role,
            content: vec![Content::Text { text: text.into() }],
        }
    }

    #[must_use]
    pub fn with_prompt_id(mut self, prompt_id: PromptId) -> Self {
        self.prompt_id = Some(prompt_id);
        self
    }

    #[must_use]
    pub fn text_summary(&self) -> String {
        self.content.iter().map(content_text_summary).collect::<Vec<_>>().join("\n")
    }
}

/// Host-provided summary context restored into follow-up prompts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionLedgerSummary {
    pub prompt_id: Option<PromptId>,
    pub text: String,
}

/// Usage attached to a prompt turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLedgerUsage {
    pub prompt_id: Option<PromptId>,
    pub usage: Usage,
}

/// Safe, host-facing receipt metadata for a prompt turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionLedgerReceipt {
    pub prompt_id: Option<PromptId>,
    pub status: String,
    pub metadata: EventMetadata,
}

/// Unsupported adapter-owned content marker. Replaying this fails closed instead
/// of silently dropping shell-only state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionLedgerUnsupported {
    pub reason: String,
}

/// Ordered neutral session ledger entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionLedgerEntry {
    Message { message: SessionLedgerMessage },
    Summary { summary: SessionLedgerSummary },
    Usage { usage: SessionLedgerUsage },
    Receipt { receipt: SessionLedgerReceipt },
    Unsupported { unsupported: SessionLedgerUnsupported },
}

impl SessionLedgerEntry {
    #[must_use]
    pub fn message(message: SessionLedgerMessage) -> Self {
        Self::Message { message }
    }

    #[must_use]
    pub fn summary(text: impl Into<String>) -> Self {
        Self::Summary {
            summary: SessionLedgerSummary {
                prompt_id: None,
                text: text.into(),
            },
        }
    }

    #[must_use]
    pub fn usage(prompt_id: PromptId, usage: Usage) -> Self {
        Self::Usage {
            usage: SessionLedgerUsage {
                prompt_id: Some(prompt_id),
                usage,
            },
        }
    }

    #[must_use]
    pub fn receipt(prompt_id: PromptId, status: impl Into<String>, metadata: EventMetadata) -> Self {
        Self::Receipt {
            receipt: SessionLedgerReceipt {
                prompt_id: Some(prompt_id),
                status: status.into(),
                metadata,
            },
        }
    }

    #[must_use]
    pub fn unsupported(reason: impl Into<String>) -> Self {
        Self::Unsupported {
            unsupported: SessionLedgerUnsupported { reason: reason.into() },
        }
    }
}

/// Replay metadata exposed to hosts without engine internals.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionLedgerReplayMetadata {
    pub entry_count: usize,
    pub message_count: usize,
    pub summary_count: usize,
    pub usage_count: usize,
    pub receipt_count: usize,
}

/// Stable projection from ledger entries to engine history.
#[derive(Debug, Clone)]
pub struct SessionLedgerReplay {
    pub messages: Vec<EngineMessage>,
    pub metadata: SessionLedgerReplayMetadata,
}

/// Neutral session record owned by a host session store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLedgerRecord {
    pub session_id: SessionId,
    pub created_at: DateTime<Utc>,
    pub entries: Vec<SessionLedgerEntry>,
}

impl SessionLedgerRecord {
    #[must_use]
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            created_at: Utc::now(),
            entries: Vec::new(),
        }
    }

    pub fn replay(&self) -> Result<SessionLedgerReplay, RuntimeError> {
        replay_ledger_entries(&self.entries)
    }
}

pub fn replay_ledger_entries(entries: &[SessionLedgerEntry]) -> Result<SessionLedgerReplay, RuntimeError> {
    let mut messages = Vec::new();
    let mut metadata = SessionLedgerReplayMetadata {
        entry_count: entries.len(),
        ..SessionLedgerReplayMetadata::default()
    };

    for entry in entries {
        match entry {
            SessionLedgerEntry::Message { message } => {
                metadata.message_count += 1;
                messages.push(engine_message_from_ledger_message(message));
            }
            SessionLedgerEntry::Summary { summary } => {
                metadata.summary_count += 1;
                messages.push(EngineMessage {
                    role: EngineMessageRole::User,
                    content: vec![Content::Text {
                        text: format!("Session summary:\n{}", summary.text),
                    }],
                });
            }
            SessionLedgerEntry::Usage { .. } => metadata.usage_count += 1,
            SessionLedgerEntry::Receipt { .. } => metadata.receipt_count += 1,
            SessionLedgerEntry::Unsupported { unsupported } => {
                return Err(RuntimeError::SessionUnsupported(unsupported.reason.clone()));
            }
        }
    }

    Ok(SessionLedgerReplay { messages, metadata })
}

#[must_use]
pub fn ledger_messages_from_engine_messages(messages: &[EngineMessage]) -> Vec<SessionLedgerMessage> {
    messages.iter().map(ledger_message_from_engine_message).collect()
}

#[must_use]
pub fn ledger_entries_from_engine_messages(messages: &[EngineMessage]) -> Vec<SessionLedgerEntry> {
    messages
        .iter()
        .map(|message| SessionLedgerEntry::message(ledger_message_from_engine_message(message)))
        .collect()
}

fn ledger_message_from_engine_message(message: &EngineMessage) -> SessionLedgerMessage {
    SessionLedgerMessage {
        prompt_id: None,
        role: ledger_role_from_engine_role(message.role.clone()),
        content: message.content.clone(),
    }
}

fn engine_message_from_ledger_message(message: &SessionLedgerMessage) -> EngineMessage {
    EngineMessage {
        role: engine_role_from_ledger_role(message.role),
        content: message.content.clone(),
    }
}

fn engine_role_from_ledger_role(role: SessionLedgerRole) -> EngineMessageRole {
    match role {
        SessionLedgerRole::User => EngineMessageRole::User,
        SessionLedgerRole::Assistant => EngineMessageRole::Assistant,
        SessionLedgerRole::Tool => EngineMessageRole::Tool,
    }
}

fn ledger_role_from_engine_role(role: EngineMessageRole) -> SessionLedgerRole {
    match role {
        EngineMessageRole::User => SessionLedgerRole::User,
        EngineMessageRole::Assistant => SessionLedgerRole::Assistant,
        EngineMessageRole::Tool => SessionLedgerRole::Tool,
    }
}

fn content_text_summary(content: &Content) -> String {
    match content {
        Content::Text { text } => text.clone(),
        Content::Thinking { thinking, .. } => thinking.clone(),
        Content::ToolUse { name, .. } => format!("tool-use:{name}"),
        Content::ToolResult { content, .. } => content.iter().map(content_text_summary).collect::<Vec<_>>().join("\n"),
        Content::Image { .. } => "[image]".to_string(),
    }
}
