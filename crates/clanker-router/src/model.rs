//! Model definitions and capabilities

use serde::Deserialize;
use serde::Serialize;

/// Model configuration and capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    /// Model identifier (e.g., "claude-opus-4-6")
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Provider name (e.g., "anthropic", "openai")
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
    /// Estimate cost for a given usage.
    ///
    /// # Tiger Style
    ///
    /// Asserts positive cost rates and returns `None` if the result would
    /// be non-finite (NaN/Inf from pathological inputs).
    pub fn estimate_cost(&self, input_tokens: usize, output_tokens: usize) -> Option<f64> {
        let input_rate = self.input_cost_per_mtok?;
        let output_rate = self.output_cost_per_mtok?;

        // Tiger Style: rates must be non-negative.
        debug_assert!(input_rate >= 0.0, "input_cost_per_mtok must be non-negative");
        debug_assert!(output_rate >= 0.0, "output_cost_per_mtok must be non-negative");

        let input_cost = input_rate * (input_tokens as f64 / 1_000_000.0);
        let output_cost = output_rate * (output_tokens as f64 / 1_000_000.0);
        let total = input_cost + output_cost;

        // Tiger Style: reject non-finite results (defense against NaN/Inf propagation).
        if total.is_finite() { Some(total) } else { None }
    }
}

/// Named aliases for common models
pub struct ModelAliases;

impl ModelAliases {
    /// Resolve a model alias to a full model ID.
    /// Returns `None` if the alias is not recognized (may be a full ID already).
    pub fn resolve(alias: &str) -> Option<&'static str> {
        match alias {
            "sonnet" | "claude-sonnet" | "claude-sonnet-4-5" => Some("claude-sonnet-4-5-20250514"),
            "opus" | "claude-opus" | "claude-opus-4" | "opus-4-6" | "claude-opus-4-6" => Some("claude-opus-4-20250514"),
            "haiku" | "claude-haiku" | "claude-haiku-4-5" => Some("claude-haiku-4-5-20250514"),
            "gpt-4o" | "4o" => Some("gpt-4o"),
            "gpt-4o-mini" | "4o-mini" => Some("gpt-4o-mini"),
            "o1" => Some("o1"),
            "o1-mini" => Some("o1-mini"),
            "o3" => Some("o3"),
            "o3-mini" => Some("o3-mini"),
            "gemini-pro" | "gemini-2.5-pro" => Some("gemini-2.5-pro-preview-05-06"),
            "gemini-flash" | "gemini-2.5-flash" => Some("gemini-2.5-flash-preview-05-20"),
            "deepseek" | "deepseek-v3" => Some("deepseek-chat"),
            "deepseek-r1" => Some("deepseek-reasoner"),
            "llama-70b" | "hf-llama" => Some("meta-llama/Llama-3.3-70B-Instruct"),
            "qwen-72b" | "hf-qwen" => Some("Qwen/Qwen2.5-72B-Instruct"),
            "qwen-coder" | "hf-coder" => Some("Qwen/Qwen2.5-Coder-32B-Instruct"),
            "phi-4" | "hf-phi" => Some("microsoft/Phi-4"),
            "gemma-27b" | "hf-gemma" => Some("google/gemma-2-27b-it"),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_estimate_cost() {
        let model = Model {
            id: "test".into(),
            name: "Test".into(),
            provider: "test".into(),
            max_input_tokens: 200_000,
            max_output_tokens: 16_384,
            supports_thinking: false,
            supports_images: false,
            supports_tools: true,
            input_cost_per_mtok: Some(3.0),
            output_cost_per_mtok: Some(15.0),
        };
        let cost = model.estimate_cost(1_000_000, 1_000).unwrap();
        assert!((cost - 3.015).abs() < 0.001);
    }

    #[test]
    fn test_model_estimate_cost_none() {
        let model = Model {
            id: "test".into(),
            name: "Test".into(),
            provider: "test".into(),
            max_input_tokens: 200_000,
            max_output_tokens: 16_384,
            supports_thinking: false,
            supports_images: false,
            supports_tools: true,
            input_cost_per_mtok: None,
            output_cost_per_mtok: None,
        };
        assert!(model.estimate_cost(1000, 1000).is_none());
    }

    #[test]
    fn test_aliases() {
        assert_eq!(ModelAliases::resolve("sonnet"), Some("claude-sonnet-4-5-20250514"));
        assert_eq!(ModelAliases::resolve("opus"), Some("claude-opus-4-20250514"));
        assert_eq!(ModelAliases::resolve("gpt-4o"), Some("gpt-4o"));
        assert_eq!(ModelAliases::resolve("unknown-model"), None);
    }
}
