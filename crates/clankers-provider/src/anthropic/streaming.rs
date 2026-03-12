//! SSE stream parsing for Anthropic Messages API

use serde::Deserialize;
use serde_json::Value;
use tokio::sync::mpsc;

use crate::error::Result;
use crate::message::Content;
use crate::streaming::*;

// SSE raw event types from Anthropic
#[derive(Debug, Deserialize)]
struct SseMessageStart {
    message: SseMessage,
}

#[derive(Debug, Deserialize)]
struct SseMessage {
    id: String,
    model: String,
}

#[derive(Debug, Deserialize)]
struct SseContentBlockStart {
    index: usize,
    content_block: SseContentBlock,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum SseContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse { id: String, name: String },
    #[serde(rename = "thinking")]
    Thinking { thinking: String },
}

#[derive(Debug, Deserialize)]
struct SseContentBlockDelta {
    index: usize,
    delta: SseDelta,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum SseDelta {
    #[serde(rename = "text_delta")]
    Text { text: String },
    #[serde(rename = "input_json_delta")]
    InputJson { partial_json: String },
    #[serde(rename = "thinking_delta")]
    Thinking { thinking: String },
    #[serde(rename = "signature_delta")]
    Signature { signature: String },
}

#[derive(Debug, Deserialize)]
struct SseContentBlockStop {
    index: usize,
}

#[derive(Debug, Deserialize)]
struct SseMessageDelta {
    delta: SseMessageDeltaInner,
    usage: SseUsage,
}

#[derive(Debug, Deserialize)]
struct SseMessageDeltaInner {
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SseUsage {
    #[serde(default)]
    input_tokens: usize,
    #[serde(default)]
    output_tokens: usize,
    #[serde(default)]
    cache_read_input_tokens: usize,
    #[serde(default)]
    cache_creation_input_tokens: usize,
}

#[derive(Debug, Deserialize)]
struct SseError {
    error: SseErrorInner,
}

#[derive(Debug, Deserialize)]
struct SseErrorInner {
    #[serde(rename = "type")]
    error_type: String,
    message: String,
}

/// Parse a raw SSE response stream and send StreamEvents through the channel.
///
/// Uses reqwest's byte stream to read SSE lines.
pub async fn parse_sse_stream(response: reqwest::Response, tx: mpsc::Sender<StreamEvent>) -> Result<()> {
    use futures::StreamExt;

    // Read the response as a stream of bytes, parse SSE manually
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut event_type = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| crate::error::streaming_err(e.to_string()))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        // Process complete lines
        while let Some(newline_pos) = buffer.find('\n') {
            let line = buffer[..newline_pos].trim_end_matches('\r').to_string();
            buffer = buffer[newline_pos + 1..].to_string();

            if line.is_empty() {
                // Empty line = end of event, dispatch
                event_type.clear();
                continue;
            }

            if let Some(event_name) = line.strip_prefix("event: ") {
                event_type = event_name.to_string();
            } else if let Some(data) = line.strip_prefix("data: ")
                && let Some(event) = parse_sse_event(&event_type, data)
                && tx.send(event).await.is_err()
            {
                return Ok(()); // receiver dropped
            }
        }
    }

    Ok(())
}

fn parse_sse_event(event_type: &str, data: &str) -> Option<StreamEvent> {
    match event_type {
        "message_start" => {
            let parsed: SseMessageStart = serde_json::from_str(data).ok()?;
            Some(StreamEvent::MessageStart {
                message: MessageMetadata {
                    id: parsed.message.id,
                    model: parsed.message.model,
                    role: "assistant".to_string(),
                },
            })
        }
        "content_block_start" => {
            let parsed: SseContentBlockStart = serde_json::from_str(data).ok()?;
            let content_block = match parsed.content_block {
                SseContentBlock::Text { text } => Content::Text { text },
                SseContentBlock::ToolUse { id, name } => Content::ToolUse {
                    id,
                    name,
                    input: Value::Object(serde_json::Map::new()),
                },
                SseContentBlock::Thinking { thinking } => Content::Thinking {
                    thinking,
                    signature: String::new(),
                },
            };
            Some(StreamEvent::ContentBlockStart {
                index: parsed.index,
                content_block,
            })
        }
        "content_block_delta" => {
            let parsed: SseContentBlockDelta = serde_json::from_str(data).ok()?;
            let delta = match parsed.delta {
                SseDelta::Text { text } => ContentDelta::TextDelta { text },
                SseDelta::InputJson { partial_json } => ContentDelta::InputJsonDelta { partial_json },
                SseDelta::Thinking { thinking } => ContentDelta::ThinkingDelta { thinking },
                SseDelta::Signature { signature } => ContentDelta::SignatureDelta { signature },
            };
            Some(StreamEvent::ContentBlockDelta {
                index: parsed.index,
                delta,
            })
        }
        "content_block_stop" => {
            let parsed: SseContentBlockStop = serde_json::from_str(data).ok()?;
            Some(StreamEvent::ContentBlockStop { index: parsed.index })
        }
        "message_stop" => Some(StreamEvent::MessageStop),
        "error" => {
            let parsed: SseError = serde_json::from_str(data).ok()?;
            Some(StreamEvent::Error {
                error: format!("{}: {}", parsed.error.error_type, parsed.error.message),
            })
        }
        "message_delta" => {
            let parsed: SseMessageDelta = serde_json::from_str(data).ok()?;
            let usage = crate::Usage {
                input_tokens: parsed.usage.input_tokens,
                output_tokens: parsed.usage.output_tokens,
                cache_creation_input_tokens: parsed.usage.cache_creation_input_tokens,
                cache_read_input_tokens: parsed.usage.cache_read_input_tokens,
            };
            Some(StreamEvent::MessageDelta {
                stop_reason: parsed.delta.stop_reason,
                usage,
            })
        }
        "ping" => None,
        _ => None,
    }
}
