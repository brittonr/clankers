//! Clankers transcript compatibility records.
//!
//! This module owns desktop/session transcript shapes that carry Clankers
//! message IDs, wall-clock timestamps, shell execution records, branch and
//! compaction summaries, and custom history payloads. These types remain public
//! so existing session/provider/controller adapters can deserialize persisted
//! history, but they are not the generic embedded SDK message contract. Prefer
//! [`crate::Content`], [`crate::ToolDefinition`], [`crate::Usage`], streaming
//! events, and semantic events at reusable SDK boundaries.

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::Content;
use crate::StopReason;
use crate::Usage;

/// Unique Clankers transcript identifier for a stored message (8-char hex string by default).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(pub String);

impl MessageId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn generate() -> Self {
        Self(generate_id())
    }
}

impl From<String> for MessageId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for MessageId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for MessageId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for MessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Generate a random 8-character lowercase hexadecimal Clankers transcript ID.
pub fn generate_id() -> String {
    use rand::Rng;
    let mut rng = rand::rng();
    let bytes: [u8; 4] = rng.random();
    hex::encode(bytes)
}

/// A user message persisted in Clankers transcript storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessage {
    pub id: MessageId,
    pub content: Vec<Content>,
    pub timestamp: DateTime<Utc>,
}

/// An assistant response persisted in Clankers transcript storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub id: MessageId,
    pub content: Vec<Content>,
    pub model: String,
    pub usage: Usage,
    pub stop_reason: StopReason,
    pub timestamp: DateTime<Utc>,
}

/// Result of a tool execution persisted and sent back to the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultMessage {
    pub id: MessageId,
    pub call_id: String,
    pub tool_name: String,
    pub content: Vec<Content>,
    pub is_error: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
    pub timestamp: DateTime<Utc>,
}

/// Output from a bash tool execution (stored for desktop display/replay).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashExecutionMessage {
    pub id: MessageId,
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub timestamp: DateTime<Utc>,
}

/// Custom desktop transcript message with a kind discriminator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomMessage {
    pub id: MessageId,
    pub kind: String,
    pub data: Value,
    pub timestamp: DateTime<Utc>,
}

/// Summary of a Clankers conversation branch point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchSummaryMessage {
    pub id: MessageId,
    pub from_id: MessageId,
    pub summary: String,
    pub timestamp: DateTime<Utc>,
}

/// Summary after Clankers context compaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionSummaryMessage {
    pub id: MessageId,
    pub compacted_ids: Vec<MessageId>,
    pub summary: String,
    pub tokens_saved: usize,
    pub timestamp: DateTime<Utc>,
}

/// Union of all Clankers transcript message types.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AgentMessage {
    User(UserMessage),
    Assistant(AssistantMessage),
    ToolResult(ToolResultMessage),
    BashExecution(BashExecutionMessage),
    Custom(CustomMessage),
    BranchSummary(BranchSummaryMessage),
    CompactionSummary(CompactionSummaryMessage),
}

impl AgentMessage {
    /// Extract the message ID from any transcript variant.
    pub fn id(&self) -> &MessageId {
        match self {
            Self::User(m) => &m.id,
            Self::Assistant(m) => &m.id,
            Self::ToolResult(m) => &m.id,
            Self::BashExecution(m) => &m.id,
            Self::Custom(m) => &m.id,
            Self::BranchSummary(m) => &m.id,
            Self::CompactionSummary(m) => &m.id,
        }
    }

    /// Extract the persisted timestamp from any transcript variant.
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::User(m) => m.timestamp,
            Self::Assistant(m) => m.timestamp,
            Self::ToolResult(m) => m.timestamp,
            Self::BashExecution(m) => m.timestamp,
            Self::Custom(m) => m.timestamp,
            Self::CompactionSummary(m) => m.timestamp,
            Self::BranchSummary(m) => m.timestamp,
        }
    }

    /// Returns true if this is a user message.
    pub fn is_user(&self) -> bool {
        matches!(self, Self::User(_))
    }

    /// Returns true if this is an assistant message.
    pub fn is_assistant(&self) -> bool {
        matches!(self, Self::Assistant(_))
    }

    /// Returns the role string used by Clankers compatibility adapters.
    pub fn role(&self) -> &'static str {
        match self {
            Self::User(_) => "user",
            Self::Assistant(_) => "assistant",
            Self::ToolResult(_) => "user",
            Self::BashExecution(_) => "user",
            Self::Custom(_) => "user",
            Self::BranchSummary(_) => "user",
            Self::CompactionSummary(_) => "user",
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use serde_json::json;

    use super::*;

    fn fixed_timestamp() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 3, 12, 34, 56).single().expect("fixture timestamp should be valid")
    }

    fn make_user_msg() -> AgentMessage {
        AgentMessage::User(UserMessage {
            id: MessageId::new("user-1"),
            content: vec![Content::Text {
                text: "Hello".to_string(),
            }],
            timestamp: Utc::now(),
        })
    }

    fn make_assistant_msg() -> AgentMessage {
        AgentMessage::Assistant(AssistantMessage {
            id: MessageId::new("asst-1"),
            content: vec![Content::Text { text: "Hi".to_string() }],
            model: "test-model".to_string(),
            usage: Usage::default(),
            stop_reason: StopReason::Stop,
            timestamp: Utc::now(),
        })
    }

    fn make_tool_result_msg() -> AgentMessage {
        AgentMessage::ToolResult(ToolResultMessage {
            id: MessageId::new("tool-1"),
            call_id: "call_1".to_string(),
            tool_name: "bash".to_string(),
            content: vec![Content::Text {
                text: "output".to_string(),
            }],
            is_error: false,
            details: None,
            timestamp: Utc::now(),
        })
    }

    #[test]
    fn message_id_new() {
        let id = MessageId::new("test-id");
        assert_eq!(id.as_ref(), "test-id");
        assert_eq!(id.to_string(), "test-id");
    }

    #[test]
    fn message_id_from_string() {
        let id: MessageId = "hello".into();
        assert_eq!(id.0, "hello");
    }

    #[test]
    fn message_id_generate_unique() {
        let id1 = MessageId::generate();
        let id2 = MessageId::generate();
        assert_ne!(id1, id2);
    }

    #[test]
    fn message_id_equality() {
        let id1 = MessageId::new("same");
        let id2 = MessageId::new("same");
        assert_eq!(id1, id2);
    }

    #[test]
    fn agent_message_id() {
        let msg = make_user_msg();
        assert_eq!(msg.id().as_ref(), "user-1");

        let msg = make_assistant_msg();
        assert_eq!(msg.id().as_ref(), "asst-1");

        let msg = make_tool_result_msg();
        assert_eq!(msg.id().as_ref(), "tool-1");
    }

    #[test]
    fn agent_message_is_user() {
        assert!(make_user_msg().is_user());
        assert!(!make_assistant_msg().is_user());
        assert!(!make_tool_result_msg().is_user());
    }

    #[test]
    fn agent_message_is_assistant() {
        assert!(!make_user_msg().is_assistant());
        assert!(make_assistant_msg().is_assistant());
        assert!(!make_tool_result_msg().is_assistant());
    }

    #[test]
    fn agent_message_role() {
        assert_eq!(make_user_msg().role(), "user");
        assert_eq!(make_assistant_msg().role(), "assistant");
        assert_eq!(make_tool_result_msg().role(), "user");
    }

    #[test]
    fn agent_message_timestamp() {
        let before = Utc::now();
        let msg = make_user_msg();
        let after = Utc::now();
        let ts = msg.timestamp();
        assert!(ts >= before && ts <= after);
    }

    #[test]
    fn agent_message_roundtrip() {
        let msg = make_user_msg();
        let json = serde_json::to_string(&msg).expect("message should serialize to JSON");
        let parsed: AgentMessage = serde_json::from_str(&json).expect("JSON should deserialize to message");
        assert!(parsed.is_user());
        assert_eq!(parsed.id().as_ref(), "user-1");
    }

    #[test]
    fn usage_total_tokens() {
        let usage = Usage {
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        };
        assert_eq!(usage.total_tokens(), 150);
    }

    #[test]
    fn usage_default() {
        let usage = Usage::default();
        assert_eq!(usage.total_tokens(), 0);
    }

    #[test]
    fn bash_execution_message_role() {
        let msg = AgentMessage::BashExecution(BashExecutionMessage {
            id: MessageId::new("bash-1"),
            command: "ls".to_string(),
            stdout: "file.txt".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            timestamp: Utc::now(),
        });
        assert_eq!(msg.role(), "user");
        assert!(!msg.is_user());
        assert!(!msg.is_assistant());
    }

    #[test]
    fn custom_message_role() {
        let msg = AgentMessage::Custom(CustomMessage {
            id: MessageId::new("custom-1"),
            kind: "test".to_string(),
            data: json!({"key": "val"}),
            timestamp: Utc::now(),
        });
        assert_eq!(msg.role(), "user");
    }

    #[test]
    fn compaction_summary_role() {
        let msg = AgentMessage::CompactionSummary(CompactionSummaryMessage {
            id: MessageId::new("compact-1"),
            compacted_ids: vec![MessageId::new("m1"), MessageId::new("m2")],
            summary: "Summary of conversation".to_string(),
            tokens_saved: 500,
            timestamp: Utc::now(),
        });
        assert_eq!(msg.role(), "user");
        assert_eq!(msg.id().as_ref(), "compact-1");
    }

    #[test]
    fn branch_summary_role() {
        let msg = AgentMessage::BranchSummary(BranchSummaryMessage {
            id: MessageId::new("branch-1"),
            from_id: MessageId::new("m1"),
            summary: "Branched here".to_string(),
            timestamp: Utc::now(),
        });
        assert_eq!(msg.role(), "user");
        assert_eq!(msg.id().as_ref(), "branch-1");
    }

    #[test]
    fn transcript_internal_serialization_fixture_survives() {
        let fixture = AgentMessage::CompactionSummary(CompactionSummaryMessage {
            id: MessageId::new("compact-1"),
            compacted_ids: vec![MessageId::new("user-1"), MessageId::new("assistant-1")],
            summary: "compacted state".to_string(),
            tokens_saved: 42,
            timestamp: fixed_timestamp(),
        });

        let value = serde_json::to_value(&fixture).expect("serialize fixture");
        let expected = json!({
            "type": "CompactionSummary",
            "id": "compact-1",
            "compacted_ids": ["user-1", "assistant-1"],
            "summary": "compacted state",
            "tokens_saved": 42,
            "timestamp": "2026-06-03T12:34:56Z",
        });
        assert_eq!(value, expected);

        let restored: AgentMessage = serde_json::from_value(expected).expect("deserialize fixture");
        assert_eq!(restored.role(), "user");
        assert_eq!(restored.id().as_ref(), "compact-1");
    }
}
