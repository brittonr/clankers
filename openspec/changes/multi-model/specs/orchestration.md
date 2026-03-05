# Orchestration — Multi-Model Collaboration Patterns

## Overview

Orchestration enables multiple models to collaborate within a single turn.
Instead of picking one model for the entire turn, orchestration runs the
turn in **phases**, each using a different model optimized for that phase's
goal. Common patterns: propose/validate, plan/execute, draft/review.

**Note:** Orchestration is experimental and disabled by default. Enable via
`RoutingPolicyConfig::enable_orchestration = true`.

## Patterns

### 1. Propose/Validate

**Use case:** Complex tasks where speed matters but correctness is critical.

**Flow:**
1. **Phase 1 — Propose (Haiku)**: Fast model generates a draft solution
2. **Phase 2 — Validate (Opus)**: Slow model reviews, finds errors, refines

**Example:**
```
User: "Write a safe Rust parser for JSON with full error recovery"

Phase 1 (haiku):
  - Generates basic parser structure
  - Handles happy path
  - Minimal error handling

Phase 2 (opus):
  - Reviews haiku's code
  - Adds comprehensive error recovery
  - Fixes unsafe assumptions
  - Optimizes performance
```

**Cost:** ~40% cheaper than pure opus, ~90% of opus quality

### 2. Plan/Execute

**Use case:** Large refactors or multi-step changes where strategy matters.

**Flow:**
1. **Phase 1 — Plan (Opus)**: Slow model creates architecture plan
2. **Phase 2 — Execute (Sonnet)**: Medium model implements the plan

**Example:**
```
User: "Refactor the session manager to use async/await throughout"

Phase 1 (opus):
  - Identifies all sync blocking points
  - Designs async architecture
  - Plans migration order (dependencies first)
  - Writes step-by-step implementation guide

Phase 2 (sonnet):
  - Follows the plan verbatim
  - Implements each step
  - Runs tests after each change
```

**Cost:** ~60% cheaper than pure opus, maintains strategic quality

### 3. Draft/Review

**Use case:** Documentation, commit messages, or boilerplate-heavy code.

**Flow:**
1. **Phase 1 — Draft (Haiku)**: Fast model generates bulk content
2. **Phase 2 — Review (Sonnet)**: Medium model edits for clarity/style

**Example:**
```
User: "Document the entire config module with examples"

Phase 1 (haiku):
  - Generates doc comments for all public items
  - Includes basic examples
  - Covers parameters and return values

Phase 2 (sonnet):
  - Improves clarity and flow
  - Adds edge-case examples
  - Fixes technical inaccuracies
  - Ensures consistent style
```

**Cost:** ~70% cheaper than pure sonnet, near-sonnet quality

## Data Structures

### OrchestrationPlan

```rust
pub struct OrchestrationPlan {
    /// Which pattern to use
    pattern: OrchestrationPattern,

    /// Phases to execute sequentially
    phases: Vec<OrchestrationPhase>,

    /// Original user prompt (passed to all phases)
    user_prompt: String,
}

pub enum OrchestrationPattern {
    ProposeValidate,
    PlanExecute,
    DraftReview,
}

pub struct OrchestrationPhase {
    /// Model role for this phase
    role: ModelRole,

    /// System prompt suffix (guides this phase's goal)
    system_suffix: String,

    /// Context from previous phase (if any)
    previous_output: Option<String>,

    /// Whether this phase sees tool calls from previous phases
    inherit_tool_context: bool,
}
```

## Behavior

### Pattern Selection

Orchestration mode is triggered when:
- `RoutingPolicyConfig::enable_orchestration` is true, AND
- Complexity score > 60 (highly complex task), AND
- One of:
  - User prompt contains orchestration hint ("write and review", "plan and implement")
  - Task is code generation + complexity keywords ("write", "refactor", "design")

```rust
impl RoutingPolicy {
    fn should_orchestrate(&self, signals: &ComplexitySignals) -> bool {
        if !self.config.enable_orchestration {
            return false;
        }

        if signals.score < 60.0 {
            return false;
        }

        // Check for orchestration hints
        let prompt = signals.user_prompt.to_lowercase();
        let hints = [
            "write and review",
            "plan and implement",
            "draft and refine",
            "propose and validate",
        ];

        if hints.iter().any(|h| prompt.contains(h)) {
            return true;
        }

        // Heuristic: code generation + complexity
        let is_code_gen = prompt.contains("write")
            || prompt.contains("implement")
            || prompt.contains("refactor");
        let is_complex = prompt.contains("complex")
            || prompt.contains("architecture")
            || prompt.contains("design");

        is_code_gen && is_complex
    }

    fn plan_orchestration(&self, signals: &ComplexitySignals) -> OrchestrationPlan {
        // Default: ProposeValidate (haiku → opus)
        OrchestrationPlan {
            pattern: OrchestrationPattern::ProposeValidate,
            phases: vec![
                OrchestrationPhase {
                    role: ModelRole::Smol,
                    system_suffix: PROPOSE_SUFFIX.into(),
                    previous_output: None,
                    inherit_tool_context: false,
                },
                OrchestrationPhase {
                    role: ModelRole::Slow,
                    system_suffix: VALIDATE_SUFFIX.into(),
                    previous_output: None,  // Filled during execution
                    inherit_tool_context: true,
                },
            ],
            user_prompt: signals.user_prompt.clone(),
        }
    }
}
```

### Phase Execution

```rust
impl Agent {
    async fn execute_orchestrated_turn(&mut self, plan: OrchestrationPlan) -> Result<String> {
        let mut final_output = String::new();

        for (i, mut phase) in plan.phases.into_iter().enumerate() {
            // Switch to phase's model
            self.switch_to_model(
                phase.role,
                SelectionReason::Orchestration {
                    pattern: plan.pattern.clone(),
                    phase: i,
                },
            )?;

            // Build phase-specific prompt
            let prompt = self.build_phase_prompt(&plan.user_prompt, &phase);

            // Execute turn with this model
            let output = self.execute_turn_with_prompt(prompt).await?;

            // Store output for next phase
            final_output = output.clone();
            if i + 1 < plan.phases.len() {
                plan.phases[i + 1].previous_output = Some(output);
            }

            // Log phase completion
            self.log_orchestration_phase(i, phase.role, &final_output);
        }

        Ok(final_output)
    }

    fn build_phase_prompt(&self, user_prompt: &str, phase: &OrchestrationPhase) -> String {
        let mut prompt = String::new();

        // Include previous phase output if present
        if let Some(prev) = &phase.previous_output {
            prompt.push_str("## Previous Phase Output\n\n");
            prompt.push_str(prev);
            prompt.push_str("\n\n");
        }

        // Add user's original request
        prompt.push_str("## Task\n\n");
        prompt.push_str(user_prompt);
        prompt.push_str("\n\n");

        // Add phase-specific instructions
        prompt.push_str(&phase.system_suffix);

        prompt
    }
}
```

### System Prompt Suffixes

```rust
const PROPOSE_SUFFIX: &str = r#"
## Phase: Propose

Your goal is to generate a working draft solution **quickly**. Prioritize:
- Speed over perfection
- Covering the main logic flow
- Basic error handling

The next phase will review and refine your work. Don't over-engineer.
"#;

const VALIDATE_SUFFIX: &str = r#"
## Phase: Validate

A draft solution was generated in the previous phase. Your goal is to:
- Review the code for correctness, safety, and edge cases
- Add comprehensive error handling
- Optimize performance and clarity
- Fix any bugs or unsafe patterns

Preserve what works, improve what doesn't. Output the final refined version.
"#;

const PLAN_SUFFIX: &str = r#"
## Phase: Plan

Your goal is to create a detailed implementation plan. Include:
- High-level architecture and design decisions
- Step-by-step implementation order (dependencies first)
- Potential risks and how to mitigate them
- Key tests to write

Output a structured plan, not code. The next phase will execute it.
"#;

const EXECUTE_SUFFIX: &str = r#"
## Phase: Execute

A detailed plan was created in the previous phase. Your goal is to:
- Implement the plan step-by-step
- Follow the recommended order and approach
- Run tests as you go
- Stay aligned with the plan's architecture decisions

Don't deviate from the plan unless you find a critical flaw (explain if you do).
"#;
```

## Session Logging

Each orchestration phase is logged as a separate turn segment in the session JSONL:

```json
{
  "entry_type": "orchestration_start",
  "pattern": "ProposeValidate",
  "phases": [
    { "role": "smol", "model": "claude-haiku-4" },
    { "role": "slow", "model": "claude-opus-4" }
  ]
}
{
  "entry_type": "orchestration_phase",
  "phase": 0,
  "role": "smol",
  "model": "claude-haiku-4",
  "output_preview": "Generated parser with basic error handling..."
}
{
  "entry_type": "orchestration_phase",
  "phase": 1,
  "role": "slow",
  "model": "claude-opus-4",
  "output_preview": "Refined parser with comprehensive error recovery..."
}
{
  "entry_type": "orchestration_complete",
  "total_cost": 0.42,
  "savings_vs_pure_opus": 0.58
}
```

## TUI Display

During orchestration, the status bar shows the current phase:

```
[Orchestrate: ProposeValidate] Phase 1/2: haiku drafting... | $0.03
[Orchestrate: ProposeValidate] Phase 2/2: opus refining... | $0.42
```

## Cost Comparison

Example task: "Write a complex Rust parser with error recovery" (5k token output)

| Approach | Models Used | Cost | Quality |
|----------|-------------|------|---------|
| Pure Opus | opus | $1.00 | 100% |
| Pure Sonnet | sonnet | $0.25 | 85% |
| Pure Haiku | haiku | $0.05 | 60% |
| **Propose/Validate** | haiku → opus | **$0.40** | **95%** |

Orchestration trades a small quality drop (5%) for 60% cost savings.

## Disabling Orchestration

Orchestration is experimental. To disable:

```json
// settings.json
{
  "routingPolicy": {
    "enableOrchestration": false
  }
}
```

Or via environment variable:

```bash
CLANKERS_ORCHESTRATION=false pi
```

## Future Patterns

### Parallel Validation (not in scope)

Run multiple validators (sonnet + opus) on haiku's draft, merge feedback.
Requires parallel execution support.

### Iterative Refinement (not in scope)

Haiku → Sonnet → Opus, each phase improves incrementally. Risk: diminishing
returns and high cost.

## File Location

`src/routing/orchestration.rs` — new module under `src/routing/`.
