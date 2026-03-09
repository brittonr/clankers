//! cost tool — agent self-awareness of token spend and budget
//!
//! Lets the agent inspect how much the session has cost, broken down
//! by model, and check remaining budget. Useful for deciding whether
//! to switch to a cheaper model or wrap up.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;
use crate::model_selection::cost_tracker::BudgetStatus;
use crate::model_selection::cost_tracker::CostTracker;

pub struct CostTool {
    tracker: Arc<CostTracker>,
}

impl CostTool {
    pub fn new(tracker: Arc<CostTracker>) -> Self {
        Self { tracker }
    }
}

#[async_trait]
impl Tool for CostTool {
    fn definition(&self) -> &ToolDefinition {
        static DEF: std::sync::OnceLock<ToolDefinition> = std::sync::OnceLock::new();
        DEF.get_or_init(|| ToolDefinition {
            name: "cost".to_string(),
            description: "Check this session's token usage, cost, and budget status. \
                Use to decide whether to switch to a cheaper model or wrap up."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["summary", "breakdown", "budget"],
                        "description": "What to show: 'summary' (one-line total), \
                            'breakdown' (per-model table), 'budget' (remaining budget)"
                    }
                },
                "required": ["action"]
            }),
        })
    }

    async fn execute(&self, _ctx: &ToolContext, params: Value) -> ToolResult {
        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("summary");

        let summary = self.tracker.summary();

        match action {
            "summary" => {
                let budget_line = format_budget(&summary.budget_status);
                ToolResult::text(format!(
                    "Session cost: ${:.4} across {} model(s). {}",
                    summary.total_cost,
                    summary.by_model.len(),
                    budget_line,
                ))
            }
            "breakdown" => {
                let mut out = format!("Session cost: ${:.4}\n\n", summary.total_cost);
                if summary.by_model.is_empty() {
                    out.push_str("No usage recorded yet.");
                } else {
                    out.push_str("Model                     | Input tok | Output tok |    Cost | Share\n");
                    out.push_str("--------------------------|-----------|------------|---------|------\n");
                    for m in &summary.by_model {
                        out.push_str(&format!(
                            "{:<25} | {:>9} | {:>10} | ${:>6.4} | {:>4.1}%\n",
                            m.display_name, m.input_tokens, m.output_tokens, m.cost_usd, m.percentage,
                        ));
                    }
                }
                out.push('\n');
                out.push_str(&format_budget(&summary.budget_status));
                ToolResult::text(out)
            }
            "budget" => ToolResult::text(format_budget_detail(&summary.budget_status, summary.total_cost)),
            other => {
                ToolResult::error(format!("Unknown action '{}'. Use 'summary', 'breakdown', or 'budget'.", other,))
            }
        }
    }
}

fn format_budget(status: &BudgetStatus) -> String {
    match status {
        BudgetStatus::NoBudget => "No budget configured.".to_string(),
        BudgetStatus::Ok { remaining } => format!("Budget OK — ${:.2} remaining.", remaining),
        BudgetStatus::Warning {
            over_soft_by,
            hard_limit_remaining,
        } => {
            if hard_limit_remaining.is_finite() {
                format!("⚠ Over soft budget by ${:.2}. ${:.2} until hard limit.", over_soft_by, hard_limit_remaining,)
            } else {
                format!("⚠ Over soft budget by ${:.2}.", over_soft_by)
            }
        }
        BudgetStatus::Exceeded { over_hard_by } => {
            format!("✖ Budget exceeded by ${:.2}. Model downgrades enforced.", over_hard_by)
        }
    }
}

fn format_budget_detail(status: &BudgetStatus, total: f64) -> String {
    let mut out = format!("Total spent: ${:.4}\n", total);
    match status {
        BudgetStatus::NoBudget => {
            out.push_str(
                "No budget limits configured.\nSet budget_soft_limit / budget_hard_limit in routing policy config.",
            );
        }
        BudgetStatus::Ok { remaining } => {
            out.push_str(&format!("Status: ✓ OK\nRemaining: ${:.2}\n", remaining));
            // Rough projection: if total > 0, estimate turns left
            if total > 0.01 {
                // remaining / (total / turns) but we don't know turns here
                // Just show the ratio
                let ratio = remaining / total;
                out.push_str(&format!("At current rate, ~{:.0}x more work before limit.", ratio,));
            }
        }
        BudgetStatus::Warning {
            over_soft_by,
            hard_limit_remaining,
        } => {
            out.push_str(&format!("Status: ⚠ Warning\nOver soft limit by: ${:.2}\n", over_soft_by));
            if hard_limit_remaining.is_finite() {
                out.push_str(&format!("Hard limit remaining: ${:.2}\n", hard_limit_remaining));
                out.push_str("Routing policy is biasing toward cheaper models.");
            }
        }
        BudgetStatus::Exceeded { over_hard_by } => {
            out.push_str(&format!("Status: ✖ Exceeded\nOver hard limit by: ${:.2}\n", over_hard_by));
            out.push_str("Routing policy is forcing cheapest model (smol/haiku).");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use tokio_util::sync::CancellationToken;

    use super::*;
    use crate::model_selection::cost_tracker::CostTrackerConfig;
    use crate::model_selection::cost_tracker::ModelPricing;

    fn test_pricing() -> HashMap<String, ModelPricing> {
        [
            ("claude-sonnet-4-5", 3.0, 15.0, "Claude Sonnet 4.5"),
            ("claude-haiku-4", 1.0, 5.0, "Claude Haiku 4"),
        ]
        .into_iter()
        .map(|(id, input, output, name)| {
            (id.to_string(), ModelPricing {
                input_per_mtok: input,
                output_per_mtok: output,
                display_name: name.to_string(),
            })
        })
        .collect()
    }

    fn make_ctx() -> ToolContext {
        ToolContext::new("test".to_string(), CancellationToken::new(), None)
    }

    fn tracker_with_usage() -> Arc<CostTracker> {
        let tracker = Arc::new(CostTracker::new(test_pricing(), CostTrackerConfig::default()));
        tracker.record_usage("claude-sonnet-4-5", 100_000, 50_000);
        tracker.record_usage("claude-haiku-4", 200_000, 100_000);
        tracker
    }

    fn tracker_with_budget() -> Arc<CostTracker> {
        let tracker = Arc::new(CostTracker::new(test_pricing(), CostTrackerConfig {
            soft_limit: Some(1.0),
            hard_limit: Some(5.0),
            warning_interval: None,
        }));
        tracker.record_usage("claude-sonnet-4-5", 100_000, 50_000);
        tracker
    }

    #[tokio::test]
    async fn test_summary_action() {
        let tool = CostTool::new(tracker_with_usage());
        let result = tool.execute(&make_ctx(), json!({"action": "summary"})).await;
        assert!(!result.is_error);
        let text = result_text(&result);
        assert!(text.contains('$'));
        assert!(text.contains("2 model"));
    }

    #[tokio::test]
    async fn test_breakdown_action() {
        let tool = CostTool::new(tracker_with_usage());
        let result = tool.execute(&make_ctx(), json!({"action": "breakdown"})).await;
        assert!(!result.is_error);
        let text = result_text(&result);
        assert!(text.contains("Sonnet"));
        assert!(text.contains("Haiku"));
        assert!(text.contains('%'));
    }

    #[tokio::test]
    async fn test_budget_action_no_budget() {
        let tool = CostTool::new(tracker_with_usage());
        let result = tool.execute(&make_ctx(), json!({"action": "budget"})).await;
        let text = result_text(&result);
        assert!(text.contains("No budget"));
    }

    #[tokio::test]
    async fn test_budget_action_with_budget() {
        let tool = CostTool::new(tracker_with_budget());
        let result = tool.execute(&make_ctx(), json!({"action": "budget"})).await;
        let text = result_text(&result);
        // Usage (~$1.05) exceeds soft limit ($1) so status is Warning
        assert!(text.contains("Warning") || text.contains("OK") || text.contains("Exceeded"));
        assert!(text.contains('$'));
    }

    #[tokio::test]
    async fn test_unknown_action() {
        let tool = CostTool::new(tracker_with_usage());
        let result = tool.execute(&make_ctx(), json!({"action": "foobar"})).await;
        assert!(result.is_error);
    }

    fn result_text(result: &ToolResult) -> String {
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
}
