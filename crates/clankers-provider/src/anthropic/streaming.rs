//! SSE stream parsing for Anthropic Messages API

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use serde::Deserialize;
use serde_json::Value;
use tokio::sync::mpsc;

use super::subscription_compat;
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
pub async fn parse_sse_stream(
    response: reqwest::Response,
    tx: mpsc::Sender<StreamEvent>,
    reverse_map: bool,
) -> Result<()> {
    use futures::StreamExt;

    // Read the response as a stream of bytes, parse SSE manually
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut event_type = String::new();
    let mut consecutive_parse_failures: usize = 0;
    let mut rewriter = subscription_compat::InboundEventRewriter::default();
    const MAX_CONSECUTIVE_FAILURES: usize = 5;

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
            } else if let Some(data) = line.strip_prefix("data: ") {
                match parse_sse_event(&event_type, data) {
                    Some(event) => {
                        consecutive_parse_failures = 0;
                        let events = if reverse_map {
                            rewriter.rewrite(event)
                        } else {
                            vec![event]
                        };
                        for event in events {
                            if tx.send(event).await.is_err() {
                                return Ok(()); // receiver dropped
                            }
                        }
                    }
                    None if !event_type.is_empty() && event_type != "ping" => {
                        consecutive_parse_failures += 1;
                        tracing::warn!(
                            event_type = %event_type,
                            consecutive_failures = consecutive_parse_failures,
                            "Failed to parse SSE event",
                        );
                        if consecutive_parse_failures >= MAX_CONSECUTIVE_FAILURES {
                            let _ = tx
                                .send(StreamEvent::Error {
                                    error: format!(
                                        "Persistent SSE parse failures: {} consecutive events failed to parse",
                                        consecutive_parse_failures,
                                    ),
                                })
                                .await;
                            return Ok(());
                        }
                    }
                    None => {} // ping or empty event type — expected
                }
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

#[cfg(test)]
mod tests {
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

    use super::*;

    async fn fetch_sse_response(body: String) -> reqwest::Response {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind test listener");
        let addr = listener.local_addr().expect("listener addr");

        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept test request");
            let mut request = vec![0u8; 4096];
            let _ = stream.read(&mut request).await;

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body,
            );
            stream.write_all(response.as_bytes()).await.expect("write test response");
            stream.flush().await.expect("flush test response");
        });

        let response = reqwest::Client::new()
            .get(format!("http://{addr}/stream"))
            .send()
            .await
            .expect("fetch SSE response");
        server.await.expect("test server task");
        response
    }

    async fn collect_events(mut rx: mpsc::Receiver<StreamEvent>) -> Vec<StreamEvent> {
        let mut events = Vec::new();
        while let Some(event) = rx.recv().await {
            events.push(event);
        }
        events
    }

    fn collect_text_deltas(events: &[StreamEvent], index: usize) -> String {
        let mut out = String::new();
        for event in events {
            if let StreamEvent::ContentBlockDelta {
                index: event_index,
                delta: ContentDelta::TextDelta { text },
            } = event
                && *event_index == index
            {
                out.push_str(text);
            }
        }
        out
    }

    fn collect_json_deltas(events: &[StreamEvent], index: usize) -> String {
        let mut out = String::new();
        for event in events {
            if let StreamEvent::ContentBlockDelta {
                index: event_index,
                delta: ContentDelta::InputJsonDelta { partial_json },
            } = event
                && *event_index == index
            {
                out.push_str(partial_json);
            }
        }
        out
    }

    fn split_rewrite_fixture_body() -> String {
        [
            "event: message_start",
            r#"data: {"type":"message_start","message":{"id":"msg-1","model":"claude-test"}}"#,
            "",
            "event: content_block_start",
            r#"data: {"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#,
            "",
            "event: content_block_delta",
            r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":".ccage"}}"#,
            "",
            "event: content_block_delta",
            r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"nt running o"}}"#,
            "",
            "event: content_block_delta",
            r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"n"}}"#,
            "",
            "event: content_block_stop",
            r#"data: {"type":"content_block_stop","index":0}"#,
            "",
            "event: content_block_start",
            r#"data: {"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"call_1","name":"read"}}"#,
            "",
            "event: content_block_delta",
            r#"data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"path\":\".ccage"}}"#,
            "",
            "event: content_block_delta",
            r#"data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"nt/config.json\",\"mode\":\"running o"}}"#,
            "",
            "event: content_block_delta",
            r#"data: {"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"n\"}"}}"#,
            "",
            "event: content_block_stop",
            r#"data: {"type":"content_block_stop","index":1}"#,
            "",
            "event: message_stop",
            r#"data: {"type":"message_stop"}"#,
            "",
        ]
        .join("\n")
    }

    #[tokio::test]
    async fn parse_sse_stream_reverse_maps_text_and_tool_json() {
        let response = fetch_sse_response(split_rewrite_fixture_body()).await;

        let (tx, rx) = mpsc::channel(32);
        parse_sse_stream(response, tx, true).await.expect("parse reverse-mapped stream");
        let events = collect_events(rx).await;

        assert!(events.iter().any(|event| matches!(event, StreamEvent::MessageStart { .. })));
        assert!(events.iter().any(|event| matches!(event, StreamEvent::MessageStop)));
        assert_eq!(collect_text_deltas(&events, 0), ".clankers running inside");
        assert_eq!(collect_json_deltas(&events, 1), r#"{"path":".clankers/config.json","mode":"running inside"}"#);
    }

    #[tokio::test]
    async fn parse_sse_stream_preserves_text_and_tool_json_when_reverse_map_is_disabled() {
        let response = fetch_sse_response(split_rewrite_fixture_body()).await;

        let (tx, rx) = mpsc::channel(32);
        parse_sse_stream(response, tx, false).await.expect("parse passthrough stream");
        let events = collect_events(rx).await;

        assert!(events.iter().any(|event| matches!(event, StreamEvent::MessageStart { .. })));
        assert!(events.iter().any(|event| matches!(event, StreamEvent::MessageStop)));
        assert_eq!(collect_text_deltas(&events, 0), ".ccagent running on");
        assert_eq!(collect_json_deltas(&events, 1), r#"{"path":".ccagent/config.json","mode":"running on"}"#);
    }
}
