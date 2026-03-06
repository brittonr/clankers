//! Multi-model orchestration patterns
//!
//! Runs a single user turn in sequential phases, each using a different
//! model optimized for that phase's goal. Disabled by default.

use serde::Deserialize;
use serde::Serialize;

// ── Patterns ────────────────────────────────────────────────────────────────

/// Which collaboration pattern to use
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrchestrationPattern {
    /// Fast model drafts, slow model refines (haiku → opus)
    ProposeValidate,
    /// Slow model architects, medium model implements (opus → sonnet)
    PlanExecute,
    /// Fast model writes boilerplate, medium model polishes (haiku → sonnet)
    DraftReview,
}

impl std::fmt::Display for OrchestrationPattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ProposeValidate => write!(f, "ProposeValidate"),
            Self::PlanExecute => write!(f, "PlanExecute"),
            Self::DraftReview => write!(f, "DraftReview"),
        }
    }
}

// ── Plan ────────────────────────────────────────────────────────────────────

/// A concrete plan: pattern + ordered phases
#[derive(Debug, Clone)]
pub struct OrchestrationPlan {
    pub pattern: OrchestrationPattern,
    pub phases: Vec<OrchestrationPhase>,
}

/// One phase of an orchestrated turn
#[derive(Debug, Clone)]
pub struct OrchestrationPhase {
    /// Model role for this phase (resolved to a model ID at execution time)
    pub role: String,
    /// Label shown in logs / status bar
    pub label: String,
    /// Text appended to the system prompt for this phase
    pub system_suffix: String,
}

// ── Plan builders ───────────────────────────────────────────────────────────

impl OrchestrationPlan {
    /// Haiku drafts → Opus refines
    pub fn propose_validate() -> Self {
        Self {
            pattern: OrchestrationPattern::ProposeValidate,
            phases: vec![
                OrchestrationPhase {
                    role: "smol".to_string(),
                    label: "proposing".to_string(),
                    system_suffix: PROPOSE_SUFFIX.to_string(),
                },
                OrchestrationPhase {
                    role: "slow".to_string(),
                    label: "validating".to_string(),
                    system_suffix: VALIDATE_SUFFIX.to_string(),
                },
            ],
        }
    }

    /// Opus plans → Sonnet executes
    pub fn plan_execute() -> Self {
        Self {
            pattern: OrchestrationPattern::PlanExecute,
            phases: vec![
                OrchestrationPhase {
                    role: "slow".to_string(),
                    label: "planning".to_string(),
                    system_suffix: PLAN_SUFFIX.to_string(),
                },
                OrchestrationPhase {
                    role: "default".to_string(),
                    label: "executing".to_string(),
                    system_suffix: EXECUTE_SUFFIX.to_string(),
                },
            ],
        }
    }

    /// Haiku drafts → Sonnet polishes
    pub fn draft_review() -> Self {
        Self {
            pattern: OrchestrationPattern::DraftReview,
            phases: vec![
                OrchestrationPhase {
                    role: "smol".to_string(),
                    label: "drafting".to_string(),
                    system_suffix: DRAFT_SUFFIX.to_string(),
                },
                OrchestrationPhase {
                    role: "default".to_string(),
                    label: "reviewing".to_string(),
                    system_suffix: REVIEW_SUFFIX.to_string(),
                },
            ],
        }
    }
}

// ── Detection ───────────────────────────────────────────────────────────────

/// Decide whether to orchestrate and which pattern to use.
///
/// Returns `None` if orchestration is not warranted.
pub fn detect_pattern(prompt: &str, complexity_score: f32) -> Option<OrchestrationPlan> {
    if complexity_score < 40.0 {
        return None;
    }

    let lower = prompt.to_lowercase();

    // Explicit orchestration hints win outright
    if lower.contains("propose and validate") || lower.contains("draft and refine") {
        return Some(OrchestrationPlan::propose_validate());
    }
    if lower.contains("plan and implement") || lower.contains("plan and execute") {
        return Some(OrchestrationPlan::plan_execute());
    }
    if lower.contains("draft and review") || lower.contains("write and review") {
        return Some(OrchestrationPlan::draft_review());
    }

    // Heuristic detection at high complexity
    if complexity_score < 60.0 {
        return None;
    }

    let is_generation = lower.contains("write")
        || lower.contains("implement")
        || lower.contains("create")
        || lower.contains("build");
    let is_architecture = lower.contains("refactor")
        || lower.contains("architecture")
        || lower.contains("design")
        || lower.contains("complex");

    if is_generation && is_architecture {
        // Default to ProposeValidate for complex code generation
        return Some(OrchestrationPlan::propose_validate());
    }

    None
}

// ── System prompt suffixes ──────────────────────────────────────────────────

const PROPOSE_SUFFIX: &str = "\n\n\
## Phase: Propose\n\n\
Generate a working draft solution quickly. Prioritize:\n\
- Covering the main logic flow\n\
- Basic error handling\n\
- Correctness of the core approach\n\n\
A second model will review and refine your work, so don't over-engineer.";

const VALIDATE_SUFFIX: &str = "\n\n\
## Phase: Validate\n\n\
A draft solution was generated in the previous assistant message. Your goal:\n\
- Review for correctness, safety, and edge cases\n\
- Add comprehensive error handling\n\
- Fix bugs or unsafe patterns\n\
- Improve clarity and performance\n\n\
Preserve what works. Output the final refined version.";

const PLAN_SUFFIX: &str = "\n\n\
## Phase: Plan\n\n\
Create a detailed implementation plan. Include:\n\
- Architecture and key design decisions\n\
- Step-by-step implementation order (dependencies first)\n\
- Potential risks and mitigations\n\
- Key tests to write\n\n\
Output a structured plan, not code. The next phase will execute it.";

const EXECUTE_SUFFIX: &str = "\n\n\
## Phase: Execute\n\n\
A detailed plan was created in the previous assistant message. Your goal:\n\
- Implement the plan step-by-step\n\
- Follow the recommended order and approach\n\
- Run tests as you go\n\
- Stay aligned with the plan's architecture\n\n\
Don't deviate from the plan unless you find a critical flaw (explain if you do).";

const DRAFT_SUFFIX: &str = "\n\n\
## Phase: Draft\n\n\
Generate the bulk content quickly. Cover all items comprehensively.\n\
Focus on completeness over polish — a second model will refine your output.";

const REVIEW_SUFFIX: &str = "\n\n\
## Phase: Review\n\n\
A draft was generated in the previous assistant message. Your goal:\n\
- Improve clarity, accuracy, and consistency\n\
- Fix technical errors\n\
- Add missing edge cases or examples\n\
- Polish language and formatting\n\n\
Output the final refined version.";

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_explicit_propose_validate() {
        let plan = detect_pattern("propose and validate a new parser design", 50.0);
        assert!(plan.is_some());
        assert_eq!(plan.unwrap().pattern, OrchestrationPattern::ProposeValidate);
    }

    #[test]
    fn test_detect_explicit_plan_execute() {
        let plan = detect_pattern("plan and implement a new auth system", 50.0);
        assert!(plan.is_some());
        assert_eq!(plan.unwrap().pattern, OrchestrationPattern::PlanExecute);
    }

    #[test]
    fn test_detect_explicit_draft_review() {
        let plan = detect_pattern("draft and review the API documentation", 50.0);
        assert!(plan.is_some());
        assert_eq!(plan.unwrap().pattern, OrchestrationPattern::DraftReview);
    }

    #[test]
    fn test_detect_heuristic_high_complexity() {
        let plan = detect_pattern("write a complex parser with error recovery", 70.0);
        assert!(plan.is_some());
        assert_eq!(plan.unwrap().pattern, OrchestrationPattern::ProposeValidate);
    }

    #[test]
    fn test_no_detect_low_complexity() {
        let plan = detect_pattern("write a complex parser", 30.0);
        assert!(plan.is_none());
    }

    #[test]
    fn test_no_detect_medium_without_hints() {
        let plan = detect_pattern("list all files in the directory", 55.0);
        assert!(plan.is_none());
    }

    #[test]
    fn test_propose_validate_has_two_phases() {
        let plan = OrchestrationPlan::propose_validate();
        assert_eq!(plan.phases.len(), 2);
        assert_eq!(plan.phases[0].role, "smol");
        assert_eq!(plan.phases[1].role, "slow");
    }

    #[test]
    fn test_plan_execute_has_two_phases() {
        let plan = OrchestrationPlan::plan_execute();
        assert_eq!(plan.phases.len(), 2);
        assert_eq!(plan.phases[0].role, "slow");
        assert_eq!(plan.phases[1].role, "default");
    }

    #[test]
    fn test_draft_review_has_two_phases() {
        let plan = OrchestrationPlan::draft_review();
        assert_eq!(plan.phases.len(), 2);
        assert_eq!(plan.phases[0].role, "smol");
        assert_eq!(plan.phases[1].role, "default");
    }

    #[test]
    fn test_pattern_display() {
        assert_eq!(format!("{}", OrchestrationPattern::ProposeValidate), "ProposeValidate");
        assert_eq!(format!("{}", OrchestrationPattern::PlanExecute), "PlanExecute");
    }
}
