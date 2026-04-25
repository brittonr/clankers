//! Core provider trait and request/response types
//!
//! Each LLM backend (Anthropic, OpenAI, OpenRouter, etc.) implements the
//! [`Provider`] trait to expose a unified streaming completion interface.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use async_trait::async_trait;
pub use clanker_message::ThinkingConfig;
pub use clanker_message::ToolDefinition;
pub use clanker_message::Usage;
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

/// Cost breakdown for a completion.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Cost {
    pub input_cost: f64,
    pub output_cost: f64,
    pub total_cost: f64,
}
