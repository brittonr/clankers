//! Desktop session compatibility adapters for neutral ledger DTOs.
//!
//! `clankers-session` remains the desktop transcript store. This module is the
//! app-edge seam that projects persisted `AgentMessage` values into the neutral
//! ledger DTOs used by SDK/runtime resume paths.

use clanker_message::transcript::AgentMessage;
use clanker_message::Content;
use clankers_protocol::SerializedMessage;
use clankers_runtime::SessionLedgerEntry;
use clankers_runtime::SessionLedgerMessage;
use clankers_runtime::SessionLedgerRole;

/// Convert desktop transcript messages into neutral ledger entries.
///
/// Unsupported desktop-only metadata stays at this compatibility edge instead
/// of leaking into reusable SDK/session-store code.
pub(crate) fn desktop_messages_to_ledger_entries(messages: &[AgentMessage]) -> Vec<SessionLedgerEntry> {
    messages.iter().filter_map(desktop_message_to_ledger_entry).collect()
}

/// Convert desktop transcript messages to daemon seed messages through the
/// neutral ledger adapter. The current daemon seed protocol supports only
/// user/assistant text messages, so richer ledger entries remain adapter-owned.
pub(crate) fn desktop_messages_to_serialized_seed_messages(messages: &[AgentMessage]) -> Vec<SerializedMessage> {
    let entries = desktop_messages_to_ledger_entries(messages);
    let mut assistant_models = messages.iter().filter_map(serialized_assistant_model_from_desktop_message);
    entries
        .iter()
        .filter_map(|entry| {
            serialized_seed_from_ledger_entry(entry).map(|mut seed| {
                if seed.role == "assistant" {
                    seed.model = assistant_models.next();
                }
                seed
            })
        })
        .collect()
}

fn desktop_message_to_ledger_entry(message: &AgentMessage) -> Option<SessionLedgerEntry> {
    match message {
        AgentMessage::User(user) => Some(SessionLedgerEntry::message(SessionLedgerMessage {
            prompt_id: None,
            role: SessionLedgerRole::User,
            content: user.content.clone(),
        })),
        AgentMessage::Assistant(assistant) => Some(SessionLedgerEntry::message(SessionLedgerMessage {
            prompt_id: None,
            role: SessionLedgerRole::Assistant,
            content: assistant.content.clone(),
        })),
        AgentMessage::ToolResult(result) => Some(SessionLedgerEntry::message(SessionLedgerMessage {
            prompt_id: None,
            role: SessionLedgerRole::Tool,
            content: result.content.clone(),
        })),
        AgentMessage::BranchSummary(summary) => {
            Some(SessionLedgerEntry::summary(format!("Branch summary:\n{}", summary.summary)))
        }
        AgentMessage::CompactionSummary(summary) => {
            Some(SessionLedgerEntry::summary(format!("Compaction summary:\n{}", summary.summary)))
        }
        AgentMessage::BashExecution(_) | AgentMessage::Custom(_) => None,
    }
}

fn serialized_assistant_model_from_desktop_message(message: &AgentMessage) -> Option<String> {
    match message {
        AgentMessage::Assistant(assistant) if !text_content(&assistant.content).is_empty() => {
            Some(assistant.model.clone())
        }
        _ => None,
    }
}

fn serialized_seed_from_ledger_entry(entry: &SessionLedgerEntry) -> Option<SerializedMessage> {
    let SessionLedgerEntry::Message { message } = entry else {
        return None;
    };
    let (role, model) = match message.role {
        SessionLedgerRole::User => ("user", None),
        SessionLedgerRole::Assistant => ("assistant", None),
        SessionLedgerRole::Tool => return None,
    };
    let content = text_content(&message.content);
    if content.is_empty() {
        return None;
    }
    Some(SerializedMessage {
        role: role.to_string(),
        content,
        model,
        timestamp: None,
    })
}

fn text_content(content: &[Content]) -> String {
    content
        .iter()
        .filter_map(|part| match part {
            Content::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use clanker_message::transcript::AssistantMessage;
    use clanker_message::transcript::BranchSummaryMessage;
    use clanker_message::transcript::MessageId;
    use clanker_message::StopReason;
    use clanker_message::transcript::ToolResultMessage;
    use clanker_message::Usage;
    use clanker_message::transcript::UserMessage;

    use super::*;

    fn desktop_fixture() -> Vec<AgentMessage> {
        vec![
            AgentMessage::User(UserMessage {
                id: MessageId::new("user-1"),
                content: vec![Content::Text {
                    text: "hello".to_string(),
                }],
                timestamp: Utc::now(),
            }),
            AgentMessage::ToolResult(ToolResultMessage {
                id: MessageId::new("tool-1"),
                call_id: "call-1".to_string(),
                tool_name: "lookup".to_string(),
                content: vec![Content::Text {
                    text: "tool output".to_string(),
                }],
                is_error: false,
                details: None,
                timestamp: Utc::now(),
            }),
            AgentMessage::Assistant(AssistantMessage {
                id: MessageId::new("assistant-1"),
                content: vec![Content::Text {
                    text: "answer".to_string(),
                }],
                model: "fixture-model".to_string(),
                usage: Usage::default(),
                stop_reason: StopReason::Stop,
                timestamp: Utc::now(),
            }),
            AgentMessage::BranchSummary(BranchSummaryMessage {
                id: MessageId::new("branch-1"),
                from_id: MessageId::new("user-1"),
                summary: "branched context".to_string(),
                timestamp: Utc::now(),
            }),
        ]
    }

    #[test]
    fn desktop_messages_project_to_neutral_ledger_entries_at_app_edge() {
        let entries = desktop_messages_to_ledger_entries(&desktop_fixture());

        assert_eq!(entries.len(), 4);
        assert!(matches!(
            &entries[0],
            SessionLedgerEntry::Message { message }
                if message.role == SessionLedgerRole::User && message.text_summary() == "hello"
        ));
        assert!(matches!(
            &entries[1],
            SessionLedgerEntry::Message { message }
                if message.role == SessionLedgerRole::Tool && message.text_summary() == "tool output"
        ));
        assert!(matches!(
            &entries[2],
            SessionLedgerEntry::Message { message }
                if message.role == SessionLedgerRole::Assistant && message.text_summary() == "answer"
        ));
        assert!(matches!(
            &entries[3],
            SessionLedgerEntry::Summary { summary } if summary.text.contains("branched context")
        ));
    }

    #[test]
    fn daemon_seed_projection_uses_ledger_but_keeps_protocol_shape() {
        let seed = desktop_messages_to_serialized_seed_messages(&desktop_fixture());

        assert_eq!(seed.len(), 2);
        assert_eq!(seed[0].role, "user");
        assert_eq!(seed[0].content, "hello");
        assert_eq!(seed[1].role, "assistant");
        assert_eq!(seed[1].content, "answer");
        assert_eq!(seed[1].model.as_deref(), Some("fixture-model"));
    }
}
