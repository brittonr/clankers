//! Anthropic Messages API HTTP client

use reqwest::Client;
use serde::Serialize;
use serde_json::Value;

use crate::error::Result;
use crate::CompletionRequest;
use crate::auth::Credential;
use crate::retry::RetryConfig;
use crate::retry::is_retryable_status;
use crate::retry::parse_retry_after;

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const API_VERSION: &str = "2023-06-01";
const CLANKERS_VERSION: &str = env!("CARGO_PKG_VERSION");

// --- Anthropic API request types ---

#[derive(Debug, Serialize)]
pub(crate) struct ApiRequest {
    pub model: String,
    pub messages: Vec<ApiMessage>,
    pub max_tokens: usize,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<Vec<SystemBlock>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ApiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingParam>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SystemBlock {
    #[serde(rename = "type")]
    pub block_type: String, // always "text"
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CacheControl {
    #[serde(rename = "type")]
    pub control_type: String, // "ephemeral"
}

impl CacheControl {
    pub fn ephemeral() -> Self {
        Self {
            control_type: "ephemeral".to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub(crate) enum ThinkingParam {
    #[serde(rename = "enabled")]
    Enabled { budget_tokens: usize },
    /// API-contract variant: explicit opt-out of extended thinking.
    /// Not currently constructed but required for protocol completeness.
    #[serde(rename = "disabled")]
    _Disabled,
}

#[derive(Debug, Serialize)]
pub(crate) struct ApiMessage {
    pub role: String,
    pub content: Vec<ApiContentBlock>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub(crate) enum ApiContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { source: ApiImageSource },
    #[serde(rename = "tool_use")]
    ToolUse { id: String, name: String, input: Value },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: Vec<ApiContentBlock>,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
    #[serde(rename = "thinking")]
    Thinking { thinking: String, signature: String },
}

#[derive(Debug, Serialize)]
pub(crate) struct ApiImageSource {
    #[serde(rename = "type")]
    pub source_type: String, // "base64"
    pub media_type: String,
    pub data: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ApiTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

// --- Client ---

pub struct AnthropicClient {
    client: Client,
    base_url: String,
}

impl AnthropicClient {
    pub fn new(base_url: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("failed to build HTTP client");
        Self {
            client,
            base_url: base_url.unwrap_or_else(|| DEFAULT_BASE_URL.to_string()),
        }
    }

    /// Build and send a streaming request, returning the raw response for SSE parsing
    pub(crate) async fn send_streaming(
        &self,
        request: &ApiRequest,
        credential: &Credential,
    ) -> Result<reqwest::Response> {
        let url = format!("{}/v1/messages", self.base_url);
        let retry_config = RetryConfig::default();

        let mut attempt = 0;
        loop {
            let mut builder = self
                .client
                .post(&url)
                .header("content-type", "application/json")
                .header("anthropic-version", API_VERSION)
                .header("accept", "text/event-stream");

            if credential.is_oauth() {
                builder = builder
                    .header("authorization", format!("Bearer {}", credential.token()))
                    .header("anthropic-beta", "claude-code-20250219,oauth-2025-04-20,fine-grained-tool-streaming-2025-05-14,interleaved-thinking-2025-05-14")
                    .header("user-agent", format!("claude-cli/{} (external, cli)", "2.1.2"))
                    .header("x-app", "cli")
                    .header("anthropic-dangerous-direct-browser-access", "true");
            } else {
                builder = builder
                    .header("x-api-key", credential.token())
                    .header("anthropic-beta", "fine-grained-tool-streaming-2025-05-14,interleaved-thinking-2025-05-14")
                    .header("user-agent", format!("clankers/{}", CLANKERS_VERSION));
            }

            let result = builder.json(request).send().await;

            match result {
                Ok(response) => {
                    let status = response.status().as_u16();

                    // Success case - return the response
                    if response.status().is_success() {
                        return Ok(response);
                    }

                    // Check if the error is retryable
                    if is_retryable_status(status) && attempt < retry_config.max_retries {
                        // Check for Retry-After header
                        let backoff = if let Some(retry_after) = response.headers().get("retry-after") {
                            if let Ok(header_str) = retry_after.to_str() {
                                parse_retry_after(header_str).unwrap_or_else(|| retry_config.backoff_for(attempt))
                            } else {
                                retry_config.backoff_for(attempt)
                            }
                        } else {
                            retry_config.backoff_for(attempt)
                        };

                        eprintln!(
                            "Retryable HTTP error {} (attempt {}/{}), backing off for {:?}",
                            status,
                            attempt + 1,
                            retry_config.max_retries,
                            backoff
                        );

                        tokio::time::sleep(backoff).await;
                        attempt += 1;
                        continue;
                    }

                    // Non-retryable status or max retries exceeded
                    let body = response.text().await.unwrap_or_default();
                    return Err(crate::error::provider_err(format!("HTTP error {}: {}", status, body)));
                }
                Err(e) => {
                    // Check if the error message suggests a retryable condition
                    let error_msg = e.to_string();
                    let is_retryable = crate::retry::is_retryable_error(&error_msg);

                    if is_retryable && attempt < retry_config.max_retries {
                        let backoff = retry_config.backoff_for(attempt);
                        eprintln!(
                            "Retryable network error (attempt {}/{}): {}, backing off for {:?}",
                            attempt + 1,
                            retry_config.max_retries,
                            error_msg,
                            backoff
                        );

                        tokio::time::sleep(backoff).await;
                        attempt += 1;
                        continue;
                    }

                    // Non-retryable error or max retries exceeded
                    return Err(crate::error::provider_err(format!("HTTP request failed: {}", e)));
                }
            }
        }
    }
}

/// Convert our CompletionRequest into the Anthropic API format
pub(crate) fn build_api_request(request: &CompletionRequest, is_oauth: bool) -> ApiRequest {
    let mut system_blocks = Vec::new();
    let cache = CacheControl::ephemeral();

    // OAuth requires Claude Code identity prefix
    if is_oauth {
        system_blocks.push(SystemBlock {
            block_type: "text".to_string(),
            text: "You are Claude Code, Anthropic's official CLI for Claude.".to_string(),
            cache_control: Some(cache.clone()),
        });
    }

    if let Some(ref prompt) = request.system_prompt {
        system_blocks.push(SystemBlock {
            block_type: "text".to_string(),
            text: prompt.clone(),
            cache_control: Some(cache.clone()),
        });
    }

    let messages = convert_messages(&request.messages);
    let tools = if request.tools.is_empty() {
        None
    } else {
        Some(convert_tools(&request.tools))
    };

    let thinking = request.thinking.as_ref().and_then(|t| {
        if t.enabled {
            Some(ThinkingParam::Enabled {
                budget_tokens: t.budget_tokens.unwrap_or(1024),
            })
        } else {
            None
        }
    });

    ApiRequest {
        model: request.model.clone(),
        messages,
        max_tokens: request.max_tokens.unwrap_or(16384),
        stream: true,
        system: if system_blocks.is_empty() {
            None
        } else {
            Some(system_blocks)
        },
        tools,
        temperature: request.temperature,
        thinking,
    }
}

fn convert_messages(messages: &[crate::message::AgentMessage]) -> Vec<ApiMessage> {
    use crate::message::AgentMessage;
    let mut api_messages = Vec::new();

    for msg in messages {
        match msg {
            AgentMessage::User(user) => {
                let content = user.content.iter().map(convert_content_block).collect();
                api_messages.push(ApiMessage {
                    role: "user".to_string(),
                    content,
                });
            }
            AgentMessage::Assistant(assistant) => {
                let content = assistant.content.iter().map(convert_content_block).collect();
                api_messages.push(ApiMessage {
                    role: "assistant".to_string(),
                    content,
                });
            }
            AgentMessage::ToolResult(result) => {
                // Tool results go as user messages with tool_result content blocks
                let content_blocks = result.content.iter().map(convert_content_block).collect();
                api_messages.push(ApiMessage {
                    role: "user".to_string(),
                    content: vec![ApiContentBlock::ToolResult {
                        tool_use_id: result.call_id.clone(),
                        content: content_blocks,
                        is_error: if result.is_error { Some(true) } else { None },
                    }],
                });
            }
            // Skip metadata messages (BashExecution, Custom, BranchSummary, CompactionSummary)
            // — they're not sent to the LLM
            _ => {}
        }
    }

    api_messages
}

fn convert_content_block(content: &crate::message::Content) -> ApiContentBlock {
    use crate::message::Content;
    use crate::message::ImageSource;
    match content {
        Content::Text { text } => ApiContentBlock::Text { text: text.clone() },
        Content::Image { source } => match source {
            ImageSource::Base64 { media_type, data } => ApiContentBlock::Image {
                source: ApiImageSource {
                    source_type: "base64".to_string(),
                    media_type: media_type.clone(),
                    data: data.clone(),
                },
            },
            ImageSource::Url { url } => {
                // Anthropic doesn't support URL images directly, convert to text fallback
                ApiContentBlock::Text {
                    text: format!("[Image URL: {}]", url),
                }
            }
        },
        Content::Thinking { signature, .. } => ApiContentBlock::Thinking {
            // Anthropic requires thinking text to be redacted when echoing back;
            // only the opaque signature is sent.
            thinking: String::new(),
            signature: signature.clone(),
        },
        Content::ToolUse { id, name, input } => ApiContentBlock::ToolUse {
            id: id.clone(),
            name: name.clone(),
            input: input.clone(),
        },
        Content::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => {
            let blocks = content.iter().map(convert_content_block).collect();
            ApiContentBlock::ToolResult {
                tool_use_id: tool_use_id.clone(),
                content: blocks,
                is_error: *is_error,
            }
        }
    }
}

fn convert_tools(tools: &[clankers_router::provider::ToolDefinition]) -> Vec<ApiTool> {
    let mut api_tools: Vec<_> = tools
        .iter()
        .map(|t| ApiTool {
            name: t.name.clone(),
            description: t.description.clone(),
            input_schema: t.input_schema.clone(),
            cache_control: None,
        })
        .collect();

    // Add cache_control to the LAST tool (Anthropic caching convention)
    if let Some(last) = api_tools.last_mut() {
        last.cache_control = Some(CacheControl::ephemeral());
    }

    api_tools
}
