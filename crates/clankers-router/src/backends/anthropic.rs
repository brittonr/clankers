//! Native Anthropic Messages API backend
//!
//! Supports both API key and OAuth token authentication.
//! Uses SSE streaming with the Messages API v1.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use serde_json::json;
use tokio::sync::RwLock;
use tokio::sync::mpsc;
use tracing::debug;
use tracing::info;
use tracing::warn;

use crate::credential_pool::CredentialPool;
use crate::error::Error;
use crate::error::Result;
use crate::model::Model;
use crate::provider::CompletionRequest;
use crate::provider::Provider;
use crate::provider::Usage;
use crate::retry::RetryConfig;
use crate::retry::is_retryable_status;
use crate::streaming::ContentBlock;
use crate::streaming::ContentDelta;
use crate::streaming::MessageMetadata;
use crate::streaming::StreamEvent;

const BASE_URL: &str = "https://api.anthropic.com";
const API_VERSION: &str = "2023-06-01";

/// Anthropic Messages API provider.
///
/// Supports multiple credentials (accounts) via a [`CredentialPool`].
/// When one account is rate-limited, automatically fails over to the next.
pub struct AnthropicProvider {
    client: Client,
    base_url: String,
    /// Legacy single-credential field (used when pool is None)
    credential: RwLock<Credential>,
    /// Multi-credential pool (preferred when present)
    pool: Option<CredentialPool>,
    models: Vec<Model>,
    retry: RetryConfig,
}

/// Credential for the Anthropic API.
#[derive(Debug, Clone)]
pub enum Credential {
    ApiKey(String),
    OAuth(String),
}

impl Credential {
    /// Whether this credential is an OAuth token.
    pub fn is_oauth(&self) -> bool {
        matches!(self, Credential::OAuth(_))
    }

    /// Get the raw token string.
    pub fn token(&self) -> &str {
        match self {
            Credential::ApiKey(k) => k,
            Credential::OAuth(t) => t,
        }
    }
}

impl AnthropicProvider {
    /// Create a provider with a single credential (backwards compatible).
    #[allow(clippy::new_ret_no_self)]
    pub fn new(credential: Credential, base_url: Option<String>) -> Arc<dyn Provider> {
        Arc::new(Self {
            client: Client::builder().timeout(Duration::from_secs(300)).build().expect("Failed to build HTTP client"),
            base_url: base_url.unwrap_or_else(|| BASE_URL.to_string()),
            credential: RwLock::new(credential),
            pool: None,
            models: default_models(),
            retry: RetryConfig::default(),
        })
    }

    /// Create a provider with multiple credentials for load balancing / failover.
    ///
    /// When one account hits rate limits, the provider automatically rotates to
    /// the next healthy credential.
    pub fn with_pool(
        pool: CredentialPool,
        base_url: Option<String>,
    ) -> Arc<dyn Provider> {
        // Use the first credential as the legacy fallback
        let fallback = Credential::ApiKey(String::new());
        Arc::new(Self {
            client: Client::builder().timeout(Duration::from_secs(300)).build().expect("Failed to build HTTP client"),
            base_url: base_url.unwrap_or_else(|| BASE_URL.to_string()),
            credential: RwLock::new(fallback),
            pool: Some(pool),
            models: default_models(),
            retry: RetryConfig::default(),
        })
    }

    /// Update the credential (e.g. after OAuth refresh).
    pub async fn update_credential(&self, cred: Credential) {
        *self.credential.write().await = cred;
    }

    /// Get a reference to the credential pool, if configured.
    pub fn pool(&self) -> Option<&CredentialPool> {
        self.pool.as_ref()
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    async fn complete(&self, request: CompletionRequest, tx: mpsc::Sender<StreamEvent>) -> Result<()> {
        // If we have a credential pool, use pool-aware dispatch with auto-rotation.
        // Otherwise, fall back to single-credential path.
        if let Some(ref pool) = self.pool {
            return self.complete_with_pool(pool, request, tx).await;
        }

        let is_oauth = self.credential.read().await.is_oauth();
        let body = build_request_body(&request, is_oauth)?;
        let cred = self.credential.read().await.clone();

        self.do_request_with_retry(&cred, &body, &tx).await
    }

    fn models(&self) -> &[Model] {
        &self.models
    }

    fn name(&self) -> &str {
        "anthropic"
    }

    async fn is_available(&self) -> bool {
        // If we have a pool, check if any credential is available
        if let Some(ref pool) = self.pool {
            return pool.select().await.is_some();
        }

        let cred = self.credential.read().await;
        match &*cred {
            Credential::ApiKey(key) => !key.is_empty(),
            Credential::OAuth(token) => !token.is_empty(),
        }
    }
}

impl AnthropicProvider {
    /// Complete a request using the credential pool, rotating on rate-limit errors.
    async fn complete_with_pool(
        &self,
        pool: &CredentialPool,
        request: CompletionRequest,
        tx: mpsc::Sender<StreamEvent>,
    ) -> Result<()> {
        let leases = pool.select_all_available().await;

        if leases.is_empty() {
            return Err(Error::Provider {
                message: "all Anthropic credentials exhausted (rate-limited)".into(),
                status: Some(429),
            });
        }

        let num_creds = leases.len();
        let mut last_error: Option<Error> = None;

        for (i, lease) in leases.iter().enumerate() {
            let is_oauth = lease.is_oauth();
            let body = build_request_body(&request, is_oauth)?;

            let cred = if is_oauth {
                Credential::OAuth(lease.token().to_string())
            } else {
                Credential::ApiKey(lease.token().to_string())
            };

            if i > 0 {
                info!(
                    "rotating to Anthropic account '{}' ({}/{})",
                    lease.account(),
                    i + 1,
                    num_creds,
                );
            }

            match self.do_request_with_retry(&cred, &body, &tx).await {
                Ok(()) => {
                    lease.report_success().await;
                    return Ok(());
                }
                Err(e) => {
                    let status = e.status_code().unwrap_or(0);
                    lease.report_failure(status).await;

                    // Only rotate to next credential on retryable errors
                    if e.is_retryable() {
                        warn!(
                            "Anthropic account '{}' returned {} — trying next credential",
                            lease.account(), status,
                        );
                        last_error = Some(e);
                        continue;
                    }

                    // Non-retryable errors (auth, bad request) stop immediately
                    return Err(e);
                }
            }
        }

        // All credentials exhausted
        Err(last_error.unwrap_or_else(|| Error::Provider {
            message: "all Anthropic credentials exhausted".into(),
            status: Some(429),
        }))
    }

    /// Send a request to the Anthropic API with retries (single credential).
    async fn do_request_with_retry(
        &self,
        cred: &Credential,
        body: &Value,
        tx: &mpsc::Sender<StreamEvent>,
    ) -> Result<()> {
        let url = format!("{}/v1/messages", self.base_url);

        let mut attempt = 0;
        loop {
            attempt += 1;

            let mut builder = self
                .client
                .post(&url)
                .header("content-type", "application/json")
                .header("anthropic-version", API_VERSION);

            match cred {
                Credential::OAuth(token) => {
                    builder = builder
                        .header("authorization", format!("Bearer {}", token))
                        .header("anthropic-beta", "claude-code-20250219,oauth-2025-04-20,fine-grained-tool-streaming-2025-05-14,interleaved-thinking-2025-05-14")
                        .header("user-agent", "claude-cli/2.1.2 (external, cli)")
                        .header("x-app", "cli")
                        .header("anthropic-dangerous-direct-browser-access", "true");
                }
                Credential::ApiKey(key) => {
                    builder = builder
                        .header("x-api-key", key)
                        .header("anthropic-beta", "prompt-caching-2024-07-31,fine-grained-tool-streaming-2025-05-14,interleaved-thinking-2025-05-14");
                }
            };

            let resp = builder.json(body).send().await.map_err(|e| Error::Provider {
                message: format!("Anthropic request failed: {}", e),
                status: None,
            })?;

            let status = resp.status();
            if status.is_success() {
                return parse_sse_stream(resp, tx).await;
            }

            let status_code = status.as_u16();

            // Parse Retry-After header before consuming the body
            let retry_after = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(crate::retry::parse_retry_after);

            let body_text = resp.text().await.unwrap_or_default();

            if is_retryable_status(status_code) && attempt <= self.retry.max_retries {
                let delay = retry_after.unwrap_or_else(|| self.retry.backoff_for(attempt));
                warn!(
                    "Anthropic API returned {} (attempt {}/{}), retrying in {:?}{}",
                    status_code,
                    attempt,
                    self.retry.max_retries,
                    delay,
                    if retry_after.is_some() { " (Retry-After)" } else { "" },
                );
                tokio::time::sleep(delay).await;
                continue;
            }

            return Err(Error::provider_with_status(
                status_code,
                format!("Anthropic API error {}: {}", status_code, truncate(&body_text, 500)),
            ));
        }
    }
}

// ── Request building ────────────────────────────────────────────────────

fn build_request_body(request: &CompletionRequest, is_oauth: bool) -> Result<Value> {
    let mut body = json!({
        "model": request.model,
        "messages": request.messages,
        "stream": true,
    });

    if let Some(max_tokens) = request.max_tokens {
        body["max_tokens"] = json!(max_tokens);
    } else {
        body["max_tokens"] = json!(8192);
    }

    if let Some(temp) = request.temperature {
        body["temperature"] = json!(temp);
    }

    // System prompt — both OAuth and API key support prompt caching.
    // OAuth additionally requires the Claude Code identity prefix.
    if is_oauth {
        let mut blocks = vec![json!({
            "type": "text",
            "text": "You are Claude Code, Anthropic's official CLI for Claude.",
            "cache_control": { "type": "ephemeral" }
        })];
        if let Some(system) = &request.system_prompt {
            blocks.push(json!({
                "type": "text",
                "text": system,
                "cache_control": { "type": "ephemeral" }
            }));
        }
        body["system"] = json!(blocks);
    } else if let Some(system) = &request.system_prompt {
        // API key: use structured blocks with cache_control for prompt caching
        body["system"] = json!([{
            "type": "text",
            "text": system,
            "cache_control": { "type": "ephemeral" }
        }]);
    }

    if !request.tools.is_empty() {
        let tools: Vec<Value> = request
            .tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.input_schema,
                })
            })
            .collect();
        body["tools"] = json!(tools);
    }

    if let Some(thinking) = &request.thinking
        && thinking.enabled
    {
        body["thinking"] = json!({
            "type": "enabled",
            "budget_tokens": thinking.budget_tokens.unwrap_or(10000),
        });
        // Thinking requires temperature=1 or omitted
        body.as_object_mut().unwrap().remove("temperature");
    }

    Ok(body)
}

// ── SSE parsing ─────────────────────────────────────────────────────────

async fn parse_sse_stream(resp: reqwest::Response, tx: &mpsc::Sender<StreamEvent>) -> Result<()> {
    use tokio::io::AsyncBufReadExt;
    use tokio_stream::StreamExt;

    let bytes_stream = resp.bytes_stream();
    let reader = tokio_util::io::StreamReader::new(bytes_stream.map(|r| r.map_err(std::io::Error::other)));
    let mut lines = tokio::io::BufReader::new(reader).lines();

    let mut current_event_type = String::new();

    while let Some(line) = lines.next_line().await.map_err(|e| Error::Streaming {
        message: format!("SSE read error: {}", e),
    })? {
        let line = line.trim().to_string();

        if line.is_empty() || line.starts_with(':') {
            continue;
        }

        if let Some(event_type) = line.strip_prefix("event: ") {
            current_event_type = event_type.to_string();
            continue;
        }

        if let Some(data) = line.strip_prefix("data: ") {
            if data == "[DONE]" {
                break;
            }

            // Extract initial usage from message_start before parsing the event.
            // Anthropic sends input token counts and cache token counts in
            // message_start.message.usage — this is our only chance to capture them.
            if current_event_type == "message_start"
                && let Ok(v) = serde_json::from_str::<Value>(data)
            {
                let usage = &v["message"]["usage"];
                if usage.is_object() {
                    let input_tokens = usage["input_tokens"].as_u64().unwrap_or(0) as usize;
                    let cache_creation = usage["cache_creation_input_tokens"].as_u64().unwrap_or(0) as usize;
                    let cache_read = usage["cache_read_input_tokens"].as_u64().unwrap_or(0) as usize;

                    if input_tokens > 0 || cache_creation > 0 || cache_read > 0 {
                        let _ = tx
                            .send(StreamEvent::MessageDelta {
                                stop_reason: None,
                                usage: Usage {
                                    input_tokens,
                                    output_tokens: 0,
                                    cache_creation_input_tokens: cache_creation,
                                    cache_read_input_tokens: cache_read,
                                },
                            })
                            .await;
                    }
                }
            }

            if let Some(stream_event) = parse_sse_event(&current_event_type, data)?
                && tx.send(stream_event).await.is_err()
            {
                break; // receiver dropped
            }
        }
    }

    Ok(())
}

fn parse_sse_event(event_type: &str, data: &str) -> Result<Option<StreamEvent>> {
    let v: Value = serde_json::from_str(data).map_err(|e| Error::Streaming {
        message: format!("Failed to parse SSE data: {}", e),
    })?;

    let event = match event_type {
        "message_start" => {
            let msg = &v["message"];
            StreamEvent::MessageStart {
                message: MessageMetadata {
                    id: msg["id"].as_str().unwrap_or("").to_string(),
                    model: msg["model"].as_str().unwrap_or("").to_string(),
                    role: msg["role"].as_str().unwrap_or("assistant").to_string(),
                },
            }
        }
        "content_block_start" => {
            let index = v["index"].as_u64().unwrap_or(0) as usize;
            let cb = &v["content_block"];
            let block = match cb["type"].as_str() {
                Some("text") => ContentBlock::Text {
                    text: cb["text"].as_str().unwrap_or("").to_string(),
                },
                Some("thinking") => ContentBlock::Thinking {
                    thinking: cb["thinking"].as_str().unwrap_or("").to_string(),
                },
                Some("tool_use") => ContentBlock::ToolUse {
                    id: cb["id"].as_str().unwrap_or("").to_string(),
                    name: cb["name"].as_str().unwrap_or("").to_string(),
                    input: json!({}),
                },
                _ => ContentBlock::Text { text: String::new() },
            };
            StreamEvent::ContentBlockStart {
                index,
                content_block: block,
            }
        }
        "content_block_delta" => {
            let index = v["index"].as_u64().unwrap_or(0) as usize;
            let d = &v["delta"];
            let delta = match d["type"].as_str() {
                Some("text_delta") => ContentDelta::TextDelta {
                    text: d["text"].as_str().unwrap_or("").to_string(),
                },
                Some("thinking_delta") => ContentDelta::ThinkingDelta {
                    thinking: d["thinking"].as_str().unwrap_or("").to_string(),
                },
                Some("input_json_delta") => ContentDelta::InputJsonDelta {
                    partial_json: d["partial_json"].as_str().unwrap_or("").to_string(),
                },
                _ => ContentDelta::TextDelta { text: String::new() },
            };
            StreamEvent::ContentBlockDelta { index, delta }
        }
        "content_block_stop" => {
            let index = v["index"].as_u64().unwrap_or(0) as usize;
            StreamEvent::ContentBlockStop { index }
        }
        "message_delta" => {
            let stop_reason = v["delta"]["stop_reason"].as_str().map(|s| s.to_string());
            let usage = &v["usage"];
            StreamEvent::MessageDelta {
                stop_reason,
                usage: Usage {
                    input_tokens: usage["input_tokens"].as_u64().unwrap_or(0) as usize,
                    output_tokens: usage["output_tokens"].as_u64().unwrap_or(0) as usize,
                    cache_creation_input_tokens: usage["cache_creation_input_tokens"].as_u64().unwrap_or(0) as usize,
                    cache_read_input_tokens: usage["cache_read_input_tokens"].as_u64().unwrap_or(0) as usize,
                },
            }
        }
        "message_stop" => StreamEvent::MessageStop,
        "ping" => return Ok(None),
        "error" => {
            let msg = v["error"]["message"].as_str().unwrap_or("Unknown error").to_string();
            StreamEvent::Error { error: msg }
        }
        _ => {
            debug!("Unknown SSE event type: {}", event_type);
            return Ok(None);
        }
    };

    Ok(Some(event))
}

// ── Models ──────────────────────────────────────────────────────────────

fn default_models() -> Vec<Model> {
    vec![
        Model {
            id: "claude-sonnet-4-5-20250514".to_string(),
            name: "Claude Sonnet 4.5".to_string(),
            provider: "anthropic".to_string(),
            max_input_tokens: 200_000,
            max_output_tokens: 16_384,
            supports_thinking: true,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: Some(3.0),
            output_cost_per_mtok: Some(15.0),
        },
        Model {
            id: "claude-sonnet-4-20250514".to_string(),
            name: "Claude Sonnet 4".to_string(),
            provider: "anthropic".to_string(),
            max_input_tokens: 200_000,
            max_output_tokens: 16_384,
            supports_thinking: true,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: Some(3.0),
            output_cost_per_mtok: Some(15.0),
        },
        Model {
            id: "claude-opus-4-20250514".to_string(),
            name: "Claude Opus 4".to_string(),
            provider: "anthropic".to_string(),
            max_input_tokens: 200_000,
            max_output_tokens: 32_000,
            supports_thinking: true,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: Some(15.0),
            output_cost_per_mtok: Some(75.0),
        },
        Model {
            id: "claude-opus-4-6-20250610".to_string(),
            name: "Claude Opus 4.6".to_string(),
            provider: "anthropic".to_string(),
            max_input_tokens: 200_000,
            max_output_tokens: 32_000,
            supports_thinking: true,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: Some(15.0),
            output_cost_per_mtok: Some(75.0),
        },
        Model {
            id: "claude-haiku-4-5-20250514".to_string(),
            name: "Claude Haiku 4.5".to_string(),
            provider: "anthropic".to_string(),
            max_input_tokens: 200_000,
            max_output_tokens: 8_192,
            supports_thinking: true,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: Some(0.80),
            output_cost_per_mtok: Some(4.0),
        },
        Model {
            id: "claude-3-5-sonnet-20241022".to_string(),
            name: "Claude 3.5 Sonnet".to_string(),
            provider: "anthropic".to_string(),
            max_input_tokens: 200_000,
            max_output_tokens: 8_192,
            supports_thinking: false,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: Some(3.0),
            output_cost_per_mtok: Some(15.0),
        },
        Model {
            id: "claude-3-5-haiku-20241022".to_string(),
            name: "Claude 3.5 Haiku".to_string(),
            provider: "anthropic".to_string(),
            max_input_tokens: 200_000,
            max_output_tokens: 8_192,
            supports_thinking: false,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: Some(0.80),
            output_cost_per_mtok: Some(4.0),
        },
    ]
}

fn truncate(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len { s } else { &s[..max_len] }
}
