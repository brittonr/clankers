//! Routing policy implementation

use std::fmt;

use super::config::RoutingPolicyConfig;
use super::orchestration::OrchestrationPlan;
use super::signals::ComplexitySignals;
use super::signals::ModelRoleHint;
use super::signals::ToolComplexity;

/// Main routing policy that selects models based on complexity signals
#[derive(Debug, Clone)]
pub struct RoutingPolicy {
    config: RoutingPolicyConfig,
}

impl RoutingPolicy {
    /// Create a new routing policy with the given configuration
    pub fn new(config: RoutingPolicyConfig) -> Self {
        Self { config }
    }

    /// Create a routing policy with default configuration
    pub fn default_policy() -> Self {
        Self::new(RoutingPolicyConfig::default())
    }

    /// Select the appropriate model role based on complexity signals
    pub fn select_model(&self, signals: &ComplexitySignals) -> ModelSelectionResult {
        // If disabled, always return default
        if !self.config.enabled {
            return ModelSelectionResult {
                role: "default".to_string(),
                score: 0.0,
                reason: SelectionReason::Default,
                orchestration: None,
            };
        }

        // Hard budget limit — force smol regardless of complexity
        if let Some(hard) = self.config.budget_hard_limit
            && signals.current_cost >= hard
        {
            return ModelSelectionResult {
                role: "smol".to_string(),
                score: 0.0,
                reason: SelectionReason::BudgetThreshold {
                    limit: hard,
                    current: signals.current_cost,
                },
                orchestration: None,
            };
        }

        // User hints override complexity scoring (but not hard budget)
        if let Some(hint) = &signals.user_hint {
            let role = match hint {
                ModelRoleHint::Explicit(name) => name.clone(),
                ModelRoleHint::Fast => "smol".to_string(),
                ModelRoleHint::Thorough => "slow".to_string(),
            };
            return ModelSelectionResult {
                role,
                score: 0.0,
                reason: SelectionReason::UserRequested,
                orchestration: None,
            };
        }

        // Compute complexity score
        let score = self.compute_complexity_score(signals);

        // Apply soft budget pressure — reduce score to bias toward cheaper models
        let adjusted_score = if let Some(soft) = self.config.budget_soft_limit {
            if signals.current_cost >= soft {
                score * 0.5 // halve the score — shifts selection toward smol/default
            } else {
                score
            }
        } else {
            score
        };

        // Select role based on thresholds
        let role = if adjusted_score < self.config.low_threshold {
            "smol".to_string()
        } else if adjusted_score > self.config.high_threshold {
            "slow".to_string()
        } else {
            "default".to_string()
        };

        // Check for orchestration (experimental)
        let orchestration = if self.config.enable_orchestration {
            super::orchestration::detect_pattern(signals.prompt_text.as_deref().unwrap_or(""), adjusted_score)
        } else {
            None
        };

        ModelSelectionResult {
            role,
            score: adjusted_score,
            reason: SelectionReason::ComplexityScore(adjusted_score),
            orchestration,
        }
    }

    /// Compute a numeric complexity score from signals
    pub fn compute_complexity_score(&self, signals: &ComplexitySignals) -> f32 {
        let mut score = 0.0;

        // Token count contribution (normalized to ~0-20 range)
        // Typical prompts: 50-500 tokens
        let token_contribution = (signals.token_count as f32 / 50.0) * self.config.token_weight;
        score += token_contribution;

        // Tool complexity contribution
        let tool_contribution: f32 =
            signals.recent_tools.iter().map(|t| t.complexity.weight()).sum::<f32>() * self.config.tool_weight;
        score += tool_contribution;

        // Keyword contributions
        let keyword_contribution: f32 = signals.keywords.iter().map(|(_, weight)| weight).sum();
        score += keyword_contribution;

        score
    }

    /// Extract keywords and their weights from prompt text
    pub fn extract_keywords(&self, text: &str) -> Vec<(String, f32)> {
        let lower = text.to_lowercase();
        let mut found = Vec::new();

        for (keyword, weight) in &self.config.keyword_hints {
            if lower.contains(keyword.as_str()) {
                found.push((keyword.clone(), *weight));
            }
        }

        found
    }

    /// Parse user hint from prompt text
    pub fn parse_user_hint(&self, text: &str) -> Option<ModelRoleHint> {
        let lower = text.to_lowercase();

        // Check for explicit role mentions
        if lower.contains("use opus") || lower.contains("switch to opus") {
            return Some(ModelRoleHint::Explicit("slow".to_string()));
        }
        if lower.contains("use haiku") || lower.contains("switch to haiku") {
            return Some(ModelRoleHint::Explicit("smol".to_string()));
        }
        if lower.contains("use sonnet") || lower.contains("switch to sonnet") {
            return Some(ModelRoleHint::Explicit("default".to_string()));
        }

        // Check for fast/thorough hints
        if lower.contains("quick answer") || lower.contains("quickly") || lower.contains("fast response") {
            return Some(ModelRoleHint::Fast);
        }
        if lower.contains("think deeply")
            || lower.contains("carefully")
            || lower.contains("thorough")
            || lower.contains("detailed analysis")
        {
            return Some(ModelRoleHint::Thorough);
        }

        None
    }

    /// Classify a tool by name into a complexity tier
    pub fn classify_tool(&self, tool_name: &str) -> ToolComplexity {
        match tool_name {
            "read" | "ls" | "grep" | "find" | "glob" | "rg" => ToolComplexity::Simple,
            "bash" | "edit" | "write" | "patch" => ToolComplexity::Medium,
            "subagent" | "delegate" | "delegate_task" | "agent" => ToolComplexity::Complex,
            _ => ToolComplexity::Medium, // default to medium for unknown tools
        }
    }
}

/// Result of model selection
#[derive(Debug, Clone)]
pub struct ModelSelectionResult {
    /// Selected role name (e.g. "default", "smol", "slow")
    pub role: String,
    /// Complexity score (if reason is ComplexityScore)
    pub score: f32,
    /// Why this role was selected
    pub reason: SelectionReason,
    /// If set, run an orchestrated multi-model turn instead of a single model
    pub orchestration: Option<OrchestrationPlan>,
}

/// Reason for model selection
#[derive(Debug, Clone)]
pub enum SelectionReason {
    /// Selected based on computed complexity score
    ComplexityScore(f32),
    /// User explicitly requested this model
    UserRequested,
    /// Policy is disabled, using default
    Default,
    /// Budget threshold forced downgrade
    BudgetThreshold { limit: f64, current: f64 },
}

impl fmt::Display for SelectionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ComplexityScore(score) => write!(f, "complexity_score={:.1}", score),
            Self::UserRequested => write!(f, "user_requested"),
            Self::Default => write!(f, "default"),
            Self::BudgetThreshold { limit, current } => {
                write!(f, "budget_threshold(${:.2}/${:.2})", current, limit)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_policy_creation() {
        let policy = RoutingPolicy::default_policy();
        assert!(policy.config.enabled);
    }

    #[test]
    fn test_disabled_policy_returns_default() {
        let mut config = RoutingPolicyConfig::default();
        config.enabled = false;
        let policy = RoutingPolicy::new(config);

        let signals = ComplexitySignals {
            token_count: 10000,
            ..Default::default()
        };

        let result = policy.select_model(&signals);
        assert_eq!(result.role, "default");
        assert!(matches!(result.reason, SelectionReason::Default));
    }

    #[test]
    fn test_low_complexity_selects_smol() {
        let policy = RoutingPolicy::default_policy();

        let signals = ComplexitySignals {
            token_count: 50, // Small prompt
            recent_tools: vec![],
            keywords: vec![("simple".to_string(), -8.0)],
            user_hint: None,
            current_cost: 0.0,
            prompt_text: None,
        };

        let result = policy.select_model(&signals);
        assert_eq!(result.role, "smol");
        assert!(result.score < 20.0);
    }

    #[test]
    fn test_high_complexity_selects_slow() {
        let policy = RoutingPolicy::default_policy();

        let signals = ComplexitySignals {
            token_count: 2000, // Large prompt
            recent_tools: vec![],
            keywords: vec![("architecture".to_string(), 15.0), ("refactor".to_string(), 10.0)],
            user_hint: None,
            current_cost: 0.0,
            prompt_text: None,
        };

        let result = policy.select_model(&signals);
        assert_eq!(result.role, "slow");
        assert!(result.score > 50.0);
    }

    #[test]
    fn test_medium_complexity_selects_default() {
        let policy = RoutingPolicy::default_policy();

        // 1500 tokens / 50 * 1.0 = 30.0, plus "optimize" keyword = +8.0 = 38.0
        // Between low_threshold (20) and high_threshold (50) → default
        let signals = ComplexitySignals {
            token_count: 1500,
            recent_tools: vec![],
            keywords: vec![("optimize".to_string(), 8.0)],
            user_hint: None,
            current_cost: 0.0,
            prompt_text: None,
        };

        let result = policy.select_model(&signals);
        assert_eq!(result.role, "default");
        assert!(result.score >= 20.0 && result.score <= 50.0);
    }

    #[test]
    fn test_user_hint_fast_overrides() {
        let policy = RoutingPolicy::default_policy();

        let signals = ComplexitySignals {
            token_count: 5000, // Would normally be slow
            recent_tools: vec![],
            keywords: vec![("architecture".to_string(), 15.0)],
            user_hint: Some(ModelRoleHint::Fast),
            current_cost: 0.0,
            prompt_text: None,
        };

        let result = policy.select_model(&signals);
        assert_eq!(result.role, "smol");
        assert!(matches!(result.reason, SelectionReason::UserRequested));
    }

    #[test]
    fn test_user_hint_thorough_overrides() {
        let policy = RoutingPolicy::default_policy();

        let signals = ComplexitySignals {
            token_count: 50, // Would normally be smol
            recent_tools: vec![],
            keywords: vec![],
            user_hint: Some(ModelRoleHint::Thorough),
            current_cost: 0.0,
            prompt_text: None,
        };

        let result = policy.select_model(&signals);
        assert_eq!(result.role, "slow");
        assert!(matches!(result.reason, SelectionReason::UserRequested));
    }

    #[test]
    fn test_user_hint_explicit_role() {
        let policy = RoutingPolicy::default_policy();

        let signals = ComplexitySignals {
            token_count: 200,
            recent_tools: vec![],
            keywords: vec![],
            user_hint: Some(ModelRoleHint::Explicit("slow".to_string())),
            current_cost: 0.0,
            prompt_text: None,
        };

        let result = policy.select_model(&signals);
        assert_eq!(result.role, "slow");
    }

    #[test]
    fn test_keyword_extraction() {
        let policy = RoutingPolicy::default_policy();
        let text = "refactor this code to improve architecture";
        let keywords = policy.extract_keywords(text);

        assert!(keywords.iter().any(|(k, _)| k == "refactor"));
        assert!(keywords.iter().any(|(k, _)| k == "architecture"));
    }

    #[test]
    fn test_parse_user_hint_quick_answer() {
        let policy = RoutingPolicy::default_policy();
        let hint = policy.parse_user_hint("Give me a quick answer");
        assert_eq!(hint, Some(ModelRoleHint::Fast));
    }

    #[test]
    fn test_parse_user_hint_think_deeply() {
        let policy = RoutingPolicy::default_policy();
        let hint = policy.parse_user_hint("Think deeply about this problem");
        assert_eq!(hint, Some(ModelRoleHint::Thorough));
    }

    #[test]
    fn test_parse_user_hint_use_opus() {
        let policy = RoutingPolicy::default_policy();
        let hint = policy.parse_user_hint("use opus for this");
        assert!(matches!(
            hint,
            Some(ModelRoleHint::Explicit(ref s)) if s == "slow"
        ));
    }

    #[test]
    fn test_parse_user_hint_none() {
        let policy = RoutingPolicy::default_policy();
        let hint = policy.parse_user_hint("normal prompt");
        assert_eq!(hint, None);
    }

    #[test]
    fn test_tool_complexity_classification() {
        let policy = RoutingPolicy::default_policy();

        assert_eq!(policy.classify_tool("read"), ToolComplexity::Simple);
        assert_eq!(policy.classify_tool("ls"), ToolComplexity::Simple);
        assert_eq!(policy.classify_tool("grep"), ToolComplexity::Simple);
        assert_eq!(policy.classify_tool("find"), ToolComplexity::Simple);

        assert_eq!(policy.classify_tool("bash"), ToolComplexity::Medium);
        assert_eq!(policy.classify_tool("edit"), ToolComplexity::Medium);
        assert_eq!(policy.classify_tool("write"), ToolComplexity::Medium);

        assert_eq!(policy.classify_tool("subagent"), ToolComplexity::Complex);
        assert_eq!(policy.classify_tool("delegate"), ToolComplexity::Complex);
        assert_eq!(policy.classify_tool("delegate_task"), ToolComplexity::Complex);

        // Unknown tools default to medium
        assert_eq!(policy.classify_tool("unknown_tool"), ToolComplexity::Medium);
    }

    #[test]
    fn test_hard_budget_forces_smol() {
        let mut config = RoutingPolicyConfig::default();
        config.budget_hard_limit = Some(5.0);
        let policy = RoutingPolicy::new(config);

        let signals = ComplexitySignals {
            token_count: 5000, // Would normally be slow
            keywords: vec![("architecture".to_string(), 15.0)],
            current_cost: 6.0, // Over hard limit
            ..Default::default()
        };

        let result = policy.select_model(&signals);
        assert_eq!(result.role, "smol");
        assert!(matches!(result.reason, SelectionReason::BudgetThreshold { .. }));
    }

    #[test]
    fn test_soft_budget_biases_cheaper() {
        let mut config = RoutingPolicyConfig::default();
        config.budget_soft_limit = Some(1.0);
        let policy = RoutingPolicy::new(config);

        // Without soft budget pressure, this would select "slow"
        let signals_no_budget = ComplexitySignals {
            token_count: 2000,
            keywords: vec![("architecture".to_string(), 15.0), ("refactor".to_string(), 10.0)],
            current_cost: 0.0,
            prompt_text: None,
            ..Default::default()
        };
        let result_no_budget = policy.select_model(&signals_no_budget);
        assert_eq!(result_no_budget.role, "slow");

        // With soft budget pressure, score is halved → should downgrade
        let signals_over_soft = ComplexitySignals {
            current_cost: 2.0, // Over soft limit
            ..signals_no_budget
        };
        let result_over_soft = policy.select_model(&signals_over_soft);
        // Score halved: should be default or smol, not slow
        assert_ne!(result_over_soft.role, "slow");
    }

    #[test]
    fn test_hard_budget_overrides_user_hint() {
        let mut config = RoutingPolicyConfig::default();
        config.budget_hard_limit = Some(5.0);
        let policy = RoutingPolicy::new(config);

        let signals = ComplexitySignals {
            user_hint: Some(ModelRoleHint::Thorough), // User wants slow
            current_cost: 6.0,                        // But over hard limit
            ..Default::default()
        };

        let result = policy.select_model(&signals);
        assert_eq!(result.role, "smol"); // Hard budget wins
    }

    #[test]
    fn test_complexity_scoring() {
        let policy = RoutingPolicy::default_policy();

        // Test token contribution
        let signals = ComplexitySignals {
            token_count: 500,
            recent_tools: vec![],
            keywords: vec![],
            user_hint: None,
            current_cost: 0.0,
            prompt_text: None,
        };
        let score = policy.compute_complexity_score(&signals);
        assert!(score > 0.0);

        // Test keyword contribution
        let signals = ComplexitySignals {
            token_count: 100,
            recent_tools: vec![],
            keywords: vec![("refactor".to_string(), 10.0)],
            user_hint: None,
            current_cost: 0.0,
            prompt_text: None,
        };
        let score_with_keyword = policy.compute_complexity_score(&signals);
        let score_without_keyword = policy.compute_complexity_score(&ComplexitySignals {
            keywords: vec![],
            ..signals.clone()
        });
        assert!(score_with_keyword > score_without_keyword);
    }
}
