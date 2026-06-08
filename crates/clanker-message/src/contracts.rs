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

/// Metadata about an available tool for inventory/projection surfaces.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    /// Source of the tool: "built-in" or plugin name.
    #[serde(default)]
    pub source: String,
}

/// Named thinking budget levels shared by provider, controller, and display edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThinkingLevel {
    /// Thinking disabled.
    Off,
    /// Quick reasoning (~5k tokens).
    Low,
    /// Moderate reasoning (~10k tokens).
    Medium,
    /// Deep reasoning (~32k tokens).
    High,
    /// Maximum reasoning (~128k tokens).
    Max,
}

impl ThinkingLevel {
    /// Token budget for this level (None for Off).
    pub const fn budget_tokens(self) -> Option<u32> {
        match self {
            Self::Off => None,
            Self::Low => Some(5_000),
            Self::Medium => Some(10_000),
            Self::High => Some(32_000),
            Self::Max => Some(128_000),
        }
    }

    /// Whether thinking is enabled at this level.
    pub const fn is_enabled(self) -> bool {
        !matches!(self, Self::Off)
    }

    /// Cycle to the next level.
    pub const fn next(self) -> Self {
        match self {
            Self::Off => Self::Low,
            Self::Low => Self::Medium,
            Self::Medium => Self::High,
            Self::High => Self::Max,
            Self::Max => Self::Off,
        }
    }

    /// Display name.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Max => "max",
        }
    }

    /// Parse from a string level name.
    pub fn from_str_or_budget(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "off" | "none" | "disable" | "disabled" => Some(Self::Off),
            "low" | "lo" | "l" => Some(Self::Low),
            "medium" | "med" | "m" => Some(Self::Medium),
            "high" | "hi" | "h" => Some(Self::High),
            "xhigh" | "x-high" | "extra-high" | "max" | "maximum" | "full" | "default" => Some(Self::Max),
            _ => None,
        }
    }

    /// Find the closest level for a raw token budget.
    pub const fn from_budget(tokens: u32) -> Self {
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

    /// All levels in order.
    pub const fn all() -> &'static [Self] {
        &[Self::Off, Self::Low, Self::Medium, Self::High, Self::Max]
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_info_defaults_missing_source_for_legacy_wire_events() {
        let info: ToolInfo = serde_json::from_str(r#"{"name":"read","description":"Read files"}"#)
            .expect("tool info should deserialize");
        assert_eq!(info.source, "");
    }
}
