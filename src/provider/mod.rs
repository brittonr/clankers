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

// Re-export Model from clankers-router (canonical definition)
pub use clankers_router::Model;

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

// Re-export ThinkingConfig from clankers-router (canonical definition)
pub use clankers_router::ThinkingConfig;

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

// Re-export Usage and Cost from clankers-router (canonical definitions)
pub use clankers_router::Usage;
pub use clankers_router::provider::Cost;
