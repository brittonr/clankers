//! Green session ledger DTOs and deterministic replay projection.
//!
//! The ledger core stores model-visible history and safe turn metadata without
//! runtime identifiers, wall-clock construction, desktop storage, daemon frames,
//! or runtime-specific error types.

use std::collections::BTreeMap;

use clanker_message::Content;
use clanker_message::Usage;
use clankers_engine::EngineMessage;
use clankers_engine::EngineMessageRole;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

pub type SessionLedgerPromptId = String;
pub type SessionLedgerSessionId = String;
pub type SessionLedgerMetadata = BTreeMap<String, String>;

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
pub struct SessionLedgerMessage<PromptId = SessionLedgerPromptId> {
    pub prompt_id: Option<PromptId>,
    pub role: SessionLedgerRole,
    pub content: Vec<Content>,
}

impl<PromptId> SessionLedgerMessage<PromptId> {
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
pub struct SessionLedgerSummary<PromptId = SessionLedgerPromptId> {
    pub prompt_id: Option<PromptId>,
    pub text: String,
}

/// Usage attached to a prompt turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLedgerUsage<PromptId = SessionLedgerPromptId> {
    pub prompt_id: Option<PromptId>,
    pub usage: Usage,
}

/// Safe, host-facing receipt metadata for a prompt turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionLedgerReceipt<PromptId = SessionLedgerPromptId, Metadata = SessionLedgerMetadata> {
    pub prompt_id: Option<PromptId>,
    pub status: String,
    pub metadata: Metadata,
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
pub enum SessionLedgerEntry<PromptId = SessionLedgerPromptId, Metadata = SessionLedgerMetadata> {
    Message {
        message: SessionLedgerMessage<PromptId>,
    },
    Summary {
        summary: SessionLedgerSummary<PromptId>,
    },
    Usage {
        usage: SessionLedgerUsage<PromptId>,
    },
    Receipt {
        receipt: SessionLedgerReceipt<PromptId, Metadata>,
    },
    Unsupported {
        unsupported: SessionLedgerUnsupported,
    },
}

impl<PromptId, Metadata> SessionLedgerEntry<PromptId, Metadata> {
    #[must_use]
    pub fn message(message: SessionLedgerMessage<PromptId>) -> Self {
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
    pub fn receipt(prompt_id: PromptId, status: impl Into<String>, metadata: Metadata) -> Self {
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

/// Neutral session record owned by a host session store. The green core does not
/// allocate IDs or timestamps; hosts pass in their own deterministic session ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLedgerRecord<
    SessionId = SessionLedgerSessionId,
    PromptId = SessionLedgerPromptId,
    Metadata = SessionLedgerMetadata,
> {
    pub session_id: SessionId,
    pub entries: Vec<SessionLedgerEntry<PromptId, Metadata>>,
}

impl<SessionId, PromptId, Metadata> SessionLedgerRecord<SessionId, PromptId, Metadata> {
    #[must_use]
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            entries: Vec::new(),
        }
    }

    pub fn replay(&self) -> Result<SessionLedgerReplay, SessionLedgerError> {
        replay_ledger_entries(&self.entries)
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionLedgerError {
    #[error("unsupported session ledger entry: {reason}")]
    Unsupported { reason: String },
}

impl SessionLedgerError {
    #[must_use]
    pub fn unsupported(reason: impl Into<String>) -> Self {
        Self::Unsupported { reason: reason.into() }
    }

    #[must_use]
    pub fn safe_message(&self) -> String {
        self.to_string()
    }
}

pub fn replay_ledger_entries<PromptId, Metadata>(
    entries: &[SessionLedgerEntry<PromptId, Metadata>],
) -> Result<SessionLedgerReplay, SessionLedgerError> {
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
                return Err(SessionLedgerError::unsupported(unsupported.reason.clone()));
            }
        }
    }

    Ok(SessionLedgerReplay { messages, metadata })
}

#[must_use]
pub fn ledger_messages_from_engine_messages<PromptId>(
    messages: &[EngineMessage],
) -> Vec<SessionLedgerMessage<PromptId>> {
    messages.iter().map(ledger_message_from_engine_message).collect()
}

#[must_use]
pub fn ledger_entries_from_engine_messages<PromptId, Metadata>(
    messages: &[EngineMessage],
) -> Vec<SessionLedgerEntry<PromptId, Metadata>> {
    messages
        .iter()
        .map(|message| SessionLedgerEntry::message(ledger_message_from_engine_message(message)))
        .collect()
}

#[must_use]
pub fn ledger_message_from_engine_message<PromptId>(message: &EngineMessage) -> SessionLedgerMessage<PromptId> {
    SessionLedgerMessage {
        prompt_id: None,
        role: ledger_role_from_engine_role(message.role.clone()),
        content: message.content.clone(),
    }
}

#[must_use]
pub fn engine_messages_from_ledger_messages<PromptId>(
    messages: &[SessionLedgerMessage<PromptId>],
) -> Vec<EngineMessage> {
    messages.iter().map(engine_message_from_ledger_message).collect()
}

#[must_use]
pub fn engine_message_from_ledger_message<PromptId>(message: &SessionLedgerMessage<PromptId>) -> EngineMessage {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_ledger_replay_is_deterministic_and_counts_non_message_entries() {
        let entries = vec![
            SessionLedgerEntry::<String, SessionLedgerMetadata>::summary("prior context"),
            SessionLedgerEntry::message(SessionLedgerMessage::text(SessionLedgerRole::User, "hello")),
            SessionLedgerEntry::usage("prompt-1".to_string(), Usage {
                input_tokens: 1,
                output_tokens: 2,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            }),
        ];

        let replay = replay_ledger_entries(&entries).unwrap();
        assert_eq!(replay.metadata.entry_count, 3);
        assert_eq!(replay.metadata.summary_count, 1);
        assert_eq!(replay.metadata.message_count, 1);
        assert_eq!(replay.metadata.usage_count, 1);
        assert_eq!(replay.messages[0].role, EngineMessageRole::User);
        assert_eq!(content_text_summary(&replay.messages[0].content[0]), "Session summary:\nprior context");
        assert_eq!(content_text_summary(&replay.messages[1].content[0]), "hello");
    }

    #[test]
    fn session_ledger_unsupported_entries_fail_closed_with_neutral_error() {
        let entries = vec![SessionLedgerEntry::<String, SessionLedgerMetadata>::unsupported(
            "desktop-only branch",
        )];
        let error = replay_ledger_entries(&entries).unwrap_err();
        assert_eq!(error, SessionLedgerError::unsupported("desktop-only branch"));
    }
}
