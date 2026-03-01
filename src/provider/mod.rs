//! LLM provider abstraction
//!
//! This module defines the core provider trait and types for interacting with
//! language model APIs. It supports streaming responses, multiple content types,
//! tool use, and extended thinking modes.

use async_trait::async_trait;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::mpsc;

use crate::error::Result;

pub mod anthropic;
pub mod auth;
pub mod credential_manager;
pub mod message;
pub mod registry;
pub mod retry;
pub mod router;
pub mod rpc_provider;
pub mod streaming;

/// Provider trait for LLM API implementations.
///
/// Each provider (Anthropic, OpenAI, etc.) implements this trait to expose
/// a unified interface for model completion requests.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Send a completion request and stream the response via the provided channel.
    ///
    /// The provider should send [`StreamEvent`](streaming::StreamEvent) items as they arrive,
    /// and close the channel when the response is complete.
    async fn complete(&self, request: CompletionRequest, tx: mpsc::Sender<streaming::StreamEvent>) -> Result<()>;

    /// Returns the list of models supported by this provider.
    fn models(&self) -> &[Model];

    /// Returns the provider's unique name (e.g., "anthropic", "openai").
    fn name(&self) -> &str;

    /// Reload credentials from disk (e.g. after a fresh `/login`).
    ///
    /// Default implementation is a no-op. Providers with a `CredentialManager`
    /// override this to re-read the auth store and update in-memory state.
    async fn reload_credentials(&self) {}
}

/// Model configuration and capabilities.
///
/// Describes a specific model's identifier, token limits, and feature support.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    /// Model identifier (e.g., "claude-opus-4-20250514")
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Provider name
    pub provider: String,

    /// Maximum input tokens (context window)
    pub max_input_tokens: usize,

    /// Maximum output tokens per response
    pub max_output_tokens: usize,

    /// Whether the model supports extended thinking mode
    pub supports_thinking: bool,

    /// Whether the model supports image inputs
    pub supports_images: bool,

    /// Whether the model supports tool use
    #[serde(default = "default_true")]
    pub supports_tools: bool,

    /// Cost per million input tokens (USD), if known
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_cost_per_mtok: Option<f64>,

    /// Cost per million output tokens (USD), if known
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_cost_per_mtok: Option<f64>,
}

fn default_true() -> bool {
    true
}

impl Model {
    /// Convert to a clankers-router Model
    pub fn to_router_model(&self) -> clankers_router::Model {
        clankers_router::Model {
            id: self.id.clone(),
            name: self.name.clone(),
            provider: self.provider.clone(),
            max_input_tokens: self.max_input_tokens,
            max_output_tokens: self.max_output_tokens,
            supports_thinking: self.supports_thinking,
            supports_images: self.supports_images,
            supports_tools: self.supports_tools,
            input_cost_per_mtok: self.input_cost_per_mtok,
            output_cost_per_mtok: self.output_cost_per_mtok,
        }
    }

    /// Create from a clankers-router Model
    pub fn from_router_model(m: &clankers_router::Model) -> Self {
        Self {
            id: m.id.clone(),
            name: m.name.clone(),
            provider: m.provider.clone(),
            max_input_tokens: m.max_input_tokens,
            max_output_tokens: m.max_output_tokens,
            supports_thinking: m.supports_thinking,
            supports_images: m.supports_images,
            supports_tools: m.supports_tools,
            input_cost_per_mtok: m.input_cost_per_mtok,
            output_cost_per_mtok: m.output_cost_per_mtok,
        }
    }

    /// Estimate cost for a given usage
    pub fn estimate_cost(&self, input_tokens: usize, output_tokens: usize) -> Option<f64> {
        let input_cost = self.input_cost_per_mtok? * (input_tokens as f64 / 1_000_000.0);
        let output_cost = self.output_cost_per_mtok? * (output_tokens as f64 / 1_000_000.0);
        Some(input_cost + output_cost)
    }
}

/// Request for a model completion.
///
/// Contains all parameters needed to invoke a model, including messages,
/// system prompt, sampling parameters, and tool definitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    /// Model identifier to use
    pub model: String,

    /// Conversation messages (user, assistant, tool results, etc.)
    pub messages: Vec<message::AgentMessage>,

    /// System prompt (provider-dependent placement)
    pub system_prompt: Option<String>,

    /// Maximum tokens to generate (None = model default)
    pub max_tokens: Option<usize>,

    /// Sampling temperature (typically 0.0-1.0)
    pub temperature: Option<f64>,

    /// Available tools for the model to call
    pub tools: Vec<crate::tools::ToolDefinition>,

    /// Extended thinking configuration (if supported)
    pub thinking: Option<ThinkingConfig>,
}

/// Configuration for extended thinking mode.
///
/// Enables models to spend additional tokens on internal reasoning before
/// generating the final response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingConfig {
    /// Whether extended thinking is enabled
    pub enabled: bool,

    /// Maximum tokens to allocate for thinking (None = model default)
    pub budget_tokens: Option<usize>,
}

/// Named thinking budget levels.
///
/// Provides quick presets for thinking token budgets that can be cycled
/// through with a keybinding or set via `/think <level>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThinkingLevel {
    /// Thinking disabled
    Off,
    /// Quick reasoning (~5k tokens)
    Low,
    /// Moderate reasoning (~10k tokens, default)
    Medium,
    /// Deep reasoning (~32k tokens)
    High,
    /// Maximum reasoning (~128k tokens)
    Max,
}

impl ThinkingLevel {
    /// Token budget for this level (None for Off)
    pub fn budget_tokens(self) -> Option<usize> {
        match self {
            Self::Off => None,
            Self::Low => Some(5_000),
            Self::Medium => Some(10_000),
            Self::High => Some(32_000),
            Self::Max => Some(128_000),
        }
    }

    /// Whether thinking is enabled at this level
    pub fn is_enabled(self) -> bool {
        self != Self::Off
    }

    /// Cycle to the next level
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::Low,
            Self::Low => Self::Medium,
            Self::Medium => Self::High,
            Self::High => Self::Max,
            Self::Max => Self::Off,
        }
    }

    /// Display name
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Max => "max",
        }
    }

    /// Parse from a string (name or raw number)
    pub fn from_str_or_budget(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "off" | "none" | "disable" | "disabled" => Some(Self::Off),
            "low" | "lo" | "l" => Some(Self::Low),
            "medium" | "med" | "m" | "default" => Some(Self::Medium),
            "high" | "hi" | "h" => Some(Self::High),
            "max" | "maximum" | "full" => Some(Self::Max),
            _ => None,
        }
    }

    /// Find the closest level for a raw token budget
    pub fn from_budget(tokens: usize) -> Self {
        if tokens == 0 {
            Self::Off
        } else if tokens <= 5_000 {
            Self::Low
        } else if tokens <= 10_000 {
            Self::Medium
        } else if tokens <= 32_000 {
            Self::High
        } else {
            Self::Max
        }
    }

    /// Convert to ThinkingConfig
    pub fn to_config(self) -> Option<ThinkingConfig> {
        if self.is_enabled() {
            Some(ThinkingConfig {
                enabled: true,
                budget_tokens: self.budget_tokens(),
            })
        } else {
            None
        }
    }

    /// All levels in order
    pub fn all() -> &'static [Self] {
        &[Self::Off, Self::Low, Self::Medium, Self::High, Self::Max]
    }
}

/// Token usage statistics for a completion.
///
/// Tracks input, output, and cache-related token counts.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    /// Tokens in the input (prompt)
    pub input_tokens: usize,

    /// Tokens in the output (completion)
    pub output_tokens: usize,

    /// Tokens written to cache (if prompt caching is used)
    pub cache_creation_input_tokens: usize,

    /// Tokens read from cache (if prompt caching is used)
    pub cache_read_input_tokens: usize,
}

impl Usage {
    /// Returns the total token count (input + output).
    pub fn total_tokens(&self) -> usize {
        self.input_tokens + self.output_tokens
    }
}

/// Cost breakdown for a completion.
///
/// Tracks estimated or actual costs in USD.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Cost {
    /// Cost for input tokens
    pub input_cost: f64,

    /// Cost for output tokens
    pub output_cost: f64,

    /// Total cost (input + output)
    pub total_cost: f64,
}
