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

// ── Tagged stream events (for multi-model dispatch) ─────────────────────

/// A stream event tagged with the model and provider that produced it.
///
/// Used by [`multi::MultiRequest`](crate::multi::MultiRequest) dispatch
/// to interleave events from multiple models on a single channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaggedStreamEvent {
    /// The model ID that produced this event.
    pub model: String,
    /// The provider name that served this model.
    pub provider: String,
    /// The underlying stream event.
    pub event: StreamEvent,
}

impl TaggedStreamEvent {
    /// Wrap a bare event with model/provider tags.
    pub fn new(model: impl Into<String>, provider: impl Into<String>, event: StreamEvent) -> Self {
        Self {
            model: model.into(),
            provider: provider.into(),
            event,
        }
    }

    /// Unwrap into the inner `StreamEvent`, discarding the tags.
    pub fn into_inner(self) -> StreamEvent {
        self.event
    }
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
    Thinking {
        thinking: String,
        /// Opaque signature returned by Anthropic; must be echoed back verbatim.
        #[serde(default, skip_serializing_if = "String::is_empty")]
        signature: String,
    },
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
    /// Thinking signature delta (Anthropic; must be echoed back verbatim)
    SignatureDelta { signature: String },
}
