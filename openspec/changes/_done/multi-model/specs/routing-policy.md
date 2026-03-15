# RoutingPolicy — Automatic Model Selection

## Overview

The `RoutingPolicy` evaluates complexity signals at the start of each turn
and selects the appropriate `ModelRole`. It considers token count, tool
usage patterns, keyword hints, cost budgets, and explicit user/agent requests.

## Data Structures

### RoutingPolicy

```rust
pub struct RoutingPolicy {
    config: RoutingPolicyConfig,
    cost_tracker: Arc<CostTracker>,
    history: ConversationHistory,
}

pub struct RoutingPolicyConfig {
    /// Enable automatic routing (default: true)
    /// If false, always use ModelRole::Default unless user explicitly switches
    enabled: bool,

    /// Complexity score thresholds
    /// score < low_threshold → Smol (haiku)
    /// low_threshold ≤ score < high_threshold → Default (sonnet)
    /// score ≥ high_threshold → Slow (opus)
    low_threshold: f32,   // default: 20
    high_threshold: f32,  // default: 50

    /// Weight for token count in complexity score
    /// score += (token_count / 100) * token_weight
    token_weight: f32,    // default: 1.0

    /// Weight for tool complexity
    /// Simple tools (read, ls, grep): +0
    /// Medium tools (bash, edit, write): +5
    /// Complex tools (subagent, delegate): +10
    tool_weight: f32,     // default: 1.0

    /// Keyword → complexity delta mapping
    /// "refactor" → +10, "list" → -5, etc.
    keyword_hints: HashMap<String, f32>,

    /// Budget-aware downgrade
    /// If cost > soft_limit, prefer Smol over Default/Slow
    /// If cost > hard_limit, force Smol for all turns
    budget_soft_limit: Option<f32>,  // USD, e.g., 5.0
    budget_hard_limit: Option<f32>,  // USD, e.g., 10.0

    /// Allow agent-initiated model switching
    allow_agent_switch: bool,  // default: true

    /// Orchestration mode (propose/validate pattern)
    /// If enabled and complexity is high, use multi-model workflow
    enable_orchestration: bool,  // default: false (experimental)
}
```

### ComplexitySignals

```rust
struct ComplexitySignals {
    /// Number of tokens in the user's prompt
    token_count: usize,

    /// Tool calls from the last N turns
    recent_tools: Vec<ToolCallSummary>,

    /// Detected keywords with complexity hints
    keywords: Vec<(String, f32)>,  // ("refactor", +10.0)

    /// Explicit user hint ("use opus", "fast response")
    user_hint: Option<ModelRoleHint>,

    /// Current cost (USD)
    current_cost: f32,

    /// Agent's explicit request (if any)
    agent_request: Option<ModelRole>,
}

struct ToolCallSummary {
    tool_name: String,
    complexity: ToolComplexity,
}

enum ToolComplexity {
    Simple,   // read, ls, grep, find
    Medium,   // bash, edit, write
    Complex,  // subagent, delegate, labgrid
}

enum ModelRoleHint {
    Explicit(ModelRole),  // "use opus"
    Fast,                 // "quick answer", "fast please"
    Thorough,             // "think deeply", "be careful"
}
```

### ModelSelectionResult

```rust
struct ModelSelectionResult {
    /// Selected role
    role: ModelRole,

    /// Complexity score that led to this decision
    score: f32,

    /// Reason for selection (for logging/debugging)
    reason: SelectionReason,

    /// Whether to use orchestration (multi-model)
    orchestration: Option<OrchestrationPlan>,
}

enum SelectionReason {
    /// Score-based selection
    ComplexityScore(f32),

    /// User explicitly requested via /role or prompt hint
    UserRequested,

    /// Agent called switch_model tool
    AgentRequested { old_role: ModelRole },

    /// Budget threshold triggered downgrade
    BudgetThreshold { limit: f32, current: f32 },

    /// Orchestration pattern selected
    Orchestration { pattern: OrchestrationPattern },
}

struct OrchestrationPlan {
    pattern: OrchestrationPattern,
    phases: Vec<OrchestrationPhase>,
}

enum OrchestrationPattern {
    ProposeValidate,   // Fast model drafts, slow model refines
    PlanExecute,       // Slow model plans, fast model implements
    DraftReview,       // Fast model generates, slow model critiques
}

struct OrchestrationPhase {
    role: ModelRole,
    prompt_suffix: String,  // Appended to prompt for this phase
}
```

## Behavior

### Complexity Score Computation

```rust
impl RoutingPolicy {
    fn compute_complexity(&self, signals: &ComplexitySignals) -> f32 {
        let mut score = 0.0;

        // Token count contribution
        score += (signals.token_count as f32 / 100.0) * self.config.token_weight;

        // Tool complexity contribution
        for tool in &signals.recent_tools {
            let delta = match tool.complexity {
                ToolComplexity::Simple => 0.0,
                ToolComplexity::Medium => 5.0,
                ToolComplexity::Complex => 10.0,
            };
            score += delta * self.config.tool_weight;
        }

        // Keyword hints
        for (keyword, delta) in &signals.keywords {
            score += delta;
        }

        // User hints override score entirely
        if let Some(hint) = &signals.user_hint {
            return match hint {
                ModelRoleHint::Explicit(_) => f32::INFINITY,  // Always honor
                ModelRoleHint::Fast => -100.0,  // Force smol
                ModelRoleHint::Thorough => 100.0,  // Force slow
            };
        }

        score
    }

    pub fn select_model(&self, signals: ComplexitySignals) -> ModelSelectionResult {
        // 1. Check agent request first (high priority)
        if let Some(role) = signals.agent_request {
            return ModelSelectionResult {
                role,
                score: 0.0,
                reason: SelectionReason::AgentRequested { old_role: self.current_role() },
                orchestration: None,
            };
        }

        // 2. Check explicit user hint
        if let Some(ModelRoleHint::Explicit(role)) = signals.user_hint {
            return ModelSelectionResult {
                role,
                score: 0.0,
                reason: SelectionReason::UserRequested,
                orchestration: None,
            };
        }

        // 3. Check budget constraints
        if let Some(hard_limit) = self.config.budget_hard_limit {
            if signals.current_cost >= hard_limit {
                return ModelSelectionResult {
                    role: ModelRole::Smol,
                    score: 0.0,
                    reason: SelectionReason::BudgetThreshold {
                        limit: hard_limit,
                        current: signals.current_cost,
                    },
                    orchestration: None,
                };
            }
        }

        // 4. Compute complexity score
        let score = self.compute_complexity(&signals);

        // 5. Apply soft budget pressure (prefer cheaper models)
        let adjusted_score = if let Some(soft_limit) = self.config.budget_soft_limit {
            if signals.current_cost >= soft_limit {
                score * 0.7  // Reduce score to bias toward cheaper models
            } else {
                score
            }
        } else {
            score
        };

        // 6. Map score to role
        let role = if adjusted_score < self.config.low_threshold {
            ModelRole::Smol
        } else if adjusted_score < self.config.high_threshold {
            ModelRole::Default
        } else {
            ModelRole::Slow
        };

        // 7. Check orchestration eligibility
        let orchestration = if self.config.enable_orchestration && adjusted_score > 60.0 {
            Some(self.plan_orchestration(&signals, adjusted_score))
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
}
```

### Keyword Extraction

```rust
impl RoutingPolicy {
    fn extract_keywords(&self, prompt: &str) -> Vec<(String, f32)> {
        let lower = prompt.to_lowercase();
        let mut matches = Vec::new();

        for (keyword, delta) in &self.config.keyword_hints {
            if lower.contains(keyword) {
                matches.push((keyword.clone(), *delta));
            }
        }

        matches
    }
}
```

### Default Keyword Hints

The default config includes:

```rust
fn default_keyword_hints() -> HashMap<String, f32> {
    [
        // Complexity increasers
        ("refactor", 10.0),
        ("architecture", 15.0),
        ("design", 10.0),
        ("complex", 10.0),
        ("optimize", 8.0),
        ("analyze", 8.0),
        ("debug", 8.0),
        ("security", 12.0),
        ("performance", 8.0),

        // Complexity reducers
        ("list", -5.0),
        ("show", -5.0),
        ("read", -5.0),
        ("grep", -8.0),
        ("find", -8.0),
        ("quick", -10.0),
        ("simple", -8.0),

        // User hints
        ("use opus", f32::INFINITY),
        ("use haiku", f32::NEG_INFINITY),
        ("fast please", -15.0),
        ("think deeply", 15.0),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), *v))
    .collect()
}
```

## Integration

### In Agent Turn Loop

```rust
// At the start of each turn
let signals = ComplexitySignals {
    token_count: user_message.len() / 4,  // rough token estimate
    recent_tools: self.get_recent_tool_calls(5),
    keywords: self.policy.extract_keywords(&user_message),
    user_hint: self.parse_user_hint(&user_message),
    current_cost: self.cost_tracker.total_cost(),
    agent_request: None,
};

let selection = self.policy.select_model(signals);

if selection.role != self.current_model_role() {
    self.switch_to_model(
        selection.role,
        selection.reason.clone(),
    )?;
}

// If orchestration is planned, execute multi-phase turn
if let Some(plan) = selection.orchestration {
    self.execute_orchestrated_turn(plan, user_message).await?;
} else {
    self.execute_turn(user_message).await?;
}
```

## File Location

`src/routing/policy.rs` — new module under `src/routing/`.
