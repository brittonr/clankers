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
                continue;
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
}
