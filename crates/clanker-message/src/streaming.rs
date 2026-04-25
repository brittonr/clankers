//! Streaming response types.
//!
//! These are plain serde data contracts shared by providers, routers, and
//! embeddable engine adapters. Router-specific content-block conversion lives in
//! `clanker-router` so this crate stays free of router/runtime dependencies.

use serde::Deserialize;
use serde::Serialize;

use crate::Usage;
use crate::message::Content;

/// Metadata about a streaming message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMetadata {
    pub id: String,
    pub model: String,
    pub role: String,
}

/// Incremental delta for content blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentDelta {
    /// Text delta.
    TextDelta { text: String },
    /// Thinking delta.
    ThinkingDelta { thinking: String },
    /// Input JSON delta for tool use.
    InputJsonDelta { partial_json: String },
    /// Thinking signature delta (Anthropic; must be echoed back verbatim).
    SignatureDelta { signature: String },
}

/// Delta update for streaming messages.
pub type StreamDelta = ContentDelta;

/// Events streamed during model completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StreamEvent {
    /// Message stream started.
    MessageStart { message: MessageMetadata },
    /// Content block started.
    ContentBlockStart { index: usize, content_block: Content },
    /// Content block delta (incremental update).
    ContentBlockDelta { index: usize, delta: ContentDelta },
    /// Content block completed.
    ContentBlockStop { index: usize },
    /// Message-level delta (stop reason + final usage).
    MessageDelta { stop_reason: Option<String>, usage: Usage },
    /// Message stream completed.
    MessageStop,
    /// Error occurred during streaming.
    Error { error: String },
}
