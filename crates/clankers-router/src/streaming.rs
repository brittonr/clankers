//! Streaming response types for model completions

use serde::Deserialize;
use serde::Serialize;

/// Events streamed during model completion
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StreamEvent {
    /// Message stream started
    MessageStart { message: MessageMetadata },
    /// Content block started
    ContentBlockStart { index: usize, content_block: ContentBlock },
    /// Content block delta (incremental update)
    ContentBlockDelta { index: usize, delta: ContentDelta },
    /// Content block completed
    ContentBlockStop { index: usize },
    /// Message-level delta (stop reason + final usage)
    MessageDelta {
        stop_reason: Option<String>,
        usage: crate::provider::Usage,
    },
    /// Message stream completed
    MessageStop,
    /// Error occurred during streaming
    Error { error: String },
}

/// Metadata about a streaming message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMetadata {
    pub id: String,
    pub model: String,
    pub role: String,
}

/// A content block within a message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    /// Text content
    #[serde(rename = "text")]
    Text { text: String },
    /// Extended thinking content
    #[serde(rename = "thinking")]
    Thinking { thinking: String },
    /// Tool use request
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

/// Incremental delta for content blocks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentDelta {
    /// Text delta
    TextDelta { text: String },
    /// Thinking delta
    ThinkingDelta { thinking: String },
    /// Input JSON delta for tool use
    InputJsonDelta { partial_json: String },
}
