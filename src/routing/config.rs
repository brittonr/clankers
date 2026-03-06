//! Routing policy configuration

use std::collections::HashMap;

use serde::Deserialize;
use serde::Serialize;

/// Configuration for the routing policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingPolicyConfig {
    /// Enable/disable routing (default: true)
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Score threshold below which "smol" role is selected (default: 20.0)
    #[serde(default = "default_low_threshold")]
    pub low_threshold: f32,

    /// Score threshold above which "slow" role is selected (default: 50.0)
    #[serde(default = "default_high_threshold")]
    pub high_threshold: f32,

    /// Weight for token count in scoring (default: 1.0)
    #[serde(default = "default_weight")]
    pub token_weight: f32,

    /// Weight for tool complexity in scoring (default: 1.0)
    #[serde(default = "default_weight")]
    pub tool_weight: f32,

    /// Keyword hints: map of keyword → complexity adjustment
    #[serde(default = "default_keyword_hints")]
    pub keyword_hints: HashMap<String, f32>,

    /// Soft budget limit (USD) — bias toward cheaper models when exceeded
    #[serde(default)]
    pub budget_soft_limit: Option<f64>,

    /// Hard budget limit (USD) — force cheapest model when exceeded
    #[serde(default)]
    pub budget_hard_limit: Option<f64>,

    /// Enable multi-model orchestration (experimental, default: false)
    #[serde(default)]
    pub enable_orchestration: bool,
}

impl Default for RoutingPolicyConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            low_threshold: default_low_threshold(),
            high_threshold: default_high_threshold(),
            token_weight: default_weight(),
            tool_weight: default_weight(),
            keyword_hints: default_keyword_hints(),
            budget_soft_limit: None,
            budget_hard_limit: None,
            enable_orchestration: false,
        }
    }
}

fn default_enabled() -> bool {
    true
}

fn default_low_threshold() -> f32 {
    20.0
}

fn default_high_threshold() -> f32 {
    50.0
}

fn default_weight() -> f32 {
    1.0
}

fn default_keyword_hints() -> HashMap<String, f32> {
    let mut hints = HashMap::new();

    // Complexity increasers
    hints.insert("refactor".to_string(), 10.0);
    hints.insert("architecture".to_string(), 15.0);
    hints.insert("design".to_string(), 10.0);
    hints.insert("complex".to_string(), 10.0);
    hints.insert("optimize".to_string(), 8.0);
    hints.insert("analyze".to_string(), 8.0);
    hints.insert("debug".to_string(), 8.0);
    hints.insert("security".to_string(), 12.0);

    // Complexity reducers
    hints.insert("list".to_string(), -5.0);
    hints.insert("show".to_string(), -5.0);
    hints.insert("read".to_string(), -5.0);
    hints.insert("grep".to_string(), -8.0);
    hints.insert("find".to_string(), -8.0);
    hints.insert("quick".to_string(), -10.0);
    hints.insert("simple".to_string(), -8.0);

    hints
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RoutingPolicyConfig::default();
        assert!(config.enabled);
        assert_eq!(config.low_threshold, 20.0);
        assert_eq!(config.high_threshold, 50.0);
        assert_eq!(config.token_weight, 1.0);
        assert_eq!(config.tool_weight, 1.0);
        assert!(!config.keyword_hints.is_empty());
    }

    #[test]
    fn test_default_keyword_hints() {
        let hints = default_keyword_hints();
        assert!(hints.get("refactor").unwrap() > &0.0);
        assert!(hints.get("quick").unwrap() < &0.0);
        assert!(hints.get("architecture").unwrap() > &0.0);
        assert!(hints.get("grep").unwrap() < &0.0);
    }

    #[test]
    fn test_config_serialization() {
        let config = RoutingPolicyConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let decoded: RoutingPolicyConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.enabled, config.enabled);
        assert_eq!(decoded.low_threshold, config.low_threshold);
    }
}
