//! Integration tests for the multi-model conversations feature
//!
//! These tests verify the full routing + cost + orchestration pipeline works
//! together without requiring actual LLM calls or a provider.

use std::sync::Arc;
use std::time::Instant;

use parking_lot::Mutex;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use std::collections::HashMap;

use super::config::RoutingPolicyConfig;
use super::cost_tracker::{BudgetStatus, CostTracker, CostTrackerConfig, ModelPricing};
use super::orchestration::OrchestrationPattern;
use super::policy::{RoutingPolicy, SelectionReason};
use super::signals::{ComplexitySignals, ModelRoleHint, ToolCallSummary, ToolComplexity};
use crate::config::model_roles::ModelRoles;
use crate::tools::cost::CostTool;
use crate::tools::switch_model::{model_switch_slot, SwitchModelTool};
use crate::tools::{Tool, ToolContext};

// ── Test helpers ────────────────────────────────────────────────────────────

/// Pricing table used across integration tests.
fn test_pricing() -> HashMap<String, ModelPricing> {
    [
        ("claude-opus-4", 15.0, 75.0, "Claude Opus 4"),
        ("claude-sonnet-4-5", 3.0, 15.0, "Claude Sonnet 4.5"),
        ("claude-sonnet-4", 3.0, 15.0, "Claude Sonnet 4"),
        ("claude-haiku-4", 1.0, 5.0, "Claude Haiku 4"),
    ]
    .into_iter()
    .map(|(id, input, output, name)| {
        (
            id.to_string(),
            ModelPricing {
                input_per_mtok: input,
                output_per_mtok: output,
                display_name: name.to_string(),
            },
        )
    })
    .collect()
}

fn make_tool_ctx() -> ToolContext {
    ToolContext::new("test-call".to_string(), CancellationToken::new(), None)
}

fn setup_model_roles() -> ModelRoles {
    let mut roles = ModelRoles::with_defaults();
    roles.set_model("smol", "claude-haiku-4".to_string());
    roles.set_model("default", "claude-sonnet-4-5".to_string());
    roles.set_model("slow", "claude-opus-4".to_string());
    roles
}

fn result_text(result: &crate::tools::ToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|c| match c {
            crate::tools::ToolResultContent::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

// ── Test 1: Full routing pipeline ───────────────────────────────────────────

#[test]
fn test_full_routing_pipeline_simple() {
    let policy = RoutingPolicy::default_policy();

    // Simple prompt: low tokens, no tools, simple keywords
    let signals = ComplexitySignals {
        token_count: 50,
        recent_tools: vec![ToolCallSummary {
            tool_name: "read".to_string(),
            complexity: ToolComplexity::Simple,
        }],
        keywords: vec![("simple".to_string(), -8.0)],
        user_hint: None,
        current_cost: 0.0,
        prompt_text: None,
    };

    let result = policy.select_model(&signals);
    assert_eq!(result.role, "smol");
    assert!(matches!(result.reason, SelectionReason::ComplexityScore(_)));
}

#[test]
fn test_full_routing_pipeline_complex() {
    let policy = RoutingPolicy::default_policy();

    // Complex prompt: many tokens, complex tools, architecture keywords
    let signals = ComplexitySignals {
        token_count: 2000,
        recent_tools: vec![
            ToolCallSummary {
                tool_name: "subagent".to_string(),
                complexity: ToolComplexity::Complex,
            },
            ToolCallSummary {
                tool_name: "delegate".to_string(),
                complexity: ToolComplexity::Complex,
            },
        ],
        keywords: vec![
            ("architecture".to_string(), 15.0),
            ("refactor".to_string(), 10.0),
        ],
        user_hint: None,
        current_cost: 0.0,
        prompt_text: None,
    };

    let result = policy.select_model(&signals);
    assert_eq!(result.role, "slow");
    assert!(result.score > 50.0);
}

#[test]
fn test_full_routing_pipeline_medium() {
    let policy = RoutingPolicy::default_policy();

    // Medium prompt: moderate tokens, medium tools, some keywords
    let signals = ComplexitySignals {
        token_count: 1500,
        recent_tools: vec![ToolCallSummary {
            tool_name: "bash".to_string(),
            complexity: ToolComplexity::Medium,
        }],
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
fn test_full_routing_pipeline_user_hint_opus() {
    let policy = RoutingPolicy::default_policy();

    // User hint "use opus" overrides everything
    let signals = ComplexitySignals {
        token_count: 50,
        recent_tools: vec![],
        keywords: vec![],
        user_hint: Some(ModelRoleHint::Explicit("slow".to_string())),
        current_cost: 0.0,
        prompt_text: None,
    };

    let result = policy.select_model(&signals);
    assert_eq!(result.role, "slow");
    assert!(matches!(result.reason, SelectionReason::UserRequested));
}

#[test]
fn test_full_routing_pipeline_user_hint_quick() {
    let policy = RoutingPolicy::default_policy();

    // User hint "quick answer" forces smol
    let signals = ComplexitySignals {
        token_count: 2000,
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

// ── Test 2: Budget threshold triggers downgrade ─────────────────────────────

#[test]
fn test_budget_soft_limit_biases_cheaper() {
    let mut config = RoutingPolicyConfig::default();
    config.budget_soft_limit = Some(5.0);
    config.budget_hard_limit = Some(10.0);
    let policy = RoutingPolicy::new(config);

    // Without budget pressure, this would select "slow"
    let signals_no_budget = ComplexitySignals {
        token_count: 2000,
        keywords: vec![
            ("architecture".to_string(), 15.0),
            ("refactor".to_string(), 10.0),
        ],
        current_cost: 0.0,
        ..Default::default()
    };
    let result_no_budget = policy.select_model(&signals_no_budget);
    assert_eq!(result_no_budget.role, "slow");

    // With cost at $6 (over soft, under hard), score should be halved
    let signals_over_soft = ComplexitySignals {
        current_cost: 6.0,
        ..signals_no_budget
    };
    let result_over_soft = policy.select_model(&signals_over_soft);
    // Score halved should bias toward cheaper model
    assert_ne!(result_over_soft.role, "slow");
    assert!(result_over_soft.score < result_no_budget.score);
}

#[test]
fn test_budget_hard_limit_forces_smol() {
    let mut config = RoutingPolicyConfig::default();
    config.budget_soft_limit = Some(5.0);
    config.budget_hard_limit = Some(10.0);
    let policy = RoutingPolicy::new(config);

    // Push cost to $11 (over hard limit)
    let signals = ComplexitySignals {
        token_count: 5000,
        keywords: vec![("architecture".to_string(), 15.0)],
        current_cost: 11.0,
        ..Default::default()
    };

    let result = policy.select_model(&signals);
    assert_eq!(result.role, "smol");
    assert!(matches!(
        result.reason,
        SelectionReason::BudgetThreshold { .. }
    ));
}

// ── Test 3: Cost tracker accumulation ───────────────────────────────────────

#[test]
fn test_cost_tracker_accumulation() {
    let tracker = CostTracker::new(test_pricing(), CostTrackerConfig::default());

    // Record usage across multiple models
    tracker.record_usage("claude-sonnet-4-5", 100_000, 50_000);
    tracker.record_usage("claude-opus-4", 10_000, 5_000);
    tracker.record_usage("claude-haiku-4", 200_000, 100_000);

    let summary = tracker.summary();

    // Should have 3 models tracked
    assert_eq!(summary.by_model.len(), 3);

    // Total cost should be sum of individual costs
    let sum: f64 = summary.by_model.iter().map(|m| m.cost_usd).sum();
    assert!((summary.total_cost - sum).abs() < 0.001);

    // Should have a most expensive model
    assert!(summary.most_expensive.is_some());
}

#[test]
fn test_cost_tracker_budget_status_transitions() {
    let tracker = CostTracker::new(
        test_pricing(),
        CostTrackerConfig {
            soft_limit: Some(1.0),
            hard_limit: Some(5.0),
            warning_interval: None,
        },
    );

    // Initially under soft limit
    tracker.record_usage("claude-haiku-4", 10_000, 5_000); // ~$0.035
    assert!(matches!(tracker.budget_status(), BudgetStatus::Ok { .. }));

    // Over soft, under hard
    tracker.record_usage("claude-sonnet-4-5", 1_000_000, 0); // +$3.00
    assert!(matches!(
        tracker.budget_status(),
        BudgetStatus::Warning { .. }
    ));

    // Over hard
    tracker.record_usage("claude-opus-4", 1_000_000, 0); // +$15.00
    assert!(matches!(
        tracker.budget_status(),
        BudgetStatus::Exceeded { .. }
    ));
}

#[test]
fn test_cost_tracker_total_cost_matches_summary() {
    let tracker = CostTracker::new(test_pricing(), CostTrackerConfig::default());
    tracker.record_usage("claude-sonnet-4-5", 100_000, 50_000);
    tracker.record_usage("claude-haiku-4", 200_000, 100_000);

    let total = tracker.total_cost();
    let summary = tracker.summary();

    assert!((total - summary.total_cost).abs() < 0.001);
}

// ── Test 4: Model switch validation ─────────────────────────────────────────

#[tokio::test]
async fn test_switch_model_to_smol_succeeds() {
    let slot = model_switch_slot();
    let roles = setup_model_roles();
    let current = Arc::new(Mutex::new("claude-sonnet-4-5".to_string()));
    let tool = SwitchModelTool::new(slot.clone(), roles, current);

    let result = tool
        .execute(
            &make_tool_ctx(),
            json!({"role": "smol", "reason": "task is simple"}),
        )
        .await;

    assert!(!result.is_error);
    assert_eq!(*slot.lock(), Some("claude-haiku-4".to_string()));
}

#[tokio::test]
async fn test_switch_model_over_budget_rejected() {
    let slot = model_switch_slot();
    let roles = setup_model_roles();
    let current = Arc::new(Mutex::new("claude-haiku-4".to_string()));

    let tracker = Arc::new(CostTracker::new(test_pricing(), CostTrackerConfig::default()));
    // Record expensive usage pushing over budget
    tracker.record_usage("claude-haiku-4", 10_000_000, 5_000_000);

    let tool = SwitchModelTool::new(slot.clone(), roles, current)
        .with_cost_tracker(tracker)
        .with_budget_hard_limit(1.0);

    // Try to upgrade to slow (opus) when over budget
    let result = tool
        .execute(
            &make_tool_ctx(),
            json!({"role": "slow", "reason": "want opus"}),
        )
        .await;

    assert!(result.is_error);
    assert!(slot.lock().is_none()); // No switch happened
}

#[tokio::test]
async fn test_switch_model_downgrade_allowed_over_budget() {
    let slot = model_switch_slot();
    let roles = setup_model_roles();
    let current = Arc::new(Mutex::new("claude-opus-4".to_string()));

    let tracker = Arc::new(CostTracker::new(test_pricing(), CostTrackerConfig::default()));
    tracker.record_usage("claude-opus-4", 10_000_000, 5_000_000);

    let tool = SwitchModelTool::new(slot.clone(), roles, current)
        .with_cost_tracker(tracker)
        .with_budget_hard_limit(1.0);

    // Downgrade to smol should be allowed even over budget
    let result = tool
        .execute(
            &make_tool_ctx(),
            json!({"role": "smol", "reason": "save money"}),
        )
        .await;

    assert!(!result.is_error);
    assert_eq!(*slot.lock(), Some("claude-haiku-4".to_string()));
}

#[tokio::test]
async fn test_switch_model_to_current_is_noop() {
    let slot = model_switch_slot();
    let roles = setup_model_roles();
    let current = Arc::new(Mutex::new("claude-sonnet-4-5".to_string()));
    let tool = SwitchModelTool::new(slot.clone(), roles, current);

    let result = tool
        .execute(
            &make_tool_ctx(),
            json!({"role": "default", "reason": "just checking"}),
        )
        .await;

    assert!(!result.is_error);
    assert!(slot.lock().is_none()); // No switch needed
    let text = result_text(&result);
    assert!(text.contains("Already using"));
}

// ── Test 5: Orchestration plan generation ───────────────────────────────────

#[test]
fn test_orchestration_plan_generation() {
    let mut config = RoutingPolicyConfig::default();
    config.enable_orchestration = true;
    let policy = RoutingPolicy::new(config);

    let signals = ComplexitySignals {
        token_count: 2000,
        keywords: vec![
            ("architecture".to_string(), 15.0),
            ("implement".to_string(), 10.0),
        ],
        prompt_text: Some("plan and implement a new auth system".to_string()),
        ..Default::default()
    };

    let result = policy.select_model(&signals);

    // Should generate an orchestration plan
    assert!(result.orchestration.is_some());
    let plan = result.orchestration.unwrap();
    assert_eq!(plan.pattern, OrchestrationPattern::PlanExecute);
    assert_eq!(plan.phases.len(), 2);
    assert_eq!(plan.phases[0].role, "slow");
    assert_eq!(plan.phases[1].role, "default");
}

#[test]
fn test_orchestration_high_complexity_generation() {
    let mut config = RoutingPolicyConfig::default();
    config.enable_orchestration = true;
    let policy = RoutingPolicy::new(config);

    let signals = ComplexitySignals {
        token_count: 3000,
        keywords: vec![
            ("refactor".to_string(), 10.0),
            ("architecture".to_string(), 15.0),
            ("complex".to_string(), 10.0),
        ],
        prompt_text: Some("write a complex distributed system with error recovery".to_string()),
        ..Default::default()
    };

    let result = policy.select_model(&signals);

    // High complexity code generation should trigger orchestration
    assert!(result.orchestration.is_some());
    let plan = result.orchestration.unwrap();
    assert_eq!(plan.pattern, OrchestrationPattern::ProposeValidate);
    assert_eq!(plan.phases.len(), 2);
}

// ── Test 6: Orchestration disabled by default ───────────────────────────────

#[test]
fn test_orchestration_disabled_by_default() {
    let policy = RoutingPolicy::default_policy(); // orchestration disabled

    let signals = ComplexitySignals {
        token_count: 2000,
        keywords: vec![
            ("architecture".to_string(), 15.0),
            ("implement".to_string(), 10.0),
        ],
        prompt_text: Some("plan and implement a new system".to_string()),
        ..Default::default()
    };

    let result = policy.select_model(&signals);

    // Should NOT generate orchestration plan
    assert!(result.orchestration.is_none());
}

// ── Test 7: Cost tool output ────────────────────────────────────────────────

#[tokio::test]
async fn test_cost_tool_summary_action() {
    let tracker = Arc::new(CostTracker::new(test_pricing(), CostTrackerConfig::default()));
    tracker.record_usage("claude-sonnet-4-5", 100_000, 50_000);
    tracker.record_usage("claude-haiku-4", 200_000, 100_000);

    let tool = CostTool::new(tracker);
    let result = tool
        .execute(&make_tool_ctx(), json!({"action": "summary"}))
        .await;

    assert!(!result.is_error);
    let text = result_text(&result);
    assert!(text.contains("$"));
    assert!(text.contains("2 model"));
}

#[tokio::test]
async fn test_cost_tool_breakdown_action() {
    let tracker = Arc::new(CostTracker::new(test_pricing(), CostTrackerConfig::default()));
    tracker.record_usage("claude-sonnet-4-5", 100_000, 50_000);
    tracker.record_usage("claude-haiku-4", 200_000, 100_000);

    let tool = CostTool::new(tracker);
    let result = tool
        .execute(&make_tool_ctx(), json!({"action": "breakdown"}))
        .await;

    assert!(!result.is_error);
    let text = result_text(&result);
    assert!(text.contains("Sonnet") || text.contains("sonnet"));
    assert!(text.contains("Haiku") || text.contains("haiku"));
    assert!(text.contains("%"));
}

#[tokio::test]
async fn test_cost_tool_budget_action() {
    let tracker = Arc::new(CostTracker::new(
        test_pricing(),
        CostTrackerConfig {
            soft_limit: Some(1.0),
            hard_limit: Some(5.0),
            warning_interval: None,
        },
    ));
    tracker.record_usage("claude-sonnet-4-5", 100_000, 50_000);

    let tool = CostTool::new(tracker);
    let result = tool
        .execute(&make_tool_ctx(), json!({"action": "budget"}))
        .await;

    assert!(!result.is_error);
    let text = result_text(&result);
    assert!(text.contains("$"));
}

// ── Test 8: Routing performance ─────────────────────────────────────────────

#[test]
fn test_routing_performance() {
    let policy = RoutingPolicy::default_policy();

    let signals = ComplexitySignals {
        token_count: 1500,
        recent_tools: vec![ToolCallSummary {
            tool_name: "bash".to_string(),
            complexity: ToolComplexity::Medium,
        }],
        keywords: vec![("optimize".to_string(), 8.0)],
        user_hint: None,
        current_cost: 0.0,
        prompt_text: None,
    };

    let start = Instant::now();
    for _ in 0..1000 {
        let _ = policy.select_model(&signals);
    }
    let elapsed = start.elapsed();

    // 1000 routing calls should take less than 1 second
    assert!(
        elapsed.as_millis() < 1000,
        "1000 routing calls took {:?}",
        elapsed
    );
}

// ── Test 9: CostTracker status_line format ──────────────────────────────────

#[test]
fn test_cost_tracker_status_line_format() {
    let tracker = CostTracker::new(
        test_pricing(),
        CostTrackerConfig {
            soft_limit: Some(5.0),
            hard_limit: Some(10.0),
            warning_interval: None,
        },
    );
    tracker.record_usage("claude-sonnet-4-5", 100_000, 50_000);

    let status = tracker.status_line("claude-sonnet-4-5");

    // Should contain model name, tokens, cost, and budget
    assert!(status.contains("Sonnet") || status.contains("sonnet"));
    assert!(status.contains("$"));
    assert!(status.contains("Budget"));
}

// ── Test 10: End-to-end routing + cost + switch interaction ────────────────

#[tokio::test]
async fn test_end_to_end_routing_cost_switch() {
    // Setup
    let mut config = RoutingPolicyConfig::default();
    config.budget_soft_limit = Some(2.0);
    let policy = RoutingPolicy::new(config);

    let tracker = Arc::new(CostTracker::new(
        test_pricing(),
        CostTrackerConfig {
            soft_limit: Some(2.0),
            hard_limit: Some(5.0),
            warning_interval: None,
        },
    ));

    let slot = model_switch_slot();
    let roles = setup_model_roles();
    let current = Arc::new(Mutex::new("claude-sonnet-4-5".to_string()));
    let switch_tool = SwitchModelTool::new(slot.clone(), roles, current.clone())
        .with_cost_tracker(tracker.clone())
        .with_budget_hard_limit(5.0);

    // Step 1: Run selection for complex task → get "slow"
    let signals1 = ComplexitySignals {
        token_count: 2000,
        keywords: vec![("architecture".to_string(), 15.0)],
        current_cost: 0.0,
        ..Default::default()
    };
    let result1 = policy.select_model(&signals1);
    assert_eq!(result1.role, "slow");

    // Step 2: Record expensive usage pushing over soft budget
    // opus: $15/MTok input + $75/MTok output
    // 100k input = $1.50, 50k output = $3.75 → ~$5.25
    tracker.record_usage("claude-opus-4", 100_000, 50_000);

    // Step 3: Run selection again → should prefer cheaper model
    let signals2 = ComplexitySignals {
        token_count: 2000,
        keywords: vec![("architecture".to_string(), 15.0)],
        current_cost: tracker.total_cost(),
        ..Default::default()
    };
    let result2 = policy.select_model(&signals2);
    // Score should be halved due to soft budget pressure
    assert!(result2.score < result1.score);

    // Step 4: Simulate agent switch to "smol"
    *current.lock() = "claude-opus-4".to_string();
    let switch_result = switch_tool
        .execute(
            &make_tool_ctx(),
            json!({"role": "smol", "reason": "save budget"}),
        )
        .await;
    assert!(!switch_result.is_error);
    assert_eq!(*slot.lock(), Some("claude-haiku-4".to_string()));

    // Step 5: Record cheap usage
    // haiku: $1/MTok input + $5/MTok output
    // 100k input = $0.10, 50k output = $0.25 → ~$0.35
    tracker.record_usage("claude-haiku-4", 100_000, 50_000);

    // Step 6: Verify total cost is sum of both (~$5.25 + ~$0.35 = ~$5.60)
    let total = tracker.total_cost();
    assert!(total > 5.0, "total cost {} should exceed $5", total);
    assert!(total < 7.0, "total cost {} should be under $7", total);
}

// ── Additional edge cases ───────────────────────────────────────────────────

#[test]
fn test_disabled_policy_always_returns_default() {
    let mut config = RoutingPolicyConfig::default();
    config.enabled = false;
    let policy = RoutingPolicy::new(config);

    let signals = ComplexitySignals {
        token_count: 5000,
        keywords: vec![("architecture".to_string(), 15.0)],
        user_hint: Some(ModelRoleHint::Thorough),
        ..Default::default()
    };

    let result = policy.select_model(&signals);
    assert_eq!(result.role, "default");
    assert!(matches!(result.reason, SelectionReason::Default));
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
    assert!(matches!(
        result.reason,
        SelectionReason::BudgetThreshold { .. }
    ));
}

#[test]
fn test_tool_complexity_weighting() {
    let policy = RoutingPolicy::default_policy();

    // Simple tools should contribute little to score
    let signals_simple = ComplexitySignals {
        token_count: 100,
        recent_tools: vec![
            ToolCallSummary {
                tool_name: "read".to_string(),
                complexity: ToolComplexity::Simple,
            },
            ToolCallSummary {
                tool_name: "ls".to_string(),
                complexity: ToolComplexity::Simple,
            },
        ],
        ..Default::default()
    };
    let score_simple = policy.compute_complexity_score(&signals_simple);

    // Complex tools should contribute significantly
    let signals_complex = ComplexitySignals {
        token_count: 100,
        recent_tools: vec![
            ToolCallSummary {
                tool_name: "subagent".to_string(),
                complexity: ToolComplexity::Complex,
            },
            ToolCallSummary {
                tool_name: "delegate".to_string(),
                complexity: ToolComplexity::Complex,
            },
        ],
        ..Default::default()
    };
    let score_complex = policy.compute_complexity_score(&signals_complex);

    assert!(score_complex > score_simple * 5.0); // Complex should be much higher
}

#[test]
fn test_orchestration_explicit_hints() {
    let mut config = RoutingPolicyConfig::default();
    config.enable_orchestration = true;
    let policy = RoutingPolicy::new(config);

    // Test "propose and validate" hint (needs high complexity to trigger)
    let signals1 = ComplexitySignals {
        token_count: 3000,
        keywords: vec![("architecture".to_string(), 15.0), ("design".to_string(), 10.0)],
        prompt_text: Some("propose and validate a new design".to_string()),
        ..Default::default()
    };
    let result1 = policy.select_model(&signals1);
    assert!(result1.orchestration.is_some());
    assert_eq!(
        result1.orchestration.unwrap().pattern,
        OrchestrationPattern::ProposeValidate
    );

    // Test "draft and review" hint
    let signals2 = ComplexitySignals {
        token_count: 3000,
        keywords: vec![("refactor".to_string(), 10.0), ("complex".to_string(), 10.0)],
        prompt_text: Some("draft and review the documentation".to_string()),
        ..Default::default()
    };
    let result2 = policy.select_model(&signals2);
    assert!(result2.orchestration.is_some());
    assert_eq!(
        result2.orchestration.unwrap().pattern,
        OrchestrationPattern::DraftReview
    );
}

// ── Test: switch_model tool integration with turn loop ──────────────────────

#[tokio::test]
async fn test_switch_model_tool_writes_slot_and_cost_validates() {
    // Simulates the full flow: agent calls switch_model, slot is set,
    // turn loop would read it and switch models on the next LLM call.
    let roles = setup_model_roles();
    let slot = model_switch_slot();
    let current = Arc::new(Mutex::new("claude-sonnet-4-5".to_string()));
    let tracker = Arc::new(CostTracker::new(test_pricing(), CostTrackerConfig::default()));

    let tool = SwitchModelTool::new(slot.clone(), roles.clone(), current.clone())
        .with_cost_tracker(tracker.clone())
        .with_budget_hard_limit(10.0);

    let ctx = make_tool_ctx();

    // Step 1: Agent switches to smol (downgrade)
    let result = tool
        .execute(&ctx, json!({"role": "smol", "reason": "simple grep task"}))
        .await;
    assert!(!result.is_error, "downgrade should succeed");
    assert_eq!(*slot.lock(), Some("claude-haiku-4".to_string()));

    // Simulate turn loop consuming the slot
    let switched_to = slot.lock().take().unwrap();
    *current.lock() = switched_to.clone();
    assert_eq!(switched_to, "claude-haiku-4");

    // Step 2: Record some usage on haiku
    tracker.record_usage("claude-haiku-4", 500_000, 200_000);
    let cost_after = tracker.total_cost();
    assert!(cost_after > 0.0);
    assert!(cost_after < 10.0, "should be well under budget");

    // Step 3: Agent switches to slow (upgrade) — should succeed under budget
    let result = tool
        .execute(&ctx, json!({"role": "slow", "reason": "complex refactor"}))
        .await;
    assert!(!result.is_error, "upgrade under budget should succeed");
    assert_eq!(*slot.lock(), Some("claude-opus-4".to_string()));

    // Consume the slot again
    let switched_to = slot.lock().take().unwrap();
    *current.lock() = switched_to;

    // Step 4: Record expensive opus usage to blow the budget
    tracker.record_usage("claude-opus-4", 2_000_000, 2_000_000);
    assert!(tracker.total_cost() > 10.0, "should exceed budget now");

    // Step 5: Attempt another upgrade — should be blocked
    let result = tool
        .execute(&ctx, json!({"role": "slow", "reason": "still want opus"}))
        .await;
    // Already on opus, so it's "already using" — no error, no switch
    assert!(!result.is_error);
    assert!(slot.lock().is_none());
    assert!(result_text(&result).contains("Already using"));

    // Step 6: Downgrade should always work even over budget
    let result = tool
        .execute(&ctx, json!({"role": "smol", "reason": "save money"}))
        .await;
    assert!(!result.is_error, "downgrade should work even over budget");
    assert_eq!(*slot.lock(), Some("claude-haiku-4".to_string()));
}

#[tokio::test]
async fn test_switch_model_with_routing_policy_interaction() {
    // Verify that routing policy and switch_model tool can coexist:
    // policy suggests a model, but a subsequent tool switch overrides it.
    let roles = setup_model_roles();
    let policy = RoutingPolicy::default_policy();

    // Policy would suggest haiku for simple tasks
    let signals = ComplexitySignals {
        token_count: 20,
        recent_tools: vec![],
        keywords: vec![],
        user_hint: None,
        current_cost: 0.0,
        prompt_text: None,
    };
    let selection = policy.select_model(&signals);
    assert_eq!(selection.role, "smol");

    // But agent decides it needs slow via the tool
    let slot = model_switch_slot();
    let current = Arc::new(Mutex::new("claude-haiku-4".to_string()));
    let tool = SwitchModelTool::new(slot.clone(), roles, current);
    let ctx = make_tool_ctx();

    let result = tool
        .execute(&ctx, json!({"role": "slow", "reason": "actually complex"}))
        .await;
    assert!(!result.is_error);
    // Tool switch takes priority — slot is set
    assert_eq!(*slot.lock(), Some("claude-opus-4".to_string()));
}

#[test]
fn test_cost_tracker_percentages_sum_to_100() {
    let tracker = CostTracker::new(test_pricing(), CostTrackerConfig::default());
    tracker.record_usage("claude-sonnet-4-5", 100_000, 50_000);
    tracker.record_usage("claude-haiku-4", 200_000, 100_000);
    tracker.record_usage("claude-opus-4", 10_000, 5_000);

    let summary = tracker.summary();
    let total_pct: f32 = summary.by_model.iter().map(|m| m.percentage).sum();

    assert!((total_pct - 100.0).abs() < 0.1);
}
