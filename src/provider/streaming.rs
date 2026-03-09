//! Streaming response types

use serde::Deserialize;
use serde::Serialize;

use crate::provider::message::Content;

/// Events streamed during model completion
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

/// Delta update for streaming messages (alias for ContentDelta)
pub type StreamDelta = ContentDelta;

/// Translate a `clankers_router` stream event into a clankers `StreamEvent`.
///
/// Used by both [`RouterCompatAdapter`](super::router::RouterCompatAdapter) and
/// [`RpcProvider`](super::rpc_provider::RpcProvider) to bridge the two type
/// hierarchies without duplicating the match arms.
pub fn translate_router_event(event: clankers_router::streaming::StreamEvent) -> StreamEvent {
    use clankers_router::streaming as router_stream;

    use crate::provider::message::Content;

    match event {
        router_stream::StreamEvent::MessageStart { message } => StreamEvent::MessageStart {
            message: MessageMetadata {
                id: message.id,
                model: message.model,
                role: message.role,
            },
        },
        router_stream::StreamEvent::ContentBlockStart { index, content_block } => {
            let block = match content_block {
                router_stream::ContentBlock::Text { text } => Content::Text { text },
                router_stream::ContentBlock::Thinking { thinking } => Content::Thinking { thinking },
                router_stream::ContentBlock::ToolUse { id, name, input } => Content::ToolUse { id, name, input },
            };
            StreamEvent::ContentBlockStart {
                index,
                content_block: block,
            }
        }
        router_stream::StreamEvent::ContentBlockDelta { index, delta } => {
            let d = match delta {
                router_stream::ContentDelta::TextDelta { text } => ContentDelta::TextDelta { text },
                router_stream::ContentDelta::ThinkingDelta { thinking } => ContentDelta::ThinkingDelta { thinking },
                router_stream::ContentDelta::InputJsonDelta { partial_json } => {
                    ContentDelta::InputJsonDelta { partial_json }
                }
            };
            StreamEvent::ContentBlockDelta { index, delta: d }
        }
        router_stream::StreamEvent::ContentBlockStop { index } => StreamEvent::ContentBlockStop { index },
        router_stream::StreamEvent::MessageDelta { stop_reason, usage } => StreamEvent::MessageDelta {
            stop_reason,
            usage: crate::provider::Usage {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                cache_creation_input_tokens: usage.cache_creation_input_tokens,
                cache_read_input_tokens: usage.cache_read_input_tokens,
            },
        },
        router_stream::StreamEvent::MessageStop => StreamEvent::MessageStop,
        router_stream::StreamEvent::Error { error } => StreamEvent::Error { error },
    }
}
