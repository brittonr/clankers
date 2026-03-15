//! Streaming response types
//!
//! Re-exports identical types from `clanker_router::streaming` and defines
//! `StreamEvent` locally because `ContentBlockStart` uses the richer
//! [`Content`](super::message::Content) type (which includes `Image` and
//! `ToolResult` variants not present in the router's `ContentBlock`).

// Re-export types that are field-identical to the router's definitions.
pub use clanker_router::Usage;
pub use clanker_router::streaming::ContentDelta;
pub use clanker_router::streaming::MessageMetadata;
use serde::Deserialize;
use serde::Serialize;

use crate::message::Content;

/// Delta update for streaming messages (alias for ContentDelta).
pub type StreamDelta = ContentDelta;

/// Events streamed during model completion.
///
/// Mirrors `clanker_router::streaming::StreamEvent` but uses the richer
/// [`Content`] enum for `ContentBlockStart` instead of `ContentBlock`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StreamEvent {
    /// Message stream started
    MessageStart { message: MessageMetadata },
    /// Content block started
    ContentBlockStart { index: usize, content_block: Content },
    /// Content block delta (incremental update)
    ContentBlockDelta { index: usize, delta: ContentDelta },
    /// Content block completed
    ContentBlockStop { index: usize },
    /// Message-level delta (stop reason + final usage)
    MessageDelta { stop_reason: Option<String>, usage: Usage },
    /// Message stream completed
    MessageStop,
    /// Error occurred during streaming
    Error { error: String },
}

/// Convert a router `ContentBlock` into a clankers `Content`.
///
/// All three router variants (`Text`, `Thinking`, `ToolUse`) map 1:1
/// to `Content` variants of the same name.
impl From<clanker_router::streaming::ContentBlock> for Content {
    fn from(block: clanker_router::streaming::ContentBlock) -> Self {
        use clanker_router::streaming::ContentBlock;
        match block {
            ContentBlock::Text { text } => Content::Text { text },
            ContentBlock::Thinking { thinking, signature } => Content::Thinking { thinking, signature },
            ContentBlock::ToolUse { id, name, input } => Content::ToolUse { id, name, input },
        }
    }
}

/// Convert a router `StreamEvent` into a clankers `StreamEvent`.
///
/// All variants map 1:1. `ContentBlockStart` converts `ContentBlock` → `Content`
/// via the `From` impl above. `Usage` fields are identical.
impl From<clanker_router::streaming::StreamEvent> for StreamEvent {
    fn from(event: clanker_router::streaming::StreamEvent) -> Self {
        use clanker_router::streaming::StreamEvent as RouterEvent;
        match event {
            RouterEvent::MessageStart { message } => StreamEvent::MessageStart { message },
            RouterEvent::ContentBlockStart { index, content_block } => StreamEvent::ContentBlockStart {
                index,
                content_block: content_block.into(),
            },
            RouterEvent::ContentBlockDelta { index, delta } => StreamEvent::ContentBlockDelta { index, delta },
            RouterEvent::ContentBlockStop { index } => StreamEvent::ContentBlockStop { index },
            RouterEvent::MessageDelta { stop_reason, usage } => StreamEvent::MessageDelta { stop_reason, usage },
            RouterEvent::MessageStop => StreamEvent::MessageStop,
            RouterEvent::Error { error } => StreamEvent::Error { error },
        }
    }
}
