//! Core provider trait and request/response types
//!
//! Each LLM backend (Anthropic, OpenAI, OpenRouter, etc.) implements the
//! [`Provider`] trait to expose a unified streaming completion interface.

use async_trait::async_trait;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::mpsc;

use crate::error::Result;
use crate::model::Model;
use crate::streaming::StreamEvent;

/// Provider trait for LLM API implementations.
///
/// Each provider implements this trait to expose a unified interface for
/// model completion requests with streaming responses.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Send a completion request and stream the response via the provided channel.
    async fn complete(&self, request: CompletionRequest, tx: mpsc::Sender<StreamEvent>) -> Result<()>;

    /// Returns the list of models supported by this provider.
    fn models(&self) -> &[Model];

    /// Returns the provider's unique name (e.g., "anthropic", "openai").
    fn name(&self) -> &str;

    /// Reload credentials from disk (e.g. after a fresh `/login`).
    ///
    /// Default implementation is a no-op.
    async fn reload_credentials(&self) {}

    /// Check if the provider has valid credentials configured.
    async fn is_available(&self) -> bool {
        true
    }
}

/// Tool definition for function calling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Request for a model completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    /// Model identifier to use
    pub model: String,

    /// Conversation messages
    pub messages: Vec<serde_json::Value>,

    /// System prompt
    pub system_prompt: Option<String>,

    /// Maximum tokens to generate
    pub max_tokens: Option<usize>,

    /// Sampling temperature (0.0-1.0)
    pub temperature: Option<f64>,

    /// Available tools
    pub tools: Vec<ToolDefinition>,

    /// Extended thinking configuration
    pub thinking: Option<ThinkingConfig>,

    /// Disable prompt caching (skip cache_control breakpoints)
    #[serde(default)]
    pub no_cache: bool,

    /// Cache TTL override (e.g. "1h" for 1-hour cache). None = default 5m ephemeral.
    #[serde(default)]
    pub cache_ttl: Option<String>,

    /// Extra provider-specific parameters passed through verbatim.
    ///
    /// Parameters like `response_format`, `seed`, `top_p`, `frequency_penalty`,
    /// `presence_penalty`, `logprobs`, `top_logprobs`, `n`, `stop`, etc. are
    /// forwarded as-is to the upstream provider. This avoids silently dropping
    /// parameters that clients (Cursor, aider, Continue) send.
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub extra_params: std::collections::HashMap<String, serde_json::Value>,
}

/// Configuration for extended thinking mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingConfig {
    /// Whether extended thinking is enabled
    pub enabled: bool,
    /// Maximum tokens for thinking
    pub budget_tokens: Option<usize>,
}

/// Token usage statistics for a completion.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub cache_creation_input_tokens: usize,
    pub cache_read_input_tokens: usize,
}

impl Usage {
    pub fn total_tokens(&self) -> usize {
        self.input_tokens + self.output_tokens
    }
}

/// Cost breakdown for a completion.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Cost {
    pub input_cost: f64,
    pub output_cost: f64,
    pub total_cost: f64,
}
