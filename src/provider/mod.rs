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
pub mod discovery;
pub mod message;
/// Model registry — re-exported from `clankers-router`.
pub use clankers_router::registry;
/// Retry logic — re-exported from `clankers-router`.
pub use clankers_router::retry;
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
// ThinkingLevel re-exported from clankers-tui-types (canonical definition).
pub use clankers_tui_types::ThinkingLevel;

/// Extension: convert ThinkingLevel to provider-specific ThinkingConfig.
pub fn thinking_level_to_config(level: ThinkingLevel) -> Option<ThinkingConfig> {
    if level.is_enabled() {
        Some(ThinkingConfig {
            enabled: true,
            budget_tokens: level.budget_tokens(),
        })
    } else {
        None
    }
}

// Re-export Usage and Cost from clankers-router (canonical definitions)
pub use clankers_router::Usage;
pub use clankers_router::provider::Cost;
