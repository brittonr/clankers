//! Complexity signals for model routing decisions

use serde::Deserialize;
use serde::Serialize;

/// All signals used to determine task complexity and route to appropriate model
#[derive(Debug, Clone, Default)]
pub struct ComplexitySignals {
    /// Estimated token count in the user prompt
    pub token_count: usize,
    /// Recent tool calls from conversation history
    pub recent_tools: Vec<ToolCallSummary>,
    /// Keywords extracted from prompt with their complexity weights
    pub keywords: Vec<(String, f32)>,
    /// User hint extracted from prompt (e.g. "quick answer", "think deeply")
    pub user_hint: Option<ModelRoleHint>,
    /// Cumulative cost so far (for budget awareness in later phases)
    pub current_cost: f64,
}

/// Summary of a tool call for complexity assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallSummary {
    pub tool_name: String,
    pub complexity: ToolComplexity,
}

/// Tool complexity classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolComplexity {
    /// Simple read-only operations: read, ls, grep, find
    Simple,
    /// Medium write/execute operations: bash, edit, write
    Medium,
    /// Complex orchestration: subagent, delegate, agent
    Complex,
}

impl ToolComplexity {
    /// Numeric weight for scoring
    pub fn weight(self) -> f32 {
        match self {
            Self::Simple => 1.0,
            Self::Medium => 3.0,
            Self::Complex => 10.0,
        }
    }
}

/// User hint parsed from prompt text
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelRoleHint {
    /// Explicit role name (e.g. "use opus" → "slow")
    Explicit(String),
    /// Fast/quick preference
    Fast,
    /// Thorough/deep thinking preference
    Thorough,
}
