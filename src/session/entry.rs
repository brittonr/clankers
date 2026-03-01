//! Session entry types

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::provider::message::AgentMessage;
use crate::provider::message::MessageId;

/// Every entry in the session JSONL file
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SessionEntry {
    /// Session header — first entry, written once
    Header(HeaderEntry),
    /// A message in the conversation
    Message(MessageEntry),
    /// Context compaction summary (replaces a range of messages)
    Compaction(CompactionEntry),
    /// Branch point — conversation forked here
    Branch(BranchEntry),
    /// Custom entry with a kind discriminator
    Custom(CustomEntry),
    /// User-applied label to a message
    Label(LabelEntry),
    /// Model was changed mid-session
    ModelChange(ModelChangeEntry),
    /// Session was resumed
    Resume(ResumeEntry),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeaderEntry {
    pub session_id: String,
    pub created_at: DateTime<Utc>,
    pub cwd: String,
    pub model: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree_branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageEntry {
    pub id: MessageId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<MessageId>,
    pub message: AgentMessage,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionEntry {
    pub id: MessageId,
    pub compacted_range: Vec<MessageId>,
    pub summary: String,
    pub tokens_before: usize,
    pub tokens_after: usize,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchEntry {
    pub id: MessageId,
    pub from_message_id: MessageId,
    pub reason: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomEntry {
    pub id: MessageId,
    pub kind: String,
    pub data: Value,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelEntry {
    pub id: MessageId,
    pub target_message_id: MessageId,
    pub label: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelChangeEntry {
    pub id: MessageId,
    pub from_model: String,
    pub to_model: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResumeEntry {
    pub id: MessageId,
    pub resumed_at: DateTime<Utc>,
    pub from_entry_id: MessageId,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::message::Content;
    use crate::provider::message::UserMessage;

    #[test]
    fn test_header_entry_serialize() {
        let header = HeaderEntry {
            session_id: "test123".to_string(),
            created_at: Utc::now(),
            cwd: "/tmp/test".to_string(),
            model: "claude-sonnet".to_string(),
            version: "1.0.0".to_string(),
            agent: Some("worker".to_string()),
            parent_session_id: None,
            worktree_path: None,
            worktree_branch: None,
        };
        let entry = SessionEntry::Header(header);
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"type\":\"Header\""));
        assert!(json.contains("test123"));
    }

    #[test]
    fn test_message_entry_roundtrip() {
        let id = MessageId::new("test-id");
        let msg = MessageEntry {
            id: id.clone(),
            parent_id: None,
            message: AgentMessage::User(UserMessage {
                id: id.clone(),
                content: vec![Content::Text {
                    text: "Hello".to_string(),
                }],
                timestamp: Utc::now(),
            }),
            timestamp: Utc::now(),
        };
        let entry = SessionEntry::Message(msg.clone());
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: SessionEntry = serde_json::from_str(&json).unwrap();

        match parsed {
            SessionEntry::Message(parsed_msg) => {
                assert_eq!(parsed_msg.id, msg.id);
                assert_eq!(parsed_msg.parent_id, msg.parent_id);
            }
            _ => panic!("Expected Message entry"),
        }
    }

    #[test]
    fn test_compaction_entry() {
        let compaction = CompactionEntry {
            id: MessageId::new("test-id"),
            compacted_range: vec![MessageId::new("test-id"), MessageId::new("test-id")],
            summary: "Compacted 2 messages".to_string(),
            tokens_before: 1000,
            tokens_after: 100,
            timestamp: Utc::now(),
        };
        let entry = SessionEntry::Compaction(compaction);
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"type\":\"Compaction\""));
        assert!(json.contains("\"tokens_before\":1000"));
    }

    #[test]
    fn test_branch_entry() {
        let branch = BranchEntry {
            id: MessageId::new("test-id"),
            from_message_id: MessageId::new("test-id"),
            reason: "User requested alternate approach".to_string(),
            timestamp: Utc::now(),
        };
        let entry = SessionEntry::Branch(branch);
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"type\":\"Branch\""));
        assert!(json.contains("alternate approach"));
    }

    #[test]
    fn test_label_entry() {
        let label = LabelEntry {
            id: MessageId::new("test-id"),
            target_message_id: MessageId::new("test-id"),
            label: "important".to_string(),
            timestamp: Utc::now(),
        };
        let entry = SessionEntry::Label(label);
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"type\":\"Label\""));
        assert!(json.contains("important"));
    }

    #[test]
    fn test_model_change_entry() {
        let change = ModelChangeEntry {
            id: MessageId::new("test-id"),
            from_model: "claude-haiku".to_string(),
            to_model: "claude-sonnet".to_string(),
            timestamp: Utc::now(),
        };
        let entry = SessionEntry::ModelChange(change);
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"type\":\"ModelChange\""));
        assert!(json.contains("claude-haiku"));
        assert!(json.contains("claude-sonnet"));
    }

    #[test]
    fn test_custom_entry() {
        let custom = CustomEntry {
            id: MessageId::new("test-id"),
            kind: "test_event".to_string(),
            data: serde_json::json!({"key": "value"}),
            timestamp: Utc::now(),
        };
        let entry = SessionEntry::Custom(custom);
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"type\":\"Custom\""));
        assert!(json.contains("test_event"));
    }
}
