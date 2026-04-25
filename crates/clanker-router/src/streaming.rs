//! Streaming response types for model completions.

pub use clanker_message::streaming::ContentDelta;
pub use clanker_message::streaming::MessageMetadata;
use serde::Deserialize;
use serde::Serialize;

/// Events streamed during model completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StreamEvent {
    /// Message stream started.
    MessageStart { message: MessageMetadata },
    /// Content block started.
    ContentBlockStart { index: usize, content_block: ContentBlock },
    /// Content block delta (incremental update).
    ContentBlockDelta { index: usize, delta: ContentDelta },
    /// Content block completed.
    ContentBlockStop { index: usize },
    /// Message-level delta (stop reason + final usage).
    MessageDelta {
        stop_reason: Option<String>,
        usage: crate::provider::Usage,
    },
    /// Message stream completed.
    MessageStop,
    /// Error occurred during streaming.
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

/// A content block within a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    /// Text content.
    #[serde(rename = "text")]
    Text { text: String },
    /// Extended thinking content.
    #[serde(rename = "thinking")]
    Thinking {
        thinking: String,
        /// Opaque signature returned by Anthropic; must be echoed back verbatim.
        #[serde(default, skip_serializing_if = "String::is_empty")]
        signature: String,
    },
    /// Tool use request.
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

/// Convert a router `ContentBlock` into a clanker message `Content`.
impl From<ContentBlock> for clanker_message::Content {
    fn from(block: ContentBlock) -> Self {
        match block {
            ContentBlock::Text { text } => clanker_message::Content::Text { text },
            ContentBlock::Thinking { thinking, signature } => {
                clanker_message::Content::Thinking { thinking, signature }
            }
            ContentBlock::ToolUse { id, name, input } => clanker_message::Content::ToolUse { id, name, input },
        }
    }
}

/// Convert clanker message `Content` back into a router `ContentBlock`.
impl From<clanker_message::Content> for ContentBlock {
    fn from(content: clanker_message::Content) -> Self {
        match content {
            clanker_message::Content::Text { text } => ContentBlock::Text { text },
            clanker_message::Content::Thinking { thinking, signature } => {
                ContentBlock::Thinking { thinking, signature }
            }
            clanker_message::Content::ToolUse { id, name, input } => ContentBlock::ToolUse { id, name, input },
            // These variants only appear in user/tool messages, never in LLM responses.
            clanker_message::Content::Image { .. } | clanker_message::Content::ToolResult { .. } => {
                ContentBlock::Text { text: String::new() }
            }
        }
    }
}

/// Convert a router `StreamEvent` into a clanker message `StreamEvent`.
impl From<StreamEvent> for clanker_message::StreamEvent {
    fn from(event: StreamEvent) -> Self {
        match event {
            StreamEvent::MessageStart { message } => clanker_message::StreamEvent::MessageStart { message },
            StreamEvent::ContentBlockStart { index, content_block } => {
                clanker_message::StreamEvent::ContentBlockStart {
                    index,
                    content_block: content_block.into(),
                }
            }
            StreamEvent::ContentBlockDelta { index, delta } => {
                clanker_message::StreamEvent::ContentBlockDelta { index, delta }
            }
            StreamEvent::ContentBlockStop { index } => clanker_message::StreamEvent::ContentBlockStop { index },
            StreamEvent::MessageDelta { stop_reason, usage } => {
                clanker_message::StreamEvent::MessageDelta { stop_reason, usage }
            }
            StreamEvent::MessageStop => clanker_message::StreamEvent::MessageStop,
            StreamEvent::Error { error } => clanker_message::StreamEvent::Error { error },
        }
    }
}

/// Convert a clanker message `StreamEvent` into a router `StreamEvent`.
impl From<clanker_message::StreamEvent> for StreamEvent {
    fn from(event: clanker_message::StreamEvent) -> Self {
        match event {
            clanker_message::StreamEvent::MessageStart { message } => StreamEvent::MessageStart { message },
            clanker_message::StreamEvent::ContentBlockStart { index, content_block } => {
                StreamEvent::ContentBlockStart {
                    index,
                    content_block: content_block.into(),
                }
            }
            clanker_message::StreamEvent::ContentBlockDelta { index, delta } => {
                StreamEvent::ContentBlockDelta { index, delta }
            }
            clanker_message::StreamEvent::ContentBlockStop { index } => StreamEvent::ContentBlockStop { index },
            clanker_message::StreamEvent::MessageDelta { stop_reason, usage } => {
                StreamEvent::MessageDelta { stop_reason, usage }
            }
            clanker_message::StreamEvent::MessageStop => StreamEvent::MessageStop,
            clanker_message::StreamEvent::Error { error } => StreamEvent::Error { error },
        }
    }
}
