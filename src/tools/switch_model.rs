//! switch_model tool — agent-initiated model switching
//!
//! Lets the agent request a different model mid-conversation when it
//! realizes the current task is simpler or harder than expected. The
//! switch takes effect on the next LLM call within the same turn loop.

use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::Mutex;
use serde_json::Value;
use serde_json::json;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;
use crate::config::model_roles::ModelRoles;
use crate::routing::cost_tracker::CostTracker;

/// Shared slot the turn loop reads after each tool execution round.
/// When `Some(model_id)`, the loop switches to that model for the next
/// LLM call, then clears the slot.
pub type ModelSwitchSlot = Arc<Mutex<Option<String>>>;

pub fn model_switch_slot() -> ModelSwitchSlot {
    Arc::new(Mutex::new(None))
}

pub struct SwitchModelTool {
    /// Slot the turn loop polls for a pending model switch
    switch_slot: ModelSwitchSlot,
    /// Roles registry for resolving role names to model IDs
    model_roles: ModelRoles,
    /// Cost tracker for budget validation (optional)
    cost_tracker: Option<Arc<CostTracker>>,
    /// Current model ID (to report what we switched from)
    current_model: Arc<Mutex<String>>,
    /// Hard budget limit — disallow upgrades when exceeded
    budget_hard_limit: Option<f64>,
}

impl SwitchModelTool {
    pub fn new(
        switch_slot: ModelSwitchSlot,
        model_roles: ModelRoles,
        current_model: Arc<Mutex<String>>,
    ) -> Self {
        Self {
            switch_slot,
            model_roles,
            cost_tracker: None,
            current_model,
            budget_hard_limit: None,
        }
    }

    pub fn with_cost_tracker(mut self, tracker: Arc<CostTracker>) -> Self {
        self.cost_tracker = Some(tracker);
        self
    }

    pub fn with_budget_hard_limit(mut self, limit: f64) -> Self {
        self.budget_hard_limit = Some(limit);
        self
    }

    /// Check if the requested model is more expensive than the current one.
    fn is_upgrade(&self, from: &str, to: &str) -> bool {
        // Simple heuristic: if the target model ID contains "opus" and
        // the source doesn't, it's an upgrade. Similarly for sonnet→opus.
        let rank = |m: &str| -> u8 {
            let lower = m.to_lowercase();
            if lower.contains("opus") {
                3
            } else if lower.contains("sonnet") {
                2
            } else if lower.contains("haiku") {
                1
            } else {
                2 // unknown defaults to mid-tier
            }
        };
        rank(to) > rank(from)
    }
}

#[async_trait]
impl Tool for SwitchModelTool {
    fn definition(&self) -> &ToolDefinition {
        // Leak a static definition (standard pattern for tool impls)
        static DEF: std::sync::OnceLock<ToolDefinition> = std::sync::OnceLock::new();
        DEF.get_or_init(|| ToolDefinition {
            name: "switch_model".to_string(),
            description: "Switch to a different model mid-conversation. Use when the current \
                task is simpler than expected (switch to a faster model) or harder than \
                expected (switch to a more capable model). The switch takes effect on the \
                next response."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "role": {
                        "type": "string",
                        "description": "Model role to switch to. Options: 'smol' (fast/cheap, \
                            good for simple tasks like grep/list/read), 'default' (balanced, \
                            general-purpose), 'slow' (powerful, for complex reasoning/refactoring). \
                            Can also use any custom role name defined in settings."
                    },
                    "reason": {
                        "type": "string",
                        "description": "Brief justification for the switch (logged for transparency)"
                    }
                },
                "required": ["role", "reason"]
            }),
        })
    }

    async fn execute(&self, _ctx: &ToolContext, params: Value) -> ToolResult {
        let role = params
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("default");

        let reason = params
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("no reason given");

        // Resolve role to model ID
        let current = self.current_model.lock().clone();
        let new_model = self.model_roles.resolve(role, &current);

        if new_model == current {
            return ToolResult::text(format!(
                "Already using {} (role '{}'). No switch needed.",
                current, role,
            ));
        }

        // Budget check: disallow upgrades when over hard limit
        if let Some(hard_limit) = self.budget_hard_limit {
            if let Some(tracker) = &self.cost_tracker {
                let total = tracker.total_cost();
                if total >= hard_limit && self.is_upgrade(&current, &new_model) {
                    return ToolResult::error(format!(
                        "Cannot upgrade to {} — budget exceeded (${:.2}/${:.2}). \
                         Try a cheaper model or continue with the current one.",
                        new_model, total, hard_limit,
                    ));
                }
            }
        }

        // Write to the shared slot — the turn loop picks this up
        *self.switch_slot.lock() = Some(new_model.clone());

        ToolResult::text(format!(
            "Switching from {} to {} (role '{}').\nReason: {}\n\
             The new model will handle the next response.",
            current, new_model, role, reason,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (SwitchModelTool, ModelSwitchSlot) {
        let slot = model_switch_slot();
        let mut roles = ModelRoles::with_defaults();
        roles.set_model("smol", "claude-haiku-4".to_string());
        roles.set_model("default", "claude-sonnet-4-5".to_string());
        roles.set_model("slow", "claude-opus-4".to_string());

        let current = Arc::new(Mutex::new("claude-sonnet-4-5".to_string()));
        let tool = SwitchModelTool::new(slot.clone(), roles, current);
        (tool, slot)
    }

    fn make_ctx() -> ToolContext {
        ToolContext::new(
            "test-call".to_string(),
            CancellationToken::new(),
            None,
        )
    }

    use tokio_util::sync::CancellationToken;

    #[tokio::test]
    async fn test_switch_to_smol() {
        let (tool, slot) = setup();
        let ctx = make_ctx();
        let result = tool
            .execute(&ctx, json!({"role": "smol", "reason": "task is simple"}))
            .await;
        assert!(!result.is_error);
        assert_eq!(*slot.lock(), Some("claude-haiku-4".to_string()));
    }

    #[tokio::test]
    async fn test_switch_to_slow() {
        let (tool, slot) = setup();
        let ctx = make_ctx();
        let result = tool
            .execute(&ctx, json!({"role": "slow", "reason": "need deep reasoning"}))
            .await;
        assert!(!result.is_error);
        assert_eq!(*slot.lock(), Some("claude-opus-4".to_string()));
    }

    #[tokio::test]
    async fn test_switch_same_model_noop() {
        let (tool, slot) = setup();
        let ctx = make_ctx();
        let result = tool
            .execute(&ctx, json!({"role": "default", "reason": "just checking"}))
            .await;
        assert!(!result.is_error);
        assert!(slot.lock().is_none()); // no switch needed
        let text = match &result.content[0] {
            crate::tools::ToolResultContent::Text { text } => text.as_str(),
            _ => panic!("expected text content"),
        };
        assert!(text.contains("Already using"));
    }

    #[tokio::test]
    async fn test_budget_blocks_upgrade() {
        let slot = model_switch_slot();
        let mut roles = ModelRoles::with_defaults();
        roles.set_model("smol", "claude-haiku-4".to_string());
        roles.set_model("slow", "claude-opus-4".to_string());

        let current = Arc::new(Mutex::new("claude-haiku-4".to_string()));
        let tracker = Arc::new(CostTracker::with_defaults());
        // Record enough usage to exceed budget
        tracker.record_usage("claude-haiku-4", 10_000_000, 5_000_000);

        let tool = SwitchModelTool::new(slot.clone(), roles, current)
            .with_cost_tracker(tracker)
            .with_budget_hard_limit(1.0); // $1 limit

        let ctx = make_ctx();
        let result = tool
            .execute(&ctx, json!({"role": "slow", "reason": "want opus"}))
            .await;
        assert!(result.is_error);
        assert!(slot.lock().is_none()); // no switch happened
    }

    #[tokio::test]
    async fn test_budget_allows_downgrade() {
        let slot = model_switch_slot();
        let mut roles = ModelRoles::with_defaults();
        roles.set_model("smol", "claude-haiku-4".to_string());
        roles.set_model("slow", "claude-opus-4".to_string());

        let current = Arc::new(Mutex::new("claude-opus-4".to_string()));
        let tracker = Arc::new(CostTracker::with_defaults());
        tracker.record_usage("claude-opus-4", 10_000_000, 5_000_000);

        let tool = SwitchModelTool::new(slot.clone(), roles, current)
            .with_cost_tracker(tracker)
            .with_budget_hard_limit(1.0);

        let ctx = make_ctx();
        let result = tool
            .execute(&ctx, json!({"role": "smol", "reason": "save money"}))
            .await;
        assert!(!result.is_error); // downgrade always allowed
        assert_eq!(*slot.lock(), Some("claude-haiku-4".to_string()));
    }

    #[tokio::test]
    async fn test_missing_role_defaults_to_default() {
        let (tool, slot) = setup();
        let ctx = make_ctx();
        // Omit "role" — defaults to "default", which is already the current model
        let result = tool.execute(&ctx, json!({"reason": "testing"})).await;
        assert!(!result.is_error);
        assert!(slot.lock().is_none()); // default == current, no switch
    }

    #[tokio::test]
    async fn test_missing_reason_still_works() {
        let (tool, slot) = setup();
        let ctx = make_ctx();
        let result = tool.execute(&ctx, json!({"role": "smol"})).await;
        assert!(!result.is_error);
        assert_eq!(*slot.lock(), Some("claude-haiku-4".to_string()));
        let text = match &result.content[0] {
            crate::tools::ToolResultContent::Text { text } => text.as_str(),
            _ => panic!("expected text"),
        };
        assert!(text.contains("no reason given"));
    }

    #[tokio::test]
    async fn test_unknown_role_resolves_to_current() {
        let (tool, slot) = setup();
        let ctx = make_ctx();
        // Unknown role falls through to fallback (current model)
        let result = tool
            .execute(&ctx, json!({"role": "nonexistent", "reason": "test"}))
            .await;
        assert!(!result.is_error);
        assert!(slot.lock().is_none()); // resolves to current
    }

    #[tokio::test]
    async fn test_budget_at_exact_limit_blocks_upgrade() {
        let slot = model_switch_slot();
        let mut roles = ModelRoles::with_defaults();
        roles.set_model("smol", "claude-haiku-4".to_string());
        roles.set_model("slow", "claude-opus-4".to_string());

        let current = Arc::new(Mutex::new("claude-haiku-4".to_string()));
        let tracker = Arc::new(CostTracker::with_defaults());
        // Record usage to land exactly at $1.00 (haiku: $1/MTok input, $5/MTok output)
        tracker.record_usage("claude-haiku-4", 1_000_000, 0);

        let tool = SwitchModelTool::new(slot.clone(), roles, current)
            .with_cost_tracker(tracker)
            .with_budget_hard_limit(1.0);

        let ctx = make_ctx();
        let result = tool
            .execute(&ctx, json!({"role": "slow", "reason": "want opus"}))
            .await;
        assert!(result.is_error);
        let text = match &result.content[0] {
            crate::tools::ToolResultContent::Text { text } => text.as_str(),
            _ => panic!("expected text"),
        };
        assert!(text.contains("budget exceeded"));
    }

    #[tokio::test]
    async fn test_no_budget_tracker_allows_upgrade() {
        let slot = model_switch_slot();
        let mut roles = ModelRoles::with_defaults();
        roles.set_model("smol", "claude-haiku-4".to_string());
        roles.set_model("slow", "claude-opus-4".to_string());

        let current = Arc::new(Mutex::new("claude-haiku-4".to_string()));
        // Has hard limit but no cost tracker — should still allow the switch
        let tool = SwitchModelTool::new(slot.clone(), roles, current)
            .with_budget_hard_limit(1.0);

        let ctx = make_ctx();
        let result = tool
            .execute(&ctx, json!({"role": "slow", "reason": "want opus"}))
            .await;
        assert!(!result.is_error);
        assert_eq!(*slot.lock(), Some("claude-opus-4".to_string()));
    }

    #[test]
    fn test_is_upgrade() {
        let (tool, _) = setup();
        assert!(tool.is_upgrade("claude-haiku-4", "claude-opus-4"));
        assert!(tool.is_upgrade("claude-haiku-4", "claude-sonnet-4-5"));
        assert!(tool.is_upgrade("claude-sonnet-4-5", "claude-opus-4"));
        assert!(!tool.is_upgrade("claude-opus-4", "claude-haiku-4"));
        assert!(!tool.is_upgrade("claude-opus-4", "claude-opus-4"));
    }
}
