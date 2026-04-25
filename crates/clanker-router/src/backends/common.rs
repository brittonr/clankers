//! Shared utilities for backend provider implementations.
//!
//! This module provides common infrastructure for HTTP client setup,
//! SSE streaming, retry logic, and authentication that is reused across
//! all provider backends (OpenAI-compatible, Anthropic, HuggingFace, etc.).

use std::time::Duration;

use reqwest::Client;
use reqwest::Response;
use serde_json::Value;
use tokio::io::AsyncBufReadExt;
use tokio::sync::mpsc;
use tracing::warn;

use crate::error::Error;
use crate::error::Result;
use crate::retry::RetryConfig;
use crate::retry::is_retryable_status;
use crate::retry::parse_retry_after;
use crate::streaming::StreamEvent;

// ── HTTP Client Setup ───────────────────────────────────────────────────

/// Build a configured HTTP client with timeout.
///
/// Used by all backends to create a consistent client setup.
pub fn build_http_client(timeout: Duration) -> Result<Client> {
    Client::builder().timeout(timeout).build().map_err(|e| Error::Config {
        message: format!("Failed to build HTTP client: {}", e),
    })
}

// ── Authentication Headers ──────────────────────────────────────────────

/// Authentication scheme for API requests.
#[derive(Debug, Clone)]
pub enum AuthScheme {
    /// Bearer token in Authorization header (OpenAI, OAuth)
    Bearer(String),
    /// Custom header with API key (Anthropic x-api-key)
    CustomHeader { name: String, value: String },
    /// No authentication (local servers)
    None,
}

impl AuthScheme {
    /// Create a Bearer token auth scheme.
    pub fn bearer(token: impl Into<String>) -> Self {
        Self::Bearer(token.into())
    }

    /// Create a custom header auth scheme.
    pub fn custom_header(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self::CustomHeader {
            name: name.into(),
            value: value.into(),
        }
    }

    /// Apply authentication to a request builder.
    pub fn apply(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match self {
            Self::Bearer(token) => builder.header("authorization", format!("Bearer {}", token)),
            Self::CustomHeader { name, value } => builder.header(name.as_str(), value.as_str()),
            Self::None => builder,
        }
    }
}

// ── Retry Logic ─────────────────────────────────────────────────────────

/// Execute an HTTP request with automatic retry on transient failures.
///
/// Handles:
/// - Rate limiting (429) with Retry-After header parsing
/// - Server errors (5xx)
/// - Network errors
/// - Exponential backoff with jitter
///
/// Returns the successful response or the final error after exhausting retries.
pub async fn request_with_retry<F, Fut>(retry_config: &RetryConfig, mut request_fn: F) -> Result<Response>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = std::result::Result<Response, reqwest::Error>>,
{
    let mut attempt = 0;

    loop {
        attempt += 1;

        let result = request_fn().await;

        match result {
            Ok(resp) if resp.status().is_success() => return Ok(resp),
            Ok(resp) => {
                let status = resp.status().as_u16();

                // Parse Retry-After header before consuming body
                let retry_after =
                    resp.headers().get("retry-after").and_then(|v| v.to_str().ok()).and_then(parse_retry_after);

                let body = resp.text().await.unwrap_or_default();

                if is_retryable_status(status) && attempt <= retry_config.max_retries {
                    let delay = retry_after.unwrap_or_else(|| retry_config.backoff_for(attempt));
                    warn!(
                        "HTTP {} (attempt {}/{}), retrying in {:?}{}",
                        status,
                        attempt,
                        retry_config.max_retries,
                        delay,
                        if retry_after.is_some() { " (Retry-After)" } else { "" },
                    );
                    tokio::time::sleep(delay).await;
                    continue;
                }

                return Err(Error::provider_with_status(status, format!("HTTP {}: {}", status, truncate(&body, 500))));
            }
            Err(e) => {
                if attempt <= retry_config.max_retries {
                    let delay = retry_config.backoff_for(attempt);
                    warn!(
                        "Request failed: {} (attempt {}/{}), retrying in {:?}",
                        e, attempt, retry_config.max_retries, delay,
                    );
                    tokio::time::sleep(delay).await;
                    continue;
                }
                return Err(e.into());
            }
        }
    }
}

// ── SSE Streaming Infrastructure ────────────────────────────────────────

/// SSE line reader that yields parsed event lines.
///
/// Abstracts the common pattern of:
/// - Converting response bytes_stream to AsyncBufRead
/// - Reading lines
/// - Filtering empty lines and comments
/// - Separating event type from data
pub struct SseLineReader {
    lines: tokio::io::Lines<
        tokio::io::BufReader<
            tokio_util::io::StreamReader<
                std::pin::Pin<
                    Box<
                        dyn tokio_stream::Stream<Item = std::result::Result<tokio_util::bytes::Bytes, std::io::Error>>
                            + Send,
                    >,
                >,
                tokio_util::bytes::Bytes,
            >,
        >,
    >,
}

impl SseLineReader {
    /// Create a new SSE line reader from a response.
    pub fn new(response: Response) -> Self {
        use tokio_stream::StreamExt;

        let bytes_stream = response.bytes_stream();
        let stream: std::pin::Pin<
            Box<dyn tokio_stream::Stream<Item = std::result::Result<tokio_util::bytes::Bytes, std::io::Error>> + Send>,
        > = Box::pin(bytes_stream.map(|r| r.map_err(std::io::Error::other)));
        let reader = tokio_util::io::StreamReader::new(stream);
        let lines = tokio::io::BufReader::new(reader).lines();

        Self { lines }
    }

    /// Read the next event from the stream.
    ///
    /// Returns `Ok(None)` when the stream ends.
    /// Returns `Ok(Some((event_type, data)))` for each event.
    /// Empty event_type means no explicit "event:" line (data-only events).
    pub async fn next_event(&mut self) -> Result<Option<SseEvent>> {
        let mut event_type = String::new();
        let mut data_lines = Vec::new();

        loop {
            let line = match self.lines.next_line().await {
                Ok(Some(line)) => line,
                Ok(None) => {
                    // End of stream — return accumulated event if any
                    if !data_lines.is_empty() {
                        return Ok(Some(SseEvent {
                            event_type: if event_type.is_empty() { None } else { Some(event_type) },
                            data: data_lines.join("\n"),
                        }));
                    }
                    return Ok(None);
                }
                Err(e) => {
                    return Err(Error::Streaming {
                        message: format!("SSE read error: {}", e),
                    });
                }
            };

            let line = line.trim();

            // Empty line = end of event
            if line.is_empty() {
                if !data_lines.is_empty() {
                    return Ok(Some(SseEvent {
                        event_type: if event_type.is_empty() { None } else { Some(event_type) },
                        data: data_lines.join("\n"),
                    }));
                }
                // Reset for next event
                event_type.clear();
                continue;
            }

            // Comment
            if line.starts_with(':') {
                continue;
            }

            // Event type
            if let Some(et) = line.strip_prefix("event: ") {
                event_type = et.to_string();
                continue;
            }

            // Data
            if let Some(d) = line.strip_prefix("data: ") {
                data_lines.push(d.to_string());
            }
        }
    }
}

/// A parsed SSE event.
#[derive(Debug, Clone)]
pub struct SseEvent {
    /// Event type (from "event:" field), or None for default "message" events
    pub event_type: Option<String>,
    /// Event data (from "data:" field(s), joined with newlines)
    pub data: String,
}

impl SseEvent {
    /// Get the event type, defaulting to "message" if not specified.
    pub fn event_type(&self) -> &str {
        self.event_type.as_deref().unwrap_or("message")
    }

    /// Check if this is a DONE marker event.
    pub fn is_done(&self) -> bool {
        self.data == "[DONE]"
    }

    /// Parse the data as JSON.
    pub fn parse_json(&self) -> Result<Value> {
        serde_json::from_str(&self.data).map_err(|e| Error::Streaming {
            message: format!("Failed to parse SSE JSON: {}", e),
        })
    }
}

// ── Common SSE Event Dispatching ────────────────────────────────────────

/// Generic SSE event handler trait.
///
/// Backends implement this to convert provider-specific SSE events into
/// our normalized StreamEvent types.
pub trait SseEventHandler: Send {
    /// Parse a provider-specific SSE event into a StreamEvent.
    ///
    /// Returns `Ok(None)` for events that should be ignored (e.g., ping).
    fn handle_event(&mut self, event: &SseEvent) -> Result<Option<StreamEvent>>;
}

/// Process an SSE stream using a custom event handler.
///
/// This is the main SSE processing loop used by all backends.
/// The handler is responsible for parsing provider-specific events
/// and converting them to our normalized StreamEvent types.
pub async fn process_sse_stream<H>(response: Response, tx: mpsc::Sender<StreamEvent>, mut handler: H) -> Result<()>
where H: SseEventHandler {
    let mut reader = SseLineReader::new(response);

    while let Some(event) = reader.next_event().await? {
        if event.is_done() {
            break;
        }

        match handler.handle_event(&event) {
            Ok(Some(stream_event)) => {
                if tx.send(stream_event).await.is_err() {
                    break; // receiver dropped
                }
            }
            Ok(None) => {
                // Event ignored (e.g., ping)
            }
            Err(e) => {
                warn!("SSE event handler error: {}", e);
                // Send error event but continue stream
                let _ = tx.send(StreamEvent::Error { error: e.to_string() }).await;
            }
        }
    }

    Ok(())
}

// ── Utilities ───────────────────────────────────────────────────────────

/// Truncate a string to a maximum length for error messages.
pub fn truncate(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len { s } else { &s[..max_len] }
}

/// Format bytes as human-readable size (for logging/display).
pub fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_scheme_bearer() {
        let auth = AuthScheme::bearer("test-token");
        assert!(matches!(auth, AuthScheme::Bearer(_)));
    }

    #[test]
    fn test_auth_scheme_custom() {
        let auth = AuthScheme::custom_header("x-api-key", "secret");
        assert!(matches!(auth, AuthScheme::CustomHeader { .. }));
    }

    #[test]
    fn test_sse_event_is_done() {
        let event = SseEvent {
            event_type: None,
            data: "[DONE]".to_string(),
        };
        assert!(event.is_done());

        let event2 = SseEvent {
            event_type: Some("message".into()),
            data: "{}".to_string(),
        };
        assert!(!event2.is_done());
    }

    #[test]
    fn test_sse_event_type() {
        let event = SseEvent {
            event_type: None,
            data: "{}".to_string(),
        };
        assert_eq!(event.event_type(), "message");

        let event2 = SseEvent {
            event_type: Some("ping".into()),
            data: "".to_string(),
        };
        assert_eq!(event2.event_type(), "ping");
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 5), "hello");
        assert_eq!(truncate("", 10), "");
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1_048_576), "1.0 MB");
        assert_eq!(format_bytes(1_073_741_824), "1.0 GB");
        assert_eq!(format_bytes(2_147_483_648), "2.0 GB");
    }

    #[tokio::test]
    async fn test_build_http_client() {
        let client = build_http_client(Duration::from_secs(60));
        assert!(client.is_ok());
    }

    // ── SSE line reader tests ───────────────────────────────────────

    /// Build an SseLineReader from raw SSE text for testing.
    fn sse_reader_from_bytes(data: &[u8]) -> SseLineReader {
        let stream: std::pin::Pin<Box<dyn tokio_stream::Stream<Item = std::result::Result<tokio_util::bytes::Bytes, std::io::Error>> + Send>> = Box::pin(
            tokio_stream::once(Ok(tokio_util::bytes::Bytes::copy_from_slice(data)))
        );
        let reader = tokio_util::io::StreamReader::new(stream);
        let lines = tokio::io::BufReader::new(reader).lines();
        SseLineReader { lines }
    }

    #[tokio::test]
    async fn test_sse_reader_basic_event() {
        let data = b"event: message\ndata: {\"key\": \"value\"}\n\n";
        let mut reader = sse_reader_from_bytes(data);

        let event = reader.next_event().await.unwrap().expect("should have event");
        assert_eq!(event.event_type(), "message");
        assert_eq!(event.data, "{\"key\": \"value\"}");

        let end = reader.next_event().await.unwrap();
        assert!(end.is_none());
    }

    #[tokio::test]
    async fn test_sse_reader_multiple_events() {
        let data = b"event: alpha\ndata: first\n\nevent: beta\ndata: second\n\n";
        let mut reader = sse_reader_from_bytes(data);

        let e1 = reader.next_event().await.unwrap().unwrap();
        assert_eq!(e1.event_type(), "alpha");
        assert_eq!(e1.data, "first");

        let e2 = reader.next_event().await.unwrap().unwrap();
        assert_eq!(e2.event_type(), "beta");
        assert_eq!(e2.data, "second");

        assert!(reader.next_event().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_sse_reader_data_only_event() {
        // No "event:" line — event_type defaults to "message"
        let data = b"data: hello\n\n";
        let mut reader = sse_reader_from_bytes(data);

        let event = reader.next_event().await.unwrap().unwrap();
        assert_eq!(event.event_type(), "message");
        assert_eq!(event.data, "hello");
    }

    #[tokio::test]
    async fn test_sse_reader_multiline_data() {
        // Multiple data: lines get joined with newlines
        let data = b"data: line1\ndata: line2\ndata: line3\n\n";
        let mut reader = sse_reader_from_bytes(data);

        let event = reader.next_event().await.unwrap().unwrap();
        assert_eq!(event.data, "line1\nline2\nline3");
    }

    #[tokio::test]
    async fn test_sse_reader_comments_ignored() {
        let data = b": this is a comment\nevent: msg\n: another comment\ndata: payload\n\n";
        let mut reader = sse_reader_from_bytes(data);

        let event = reader.next_event().await.unwrap().unwrap();
        assert_eq!(event.event_type(), "msg");
        assert_eq!(event.data, "payload");
    }

    #[tokio::test]
    async fn test_sse_reader_empty_lines_between_events() {
        // Extra blank lines between events should be tolerated
        let data = b"\n\nevent: first\ndata: a\n\n\n\nevent: second\ndata: b\n\n";
        let mut reader = sse_reader_from_bytes(data);

        let e1 = reader.next_event().await.unwrap().unwrap();
        assert_eq!(e1.data, "a");

        let e2 = reader.next_event().await.unwrap().unwrap();
        assert_eq!(e2.data, "b");
    }

    #[tokio::test]
    async fn test_sse_reader_done_marker() {
        let data = b"data: [DONE]\n\n";
        let mut reader = sse_reader_from_bytes(data);

        let event = reader.next_event().await.unwrap().unwrap();
        assert!(event.is_done());
    }

    #[tokio::test]
    async fn test_sse_reader_empty_stream() {
        let data = b"";
        let mut reader = sse_reader_from_bytes(data);
        assert!(reader.next_event().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_sse_reader_event_at_eof_without_trailing_newline() {
        // Stream ends without the final \n\n — should still yield the event
        let data = b"event: eof\ndata: last";
        let mut reader = sse_reader_from_bytes(data);

        let event = reader.next_event().await.unwrap().unwrap();
        assert_eq!(event.event_type(), "eof");
        assert_eq!(event.data, "last");
    }

    #[tokio::test]
    async fn test_sse_reader_json_data() {
        let json = r#"{"type":"text_delta","text":"Hello"}"#;
        let data = format!("event: content_block_delta\ndata: {}\n\n", json);
        let mut reader = sse_reader_from_bytes(data.as_bytes());

        let event = reader.next_event().await.unwrap().unwrap();
        let parsed = event.parse_json().expect("should parse JSON");
        assert_eq!(parsed["type"], "text_delta");
        assert_eq!(parsed["text"], "Hello");
    }

    #[tokio::test]
    async fn test_sse_reader_malformed_json() {
        let data = b"data: {broken json\n\n";
        let mut reader = sse_reader_from_bytes(data);

        let event = reader.next_event().await.unwrap().unwrap();
        assert!(event.parse_json().is_err());
    }

    #[tokio::test]
    async fn test_sse_reader_chunked_delivery() {
        // Simulate data arriving in multiple chunks (as in real HTTP streaming)
        let chunk1 = b"event: msg\n".to_vec();
        let chunk2 = b"data: hello".to_vec();
        let chunk3 = b" world\n\n".to_vec();

        let chunks = vec![
            Ok(tokio_util::bytes::Bytes::from(chunk1)),
            Ok(tokio_util::bytes::Bytes::from(chunk2)),
            Ok(tokio_util::bytes::Bytes::from(chunk3)),
        ];

        let stream: std::pin::Pin<Box<dyn tokio_stream::Stream<Item = std::result::Result<tokio_util::bytes::Bytes, std::io::Error>> + Send>> = Box::pin(
            tokio_stream::iter(chunks)
        );
        let reader_inner = tokio_util::io::StreamReader::new(stream);
        let lines = tokio::io::BufReader::new(reader_inner).lines();
        let mut reader = SseLineReader { lines };

        let event = reader.next_event().await.unwrap().unwrap();
        assert_eq!(event.event_type(), "msg");
        assert_eq!(event.data, "hello world");
    }

    #[tokio::test]
    async fn test_sse_reader_anthropic_style_stream() {
        // Simulate a realistic Anthropic SSE stream
        let stream_data = concat!(
            "event: message_start\n",
            "data: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_1\",\"model\":\"claude-sonnet-4-5-20250514\",\"role\":\"assistant\"}}\n\n",
            "event: content_block_start\n",
            "data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n",
            "event: ping\n",
            "data: {}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\" world!\"}}\n\n",
            "event: content_block_stop\n",
            "data: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
            "event: message_delta\n",
            "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":5}}\n\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n\n",
        );

        let mut reader = sse_reader_from_bytes(stream_data.as_bytes());

        // message_start
        let e = reader.next_event().await.unwrap().unwrap();
        assert_eq!(e.event_type(), "message_start");
        let j = e.parse_json().unwrap();
        assert_eq!(j["message"]["id"], "msg_1");

        // content_block_start
        let e = reader.next_event().await.unwrap().unwrap();
        assert_eq!(e.event_type(), "content_block_start");

        // ping
        let e = reader.next_event().await.unwrap().unwrap();
        assert_eq!(e.event_type(), "ping");

        // content_block_delta x2
        let e = reader.next_event().await.unwrap().unwrap();
        assert_eq!(e.event_type(), "content_block_delta");
        let j = e.parse_json().unwrap();
        assert_eq!(j["delta"]["text"], "Hello");

        let e = reader.next_event().await.unwrap().unwrap();
        let j = e.parse_json().unwrap();
        assert_eq!(j["delta"]["text"], " world!");

        // content_block_stop
        let e = reader.next_event().await.unwrap().unwrap();
        assert_eq!(e.event_type(), "content_block_stop");

        // message_delta
        let e = reader.next_event().await.unwrap().unwrap();
        assert_eq!(e.event_type(), "message_delta");
        let j = e.parse_json().unwrap();
        assert_eq!(j["delta"]["stop_reason"], "end_turn");

        // message_stop
        let e = reader.next_event().await.unwrap().unwrap();
        assert_eq!(e.event_type(), "message_stop");

        // end
        assert!(reader.next_event().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_sse_reader_openai_style_done() {
        // OpenAI-compatible streams end with data: [DONE]
        let data = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\n",
            "data: [DONE]\n\n",
        );

        let mut reader = sse_reader_from_bytes(data.as_bytes());

        let e1 = reader.next_event().await.unwrap().unwrap();
        assert!(!e1.is_done());
        assert!(e1.parse_json().is_ok());

        let e2 = reader.next_event().await.unwrap().unwrap();
        assert!(e2.is_done());
    }

}
