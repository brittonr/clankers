//! Plain LLM contract types shared by message, router, provider, and engine crates.
//!
//! This module intentionally contains only serde-friendly data contracts. It must
//! not depend on provider implementations, router runtime services, async runtimes,
//! databases, network clients, daemon protocols, or UI crates.

use serde::Deserialize;
use serde::Serialize;

/// Tool definition for function calling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Configuration for extended thinking mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingConfig {
    /// Whether extended thinking is enabled.
    pub enabled: bool,
    /// Maximum tokens for thinking.
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
    #[cfg_attr(
        dylint_lib = "tigerstyle",
        allow(
            tigerstyle::usize_in_public_api,
            reason = "Usage token counts mirror existing usize fields and internal UI metrics."
        )
    )]
    pub fn total_tokens(&self) -> usize {
        self.input_tokens.saturating_add(self.output_tokens)
    }
}
