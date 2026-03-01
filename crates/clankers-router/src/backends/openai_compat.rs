//! OpenAI-compatible provider backend
//!
//! Works with any API that follows the OpenAI Chat Completions format:
//! - OpenAI (api.openai.com)
//! - OpenRouter (openrouter.ai)
//! - Groq (api.groq.com)
//! - Together (api.together.xyz)
//! - DeepSeek (api.deepseek.com)
//! - Fireworks (api.fireworks.ai)
//! - Mistral (api.mistral.ai)
//! - Local (Ollama, LM Studio, vLLM, etc.)
//!
//! The backend translates our generic CompletionRequest into the OpenAI
//! format and parses the SSE stream back into our StreamEvent types.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;
use tokio::sync::mpsc;
use tracing::warn;

use crate::error::Result;
use crate::model::Model;
use crate::provider::CompletionRequest;
use crate::provider::Provider;
use crate::provider::Usage;
use crate::retry::RetryConfig;
use crate::retry::is_retryable_status;
use crate::retry::parse_retry_after;
use crate::streaming::ContentBlock;
use crate::streaming::ContentDelta;
use crate::streaming::MessageMetadata;
use crate::streaming::StreamEvent;

/// Configuration for an OpenAI-compatible endpoint
#[derive(Debug, Clone)]
pub struct OpenAICompatConfig {
    /// Provider name (e.g. "openai", "openrouter", "groq")
    pub name: String,
    /// Base URL (e.g. "https://api.openai.com/v1")
    pub base_url: String,
    /// API key
    pub api_key: String,
    /// Custom headers (e.g. OpenRouter requires HTTP-Referer)
    pub extra_headers: Vec<(String, String)>,
    /// Available models
    pub models: Vec<Model>,
    /// Request timeout
    pub timeout: Duration,
}

impl OpenAICompatConfig {
    /// Create config for OpenAI
    pub fn openai(api_key: String) -> Self {
        Self {
            name: "openai".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            api_key,
            extra_headers: vec![],
            models: openai_models(),
            timeout: Duration::from_secs(300),
        }
    }

    /// Create config for OpenRouter
    pub fn openrouter(api_key: String) -> Self {
        Self {
            name: "openrouter".to_string(),
            base_url: "https://openrouter.ai/api/v1".to_string(),
            api_key,
            extra_headers: vec![
                ("HTTP-Referer".to_string(), "https://github.com/clankers".to_string()),
                ("X-Title".to_string(), "clankers".to_string()),
            ],
            models: openrouter_models(),
            timeout: Duration::from_secs(300),
        }
    }

    /// Create config for Groq
    pub fn groq(api_key: String) -> Self {
        Self {
            name: "groq".to_string(),
            base_url: "https://api.groq.com/openai/v1".to_string(),
            api_key,
            extra_headers: vec![],
            models: groq_models(),
            timeout: Duration::from_secs(60),
        }
    }

    /// Create config for DeepSeek
    pub fn deepseek(api_key: String) -> Self {
        Self {
            name: "deepseek".to_string(),
            base_url: "https://api.deepseek.com/v1".to_string(),
            api_key,
            extra_headers: vec![],
            models: deepseek_models(),
            timeout: Duration::from_secs(300),
        }
    }

    /// Create config for Mistral
    pub fn mistral(api_key: String) -> Self {
        Self {
            name: "mistral".to_string(),
            base_url: "https://api.mistral.ai/v1".to_string(),
            api_key,
            extra_headers: vec![],
            models: mistral_models(),
            timeout: Duration::from_secs(300),
        }
    }

    /// Create config for Together AI
    pub fn together(api_key: String) -> Self {
        Self {
            name: "together".to_string(),
            base_url: "https://api.together.xyz/v1".to_string(),
            api_key,
            extra_headers: vec![],
            models: together_models(),
            timeout: Duration::from_secs(300),
        }
    }

    /// Create config for Fireworks AI
    pub fn fireworks(api_key: String) -> Self {
        Self {
            name: "fireworks".to_string(),
            base_url: "https://api.fireworks.ai/inference/v1".to_string(),
            api_key,
            extra_headers: vec![],
            models: fireworks_models(),
            timeout: Duration::from_secs(300),
        }
    }

    /// Create config for Perplexity
    pub fn perplexity(api_key: String) -> Self {
        Self {
            name: "perplexity".to_string(),
            base_url: "https://api.perplexity.ai".to_string(),
            api_key,
            extra_headers: vec![],
            models: perplexity_models(),
            timeout: Duration::from_secs(300),
        }
    }

    /// Create config for xAI (Grok)
    pub fn xai(api_key: String) -> Self {
        Self {
            name: "xai".to_string(),
            base_url: "https://api.x.ai/v1".to_string(),
            api_key,
            extra_headers: vec![],
            models: xai_models(),
            timeout: Duration::from_secs(300),
        }
    }

    /// Create config for Google Gemini (OpenAI-compatible endpoint)
    pub fn google(api_key: String) -> Self {
        Self {
            name: "google".to_string(),
            base_url: "https://generativelanguage.googleapis.com/v1beta/openai".to_string(),
            api_key,
            extra_headers: vec![],
            models: google_models(),
            timeout: Duration::from_secs(300),
        }
    }

    /// Create config for a local server (Ollama, LM Studio, vLLM)
    pub fn local(base_url: String, models: Vec<Model>) -> Self {
        Self {
            name: "local".to_string(),
            base_url,
            api_key: String::new(), // local servers usually don't need auth
            extra_headers: vec![],
            models,
            timeout: Duration::from_secs(600),
        }
    }
}

/// OpenAI-compatible provider implementation
pub struct OpenAICompatProvider {
    config: OpenAICompatConfig,
    client: Client,
}

impl OpenAICompatProvider {
    pub fn new(config: OpenAICompatConfig) -> Arc<Self> {
        let client = Client::builder().timeout(config.timeout).build().expect("failed to build HTTP client");
        Arc::new(Self { config, client })
    }
}

#[async_trait]
impl Provider for OpenAICompatProvider {
    async fn complete(&self, request: CompletionRequest, tx: mpsc::Sender<StreamEvent>) -> Result<()> {
        let api_request = build_openai_request(&request);
        let url = format!("{}/chat/completions", self.config.base_url);
        let retry_config = RetryConfig::default();

        let mut attempt = 0;
        let response = loop {
            let mut builder = self
                .client
                .post(&url)
                .header("content-type", "application/json")
                .header("accept", "text/event-stream");

            if !self.config.api_key.is_empty() {
                builder = builder.header("authorization", format!("Bearer {}", self.config.api_key));
            }

            for (key, value) in &self.config.extra_headers {
                builder = builder.header(key.as_str(), value.as_str());
            }

            let result = builder.json(&api_request).send().await;

            match result {
                Ok(resp) if resp.status().is_success() => break resp,
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    if is_retryable_status(status) && attempt < retry_config.max_retries {
                        let backoff = if let Some(ra) = resp.headers().get("retry-after") {
                            ra.to_str()
                                .ok()
                                .and_then(parse_retry_after)
                                .unwrap_or_else(|| retry_config.backoff_for(attempt))
                        } else {
                            retry_config.backoff_for(attempt)
                        };
                        tokio::time::sleep(backoff).await;
                        attempt += 1;
                        continue;
                    }
                    let body = resp.text().await.unwrap_or_default();
                    return Err(crate::Error::provider_with_status(status, format!("HTTP {}: {}", status, body)));
                }
                Err(e) => {
                    if attempt < retry_config.max_retries {
                        tokio::time::sleep(retry_config.backoff_for(attempt)).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(e.into());
                }
            }
        };

        // Parse SSE stream
        parse_openai_sse(response, &request.model, tx).await
    }

    fn models(&self) -> &[Model] {
        &self.config.models
    }

    fn name(&self) -> &str {
        &self.config.name
    }

    async fn is_available(&self) -> bool {
        // Local providers don't need auth; others need a non-empty key
        self.config.name == "local" || !self.config.api_key.is_empty()
    }
}

// ── OpenAI API types ────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAITool>>,
    stream_options: StreamOptions,
}

#[derive(Debug, Serialize)]
struct StreamOptions {
    include_usage: bool,
}

#[derive(Debug, Serialize)]
struct OpenAIMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Serialize)]
struct OpenAITool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAIFunction,
}

#[derive(Debug, Serialize)]
struct OpenAIFunction {
    name: String,
    description: String,
    parameters: Value,
}

fn build_openai_request(request: &CompletionRequest) -> OpenAIRequest {
    let mut messages = Vec::new();

    // System prompt
    if let Some(ref prompt) = request.system_prompt {
        messages.push(OpenAIMessage {
            role: "system".to_string(),
            content: Some(Value::String(prompt.clone())),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });
    }

    // Conversation messages (passed as raw JSON values)
    for msg in &request.messages {
        // Messages are already in a provider-agnostic JSON format
        if let Some(role) = msg.get("role").and_then(|r| r.as_str()) {
            messages.push(OpenAIMessage {
                role: role.to_string(),
                content: msg.get("content").cloned(),
                tool_calls: msg.get("tool_calls").cloned().map(
                    |v| {
                        if let Value::Array(arr) = v { arr } else { vec![v] }
                    },
                ),
                tool_call_id: msg.get("tool_call_id").and_then(|v| v.as_str()).map(String::from),
                name: msg.get("name").and_then(|v| v.as_str()).map(String::from),
            });
        }
    }

    let tools = if request.tools.is_empty() {
        None
    } else {
        Some(
            request
                .tools
                .iter()
                .map(|t| OpenAITool {
                    tool_type: "function".to_string(),
                    function: OpenAIFunction {
                        name: t.name.clone(),
                        description: t.description.clone(),
                        parameters: t.input_schema.clone(),
                    },
                })
                .collect(),
        )
    };

    OpenAIRequest {
        model: request.model.clone(),
        messages,
        stream: true,
        max_tokens: request.max_tokens,
        temperature: request.temperature,
        tools,
        stream_options: StreamOptions { include_usage: true },
    }
}

// ── SSE parsing ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct OpenAIChunk {
    id: Option<String>,
    model: Option<String>,
    choices: Option<Vec<OpenAIChoice>>,
    usage: Option<OpenAIUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    #[allow(dead_code)]
    index: Option<usize>,
    delta: Option<OpenAIDelta>,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIDelta {
    #[allow(dead_code)]
    role: Option<String>,
    content: Option<String>,
    /// Reasoning content from o3/o3-mini models
    reasoning_content: Option<String>,
    /// Reasoning content from Ollama/Qwen3 (alias for reasoning_content)
    reasoning: Option<String>,
    tool_calls: Option<Vec<OpenAIToolCall>>,
}

impl OpenAIDelta {
    /// Get reasoning content from either field (OpenAI uses `reasoning_content`, Ollama uses
    /// `reasoning`)
    fn thinking(&self) -> Option<&str> {
        self.reasoning_content.as_deref().or(self.reasoning.as_deref())
    }
}

#[derive(Debug, Deserialize)]
struct OpenAIToolCall {
    index: Option<usize>,
    id: Option<String>,
    function: Option<OpenAIFunctionCall>,
}

#[derive(Debug, Deserialize)]
struct OpenAIFunctionCall {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIUsage {
    prompt_tokens: Option<usize>,
    completion_tokens: Option<usize>,
}

async fn parse_openai_sse(response: reqwest::Response, model: &str, tx: mpsc::Sender<StreamEvent>) -> Result<()> {
    use tokio::io::AsyncBufReadExt;
    use tokio_stream::StreamExt;

    let mut sent_start = false;
    let mut content_block_started = false;
    let mut tool_blocks: std::collections::HashMap<usize, (String, String)> = Default::default();

    let bytes_stream = response.bytes_stream();
    let reader = tokio_util::io::StreamReader::new(bytes_stream.map(|r| r.map_err(std::io::Error::other)));
    let mut lines = tokio::io::BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await.map_err(|e| crate::Error::Streaming {
        message: format!("SSE read error: {}", e),
    })? {
        let line = line.trim().to_string();

        if line.is_empty() || line.starts_with(':') {
            continue;
        }

        if !line.starts_with("data: ") {
            continue;
        }

        let data = &line[6..];
        if data == "[DONE]" {
            break;
        }

        let chunk: OpenAIChunk = match serde_json::from_str(data) {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to parse SSE chunk: {}: {}", e, data);
                continue;
            }
        };

        // Send MessageStart on first chunk
        if !sent_start {
            let _ = tx
                .send(StreamEvent::MessageStart {
                    message: MessageMetadata {
                        id: chunk.id.clone().unwrap_or_default(),
                        model: chunk.model.clone().unwrap_or_else(|| model.to_string()),
                        role: "assistant".to_string(),
                    },
                })
                .await;
            sent_start = true;
        }

        if let Some(choices) = &chunk.choices {
            for choice in choices {
                if let Some(delta) = &choice.delta {
                    // Reasoning content (o3/o3-mini use `reasoning_content`, Ollama/Qwen3 use `reasoning`)
                    if let Some(reasoning) = delta.thinking()
                        && !reasoning.is_empty()
                    {
                        // Emit as a thinking block (index 0 reserved for thinking)
                        if !content_block_started {
                            let _ = tx
                                .send(StreamEvent::ContentBlockStart {
                                    index: 0,
                                    content_block: ContentBlock::Thinking {
                                        thinking: String::new(),
                                    },
                                })
                                .await;
                            content_block_started = true;
                        }
                        let _ = tx
                            .send(StreamEvent::ContentBlockDelta {
                                index: 0,
                                delta: ContentDelta::ThinkingDelta {
                                    thinking: reasoning.to_string(),
                                },
                            })
                            .await;
                    }

                    // Text content
                    if let Some(ref text) = delta.content {
                        if !content_block_started {
                            let _ = tx
                                .send(StreamEvent::ContentBlockStart {
                                    index: 0,
                                    content_block: ContentBlock::Text { text: String::new() },
                                })
                                .await;
                            content_block_started = true;
                        }
                        let _ = tx
                            .send(StreamEvent::ContentBlockDelta {
                                index: 0,
                                delta: ContentDelta::TextDelta { text: text.clone() },
                            })
                            .await;
                    }

                    // Tool calls
                    if let Some(ref tool_calls) = delta.tool_calls {
                        for tc in tool_calls {
                            let idx = tc.index.unwrap_or(0) + 1; // offset by 1 since text is 0

                            if let Some(ref func) = tc.function {
                                if let Some(ref name) = func.name {
                                    // New tool call block
                                    let id = tc.id.clone().unwrap_or_else(|| format!("call_{}", idx));
                                    tool_blocks.insert(idx, (id.clone(), name.clone()));

                                    let _ = tx
                                        .send(StreamEvent::ContentBlockStart {
                                            index: idx,
                                            content_block: ContentBlock::ToolUse {
                                                id,
                                                name: name.clone(),
                                                input: json!({}),
                                            },
                                        })
                                        .await;
                                }

                                if let Some(ref args) = func.arguments {
                                    let _ = tx
                                        .send(StreamEvent::ContentBlockDelta {
                                            index: idx,
                                            delta: ContentDelta::InputJsonDelta {
                                                partial_json: args.clone(),
                                            },
                                        })
                                        .await;
                                }
                            }
                        }
                    }
                }

                // Finish reason
                if let Some(ref reason) = choice.finish_reason {
                    // Close any open content blocks
                    if content_block_started {
                        let _ = tx.send(StreamEvent::ContentBlockStop { index: 0 }).await;
                    }
                    for &idx in tool_blocks.keys() {
                        let _ = tx.send(StreamEvent::ContentBlockStop { index: idx }).await;
                    }

                    let stop_reason = match reason.as_str() {
                        "stop" => Some("end_turn".to_string()),
                        "tool_calls" => Some("tool_use".to_string()),
                        "length" => Some("max_tokens".to_string()),
                        other => Some(other.to_string()),
                    };

                    let usage = chunk
                        .usage
                        .as_ref()
                        .map(|u| Usage {
                            input_tokens: u.prompt_tokens.unwrap_or(0),
                            output_tokens: u.completion_tokens.unwrap_or(0),
                            ..Default::default()
                        })
                        .unwrap_or_default();

                    let _ = tx.send(StreamEvent::MessageDelta { stop_reason, usage }).await;
                }
            }
        }

        // Usage in final chunk (OpenAI sends it separately with stream_options)
        if chunk.choices.is_none()
            && let Some(ref usage) = chunk.usage
        {
            let _ = tx
                .send(StreamEvent::MessageDelta {
                    stop_reason: None,
                    usage: Usage {
                        input_tokens: usage.prompt_tokens.unwrap_or(0),
                        output_tokens: usage.completion_tokens.unwrap_or(0),
                        ..Default::default()
                    },
                })
                .await;
        }
    }

    let _ = tx.send(StreamEvent::MessageStop).await;
    Ok(())
}

// ── Default model lists ─────────────────────────────────────────────────

/// Static catalog of popular OpenRouter models.
///
/// OpenRouter proxies to hundreds of providers; we include a curated set
/// of widely-used models here. Models from other providers that the user
/// has direct credentials for are registered via their native backends,
/// so these use the `openrouter/` prefix to avoid routing ambiguity.
fn openrouter_models() -> Vec<Model> {
    vec![
        Model {
            id: "openrouter/auto".into(),
            name: "OpenRouter Auto".into(),
            provider: "openrouter".into(),
            max_input_tokens: 200_000,
            max_output_tokens: 16_384,
            supports_thinking: false,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: None,
            output_cost_per_mtok: None,
        },
        Model {
            id: "anthropic/claude-sonnet-4-20250514".into(),
            name: "Claude Sonnet 4 (via OpenRouter)".into(),
            provider: "openrouter".into(),
            max_input_tokens: 200_000,
            max_output_tokens: 16_000,
            supports_thinking: true,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: Some(3.0),
            output_cost_per_mtok: Some(15.0),
        },
        Model {
            id: "google/gemini-2.5-pro-preview".into(),
            name: "Gemini 2.5 Pro (via OpenRouter)".into(),
            provider: "openrouter".into(),
            max_input_tokens: 1_048_576,
            max_output_tokens: 65_536,
            supports_thinking: true,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: Some(1.25),
            output_cost_per_mtok: Some(10.0),
        },
        Model {
            id: "google/gemini-2.5-flash-preview".into(),
            name: "Gemini 2.5 Flash (via OpenRouter)".into(),
            provider: "openrouter".into(),
            max_input_tokens: 1_048_576,
            max_output_tokens: 65_536,
            supports_thinking: true,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: Some(0.15),
            output_cost_per_mtok: Some(0.6),
        },
        Model {
            id: "meta-llama/llama-4-maverick".into(),
            name: "Llama 4 Maverick (via OpenRouter)".into(),
            provider: "openrouter".into(),
            max_input_tokens: 1_048_576,
            max_output_tokens: 65_536,
            supports_thinking: false,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: Some(0.2),
            output_cost_per_mtok: Some(0.6),
        },
        Model {
            id: "deepseek/deepseek-r1".into(),
            name: "DeepSeek R1 (via OpenRouter)".into(),
            provider: "openrouter".into(),
            max_input_tokens: 64_000,
            max_output_tokens: 8_192,
            supports_thinking: true,
            supports_images: false,
            supports_tools: false,
            input_cost_per_mtok: Some(0.55),
            output_cost_per_mtok: Some(2.19),
        },
    ]
}

fn openai_models() -> Vec<Model> {
    vec![
        Model {
            id: "gpt-4o".into(),
            name: "GPT-4o".into(),
            provider: "openai".into(),
            max_input_tokens: 128_000,
            max_output_tokens: 16_384,
            supports_thinking: false,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: Some(2.5),
            output_cost_per_mtok: Some(10.0),
        },
        Model {
            id: "gpt-4o-mini".into(),
            name: "GPT-4o Mini".into(),
            provider: "openai".into(),
            max_input_tokens: 128_000,
            max_output_tokens: 16_384,
            supports_thinking: false,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: Some(0.15),
            output_cost_per_mtok: Some(0.6),
        },
        Model {
            id: "o3".into(),
            name: "o3".into(),
            provider: "openai".into(),
            max_input_tokens: 200_000,
            max_output_tokens: 100_000,
            supports_thinking: true,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: Some(10.0),
            output_cost_per_mtok: Some(40.0),
        },
        Model {
            id: "o3-mini".into(),
            name: "o3 Mini".into(),
            provider: "openai".into(),
            max_input_tokens: 200_000,
            max_output_tokens: 100_000,
            supports_thinking: true,
            supports_images: false,
            supports_tools: true,
            input_cost_per_mtok: Some(1.1),
            output_cost_per_mtok: Some(4.4),
        },
    ]
}

fn groq_models() -> Vec<Model> {
    vec![
        Model {
            id: "llama-3.3-70b-versatile".into(),
            name: "Llama 3.3 70B".into(),
            provider: "groq".into(),
            max_input_tokens: 128_000,
            max_output_tokens: 32_768,
            supports_thinking: false,
            supports_images: false,
            supports_tools: true,
            input_cost_per_mtok: Some(0.59),
            output_cost_per_mtok: Some(0.79),
        },
        Model {
            id: "llama-3.1-8b-instant".into(),
            name: "Llama 3.1 8B".into(),
            provider: "groq".into(),
            max_input_tokens: 128_000,
            max_output_tokens: 8_192,
            supports_thinking: false,
            supports_images: false,
            supports_tools: true,
            input_cost_per_mtok: Some(0.05),
            output_cost_per_mtok: Some(0.08),
        },
    ]
}

fn deepseek_models() -> Vec<Model> {
    vec![
        Model {
            id: "deepseek-chat".into(),
            name: "DeepSeek V3".into(),
            provider: "deepseek".into(),
            max_input_tokens: 64_000,
            max_output_tokens: 8_192,
            supports_thinking: false,
            supports_images: false,
            supports_tools: true,
            input_cost_per_mtok: Some(0.27),
            output_cost_per_mtok: Some(1.1),
        },
        Model {
            id: "deepseek-reasoner".into(),
            name: "DeepSeek R1".into(),
            provider: "deepseek".into(),
            max_input_tokens: 64_000,
            max_output_tokens: 8_192,
            supports_thinking: true,
            supports_images: false,
            supports_tools: false,
            input_cost_per_mtok: Some(0.55),
            output_cost_per_mtok: Some(2.19),
        },
    ]
}

fn mistral_models() -> Vec<Model> {
    vec![
        Model {
            id: "mistral-large-latest".into(),
            name: "Mistral Large".into(),
            provider: "mistral".into(),
            max_input_tokens: 128_000,
            max_output_tokens: 8_192,
            supports_thinking: false,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: Some(2.0),
            output_cost_per_mtok: Some(6.0),
        },
        Model {
            id: "mistral-small-latest".into(),
            name: "Mistral Small".into(),
            provider: "mistral".into(),
            max_input_tokens: 128_000,
            max_output_tokens: 8_192,
            supports_thinking: false,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: Some(0.1),
            output_cost_per_mtok: Some(0.3),
        },
        Model {
            id: "codestral-latest".into(),
            name: "Codestral".into(),
            provider: "mistral".into(),
            max_input_tokens: 256_000,
            max_output_tokens: 8_192,
            supports_thinking: false,
            supports_images: false,
            supports_tools: true,
            input_cost_per_mtok: Some(0.3),
            output_cost_per_mtok: Some(0.9),
        },
    ]
}

fn together_models() -> Vec<Model> {
    vec![
        Model {
            id: "meta-llama/Llama-3.3-70B-Instruct-Turbo".into(),
            name: "Llama 3.3 70B Turbo".into(),
            provider: "together".into(),
            max_input_tokens: 128_000,
            max_output_tokens: 8_192,
            supports_thinking: false,
            supports_images: false,
            supports_tools: true,
            input_cost_per_mtok: Some(0.88),
            output_cost_per_mtok: Some(0.88),
        },
        Model {
            id: "deepseek-ai/DeepSeek-R1".into(),
            name: "DeepSeek R1 (Together)".into(),
            provider: "together".into(),
            max_input_tokens: 64_000,
            max_output_tokens: 8_192,
            supports_thinking: true,
            supports_images: false,
            supports_tools: false,
            input_cost_per_mtok: Some(3.0),
            output_cost_per_mtok: Some(7.0),
        },
        Model {
            id: "Qwen/Qwen2.5-Coder-32B-Instruct".into(),
            name: "Qwen 2.5 Coder 32B".into(),
            provider: "together".into(),
            max_input_tokens: 32_768,
            max_output_tokens: 8_192,
            supports_thinking: false,
            supports_images: false,
            supports_tools: true,
            input_cost_per_mtok: Some(0.8),
            output_cost_per_mtok: Some(0.8),
        },
    ]
}

fn fireworks_models() -> Vec<Model> {
    vec![
        Model {
            id: "accounts/fireworks/models/llama-v3p3-70b-instruct".into(),
            name: "Llama 3.3 70B (Fireworks)".into(),
            provider: "fireworks".into(),
            max_input_tokens: 128_000,
            max_output_tokens: 16_384,
            supports_thinking: false,
            supports_images: false,
            supports_tools: true,
            input_cost_per_mtok: Some(0.9),
            output_cost_per_mtok: Some(0.9),
        },
        Model {
            id: "accounts/fireworks/models/deepseek-r1".into(),
            name: "DeepSeek R1 (Fireworks)".into(),
            provider: "fireworks".into(),
            max_input_tokens: 64_000,
            max_output_tokens: 8_192,
            supports_thinking: true,
            supports_images: false,
            supports_tools: false,
            input_cost_per_mtok: Some(3.0),
            output_cost_per_mtok: Some(8.0),
        },
    ]
}

fn perplexity_models() -> Vec<Model> {
    vec![
        Model {
            id: "sonar-pro".into(),
            name: "Sonar Pro".into(),
            provider: "perplexity".into(),
            max_input_tokens: 200_000,
            max_output_tokens: 8_192,
            supports_thinking: false,
            supports_images: false,
            supports_tools: false,
            input_cost_per_mtok: Some(3.0),
            output_cost_per_mtok: Some(15.0),
        },
        Model {
            id: "sonar".into(),
            name: "Sonar".into(),
            provider: "perplexity".into(),
            max_input_tokens: 128_000,
            max_output_tokens: 8_192,
            supports_thinking: false,
            supports_images: false,
            supports_tools: false,
            input_cost_per_mtok: Some(1.0),
            output_cost_per_mtok: Some(1.0),
        },
        Model {
            id: "sonar-reasoning-pro".into(),
            name: "Sonar Reasoning Pro".into(),
            provider: "perplexity".into(),
            max_input_tokens: 200_000,
            max_output_tokens: 16_384,
            supports_thinking: true,
            supports_images: false,
            supports_tools: false,
            input_cost_per_mtok: Some(2.0),
            output_cost_per_mtok: Some(8.0),
        },
    ]
}

fn xai_models() -> Vec<Model> {
    vec![
        Model {
            id: "grok-3".into(),
            name: "Grok 3".into(),
            provider: "xai".into(),
            max_input_tokens: 131_072,
            max_output_tokens: 16_384,
            supports_thinking: false,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: Some(3.0),
            output_cost_per_mtok: Some(15.0),
        },
        Model {
            id: "grok-3-mini".into(),
            name: "Grok 3 Mini".into(),
            provider: "xai".into(),
            max_input_tokens: 131_072,
            max_output_tokens: 16_384,
            supports_thinking: true,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: Some(0.3),
            output_cost_per_mtok: Some(0.5),
        },
    ]
}

fn google_models() -> Vec<Model> {
    vec![
        Model {
            id: "gemini-2.5-pro-preview-05-06".into(),
            name: "Gemini 2.5 Pro".into(),
            provider: "google".into(),
            max_input_tokens: 1_048_576,
            max_output_tokens: 65_536,
            supports_thinking: true,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: Some(1.25),
            output_cost_per_mtok: Some(10.0),
        },
        Model {
            id: "gemini-2.5-flash-preview-05-20".into(),
            name: "Gemini 2.5 Flash".into(),
            provider: "google".into(),
            max_input_tokens: 1_048_576,
            max_output_tokens: 65_536,
            supports_thinking: true,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: Some(0.15),
            output_cost_per_mtok: Some(0.60),
        },
        Model {
            id: "gemini-2.0-flash".into(),
            name: "Gemini 2.0 Flash".into(),
            provider: "google".into(),
            max_input_tokens: 1_048_576,
            max_output_tokens: 8_192,
            supports_thinking: false,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: Some(0.10),
            output_cost_per_mtok: Some(0.40),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_config() {
        let config = OpenAICompatConfig::openai("sk-test".into());
        assert_eq!(config.name, "openai");
        assert!(config.base_url.contains("openai.com"));
        assert!(!config.models.is_empty());
    }

    #[test]
    fn test_groq_config() {
        let config = OpenAICompatConfig::groq("gsk-test".into());
        assert_eq!(config.name, "groq");
        assert!(config.base_url.contains("groq.com"));
    }

    #[test]
    fn test_deepseek_config() {
        let config = OpenAICompatConfig::deepseek("dk-test".into());
        assert_eq!(config.name, "deepseek");
        assert_eq!(config.models.len(), 2);
    }

    #[test]
    fn test_openrouter_config() {
        let config = OpenAICompatConfig::openrouter("sk-or-test".into());
        assert_eq!(config.name, "openrouter");
        assert!(!config.extra_headers.is_empty());
    }

    #[test]
    fn test_mistral_config() {
        let config = OpenAICompatConfig::mistral("mk-test".into());
        assert_eq!(config.name, "mistral");
        assert!(config.base_url.contains("mistral.ai"));
        assert!(!config.models.is_empty());
    }

    #[test]
    fn test_together_config() {
        let config = OpenAICompatConfig::together("tk-test".into());
        assert_eq!(config.name, "together");
        assert!(config.base_url.contains("together.xyz"));
    }

    #[test]
    fn test_fireworks_config() {
        let config = OpenAICompatConfig::fireworks("fk-test".into());
        assert_eq!(config.name, "fireworks");
        assert!(config.base_url.contains("fireworks.ai"));
    }

    #[test]
    fn test_perplexity_config() {
        let config = OpenAICompatConfig::perplexity("pk-test".into());
        assert_eq!(config.name, "perplexity");
        assert!(config.base_url.contains("perplexity.ai"));
    }

    #[test]
    fn test_xai_config() {
        let config = OpenAICompatConfig::xai("xk-test".into());
        assert_eq!(config.name, "xai");
        assert!(config.base_url.contains("x.ai"));
    }

    #[test]
    fn test_google_config() {
        let config = OpenAICompatConfig::google("gk-test".into());
        assert_eq!(config.name, "google");
        assert!(config.base_url.contains("generativelanguage.googleapis.com"));
        assert!(!config.models.is_empty());
    }

    #[test]
    fn test_local_config() {
        let config = OpenAICompatConfig::local("http://localhost:11434/v1".into(), vec![Model {
            id: "llama3".into(),
            name: "Llama 3".into(),
            provider: "local".into(),
            max_input_tokens: 8192,
            max_output_tokens: 4096,
            supports_thinking: false,
            supports_images: false,
            supports_tools: false,
            input_cost_per_mtok: None,
            output_cost_per_mtok: None,
        }]);
        assert_eq!(config.name, "local");
        assert!(config.api_key.is_empty());
    }

    #[test]
    fn test_build_openai_request() {
        use crate::provider::ToolDefinition;
        let request = CompletionRequest {
            model: "gpt-4o".into(),
            messages: vec![json!({"role": "user", "content": "hello"})],
            system_prompt: Some("You are helpful.".into()),
            max_tokens: Some(1024),
            temperature: Some(0.7),
            tools: vec![ToolDefinition {
                name: "bash".into(),
                description: "Run a command".into(),
                input_schema: json!({"type": "object", "properties": {"command": {"type": "string"}}}),
            }],
            thinking: None,
        };

        let oai_req = build_openai_request(&request);
        assert_eq!(oai_req.model, "gpt-4o");
        assert!(oai_req.stream);
        assert_eq!(oai_req.messages.len(), 2); // system + user
        assert_eq!(oai_req.messages[0].role, "system");
        assert_eq!(oai_req.messages[1].role, "user");
        assert!(oai_req.tools.is_some());
        assert_eq!(oai_req.tools.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_build_request_no_tools() {
        let request = CompletionRequest {
            model: "gpt-4o".into(),
            messages: vec![],
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            tools: vec![],
            thinking: None,
        };
        let oai_req = build_openai_request(&request);
        assert!(oai_req.tools.is_none());
        assert!(oai_req.messages.is_empty());
    }
}
