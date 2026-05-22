use std::collections::HashMap;
use std::sync::Arc;

use clankers_artifacts::ArtifactHash;
use clankers_provider::message::AgentMessage;
use clankers_runtime::DEFAULT_TURN_PLANNING_SEAM;
use clankers_runtime::OrchestrationCandidate;
use clankers_runtime::OrchestrationPlanReceipt;
use clankers_runtime::OrchestrationPlanStatus;
use clankers_runtime::OrchestrationRolloutStage;
use clankers_runtime::STEEL_ORCHESTRATION_PLAN_SCHEMA;
use clankers_runtime::SteelOrchestrationProfile;
use clankers_runtime::TurnPlanningInput;
use clankers_runtime::plan_turn_with_steel_or_fallback;
use serde::Serialize;
use tokio::sync::broadcast;

use crate::events::AgentEvent;
use crate::tool::Tool;

const AGENT_TURN_DECISION_ID: &str = "agent-turn-model-request";
const AGENT_TURN_DECISION_CLASS: &str = "agent-turn-plan";
const DEFAULT_STEEL_SOURCE: &str = "(host \"steel.host.plan_turn\")";

#[derive(Debug, Clone)]
pub struct AgentTurnSteelPlanningConfig {
    pub profile: SteelOrchestrationProfile,
    pub steel_source: String,
    pub steel_plan_payload: String,
    pub session_capabilities: Vec<String>,
    pub granted_ucan_abilities: Vec<String>,
    pub disabled_actions: Vec<String>,
}

impl AgentTurnSteelPlanningConfig {
    #[must_use]
    pub fn comparison_fixture(profile: SteelOrchestrationProfile) -> Self {
        let target_resource = "session:fixture";
        Self {
            steel_plan_payload: format!(
                "{STEEL_ORCHESTRATION_PLAN_SCHEMA}|{AGENT_TURN_DECISION_ID}|{DEFAULT_TURN_PLANNING_SEAM}|{target_resource}|{AGENT_TURN_DECISION_CLASS}"
            ),
            steel_source: DEFAULT_STEEL_SOURCE.to_string(),
            session_capabilities: profile.required_session_capabilities.clone(),
            granted_ucan_abilities: vec![profile.required_ucan_ability.clone()],
            disabled_actions: Vec::new(),
            profile,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentTurnExecutionPlanner {
    RustNative,
    SteelScheme,
    Blocked,
}

#[derive(Debug, Clone)]
pub struct AgentTurnPlanningOutcome {
    pub receipt: OrchestrationPlanReceipt,
    pub execution_planner: AgentTurnExecutionPlanner,
}

pub(crate) struct AgentTurnPlanningRequest<'a> {
    pub config: &'a AgentTurnSteelPlanningConfig,
    pub session_id: &'a str,
    pub model: &'a str,
    pub system_prompt: &'a str,
    pub messages: &'a [AgentMessage],
    pub tools: &'a HashMap<String, Arc<dyn Tool>>,
}

#[must_use]
pub(crate) fn plan_agent_turn(request: AgentTurnPlanningRequest<'_>) -> AgentTurnPlanningOutcome {
    let input = turn_planning_input(&request);
    let receipt = plan_turn_with_steel_or_fallback(&request.config.profile, &input);
    let execution_planner = execution_planner_for_receipt(&request.config.profile, &receipt);
    AgentTurnPlanningOutcome {
        receipt,
        execution_planner,
    }
}

pub(crate) fn emit_agent_turn_planning_receipt(
    event_tx: &broadcast::Sender<AgentEvent>,
    outcome: &AgentTurnPlanningOutcome,
) {
    let receipt = &outcome.receipt;
    let message = format!(
        "steel.host.plan_turn receipt status={:?} issue={:?} mode={:?} planner={:?} fallback={:?} receipt_hash={} plan_hash={}",
        receipt.status,
        receipt.issue_code,
        receipt.rollout_stage,
        receipt.planner,
        receipt.fallback_status,
        receipt.receipt_hash.prefixed(),
        receipt.plan_hash.map_or_else(|| "none".to_string(), ArtifactHash::prefixed),
    );
    event_tx.send(AgentEvent::SystemMessage { message }).ok();
}

fn execution_planner_for_receipt(
    profile: &SteelOrchestrationProfile,
    receipt: &OrchestrationPlanReceipt,
) -> AgentTurnExecutionPlanner {
    if receipt.status == OrchestrationPlanStatus::Blocked {
        return AgentTurnExecutionPlanner::Blocked;
    }
    if profile.rollout_stage == OrchestrationRolloutStage::Default
        && receipt.status == OrchestrationPlanStatus::Authorized
    {
        return AgentTurnExecutionPlanner::SteelScheme;
    }
    AgentTurnExecutionPlanner::RustNative
}

fn turn_planning_input(request: &AgentTurnPlanningRequest<'_>) -> TurnPlanningInput {
    let target_resource = format!("session:{}", safe_route_token(request.session_id));
    let turn_material = TurnPlanningHashMaterial {
        session_id: request.session_id,
        model: request.model,
        system_prompt_bytes: request.system_prompt.len(),
        message_count: request.messages.len(),
        tool_names: sorted_tool_names(request.tools),
    };
    let turn_bytes = stable_json_bytes(&turn_material);
    let prompt_hash = ArtifactHash::digest(&turn_bytes);
    let input_hash = ArtifactHash::digest(format!("{prompt_hash:?}:{DEFAULT_TURN_PLANNING_SEAM}").as_bytes());
    TurnPlanningInput {
        turn_id: format!("{}:{}", safe_route_token(request.session_id), request.messages.len()),
        prompt_hash,
        prompt_bytes: turn_bytes.len() as u64,
        candidate_actions: vec![OrchestrationCandidate {
            decision_id: AGENT_TURN_DECISION_ID.to_string(),
            decision_class: AGENT_TURN_DECISION_CLASS.to_string(),
            action_name: DEFAULT_TURN_PLANNING_SEAM.to_string(),
            target_resource,
            required_ucan_ability: request.config.profile.required_ucan_ability.clone(),
            required_session_capabilities: request.config.profile.required_session_capabilities.clone(),
            input_hash,
            input_bytes: turn_bytes.len() as u64,
        }],
        steel_source: request.config.steel_source.clone(),
        steel_plan_payload: steel_plan_payload_for_session(request),
        session_capabilities: request.config.session_capabilities.clone(),
        disabled_actions: request.config.disabled_actions.clone(),
        granted_ucan_abilities: request.config.granted_ucan_abilities.clone(),
    }
}

fn steel_plan_payload_for_session(request: &AgentTurnPlanningRequest<'_>) -> String {
    let expected_fixture_target = "session:fixture";
    let session_target = format!("session:{}", safe_route_token(request.session_id));
    request.config.steel_plan_payload.replace(expected_fixture_target, &session_target)
}

fn sorted_tool_names(tools: &HashMap<String, Arc<dyn Tool>>) -> Vec<&str> {
    let mut names = tools.keys().map(String::as_str).collect::<Vec<_>>();
    names.sort_unstable();
    names
}

fn stable_json_bytes<T: Serialize>(value: &T) -> Vec<u8> {
    serde_json::to_vec(value).unwrap_or_else(|_| b"serialization-failed".to_vec())
}

fn safe_route_token(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

#[derive(Serialize)]
struct TurnPlanningHashMaterial<'a> {
    session_id: &'a str,
    model: &'a str,
    system_prompt_bytes: usize,
    message_count: usize,
    tool_names: Vec<&'a str>,
}

#[cfg(test)]
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(no_panic, no_unwrap, reason = "test code — panics are assertions")
)]
mod tests {
    use std::collections::HashMap;

    use clankers_artifacts::ArtifactHash;
    use clankers_runtime::OrchestrationFallbackMode;
    use clankers_runtime::OrchestrationPlanStatus;
    use clankers_runtime::OrchestrationRolloutStage;
    use clankers_runtime::SteelOrchestrationProfile;

    use super::*;

    fn profile() -> SteelOrchestrationProfile {
        SteelOrchestrationProfile::comparison_default(ArtifactHash::digest(b"script"), ArtifactHash::digest(b"policy"))
    }

    fn request<'a>(
        config: &'a AgentTurnSteelPlanningConfig,
        tools: &'a HashMap<String, Arc<dyn Tool>>,
    ) -> AgentTurnPlanningRequest<'a> {
        AgentTurnPlanningRequest {
            config,
            session_id: "session-fixture",
            model: "model-fixture",
            system_prompt: "secret prompt body is not emitted",
            messages: &[],
            tools,
        }
    }

    fn plan(config: &AgentTurnSteelPlanningConfig) -> AgentTurnPlanningOutcome {
        let tools = HashMap::new();
        plan_agent_turn(request(config, &tools))
    }

    #[test]
    fn comparison_mode_invokes_steel_but_executes_rust_native() {
        let config = AgentTurnSteelPlanningConfig::comparison_fixture(profile());
        let outcome = plan(&config);
        assert_eq!(outcome.receipt.status, OrchestrationPlanStatus::Authorized);
        assert_eq!(outcome.receipt.rollout_stage, OrchestrationRolloutStage::Comparison);
        assert_eq!(outcome.execution_planner, AgentTurnExecutionPlanner::RustNative);
        assert!(outcome.receipt.steel_receipt_hash.is_some());
        assert_eq!(outcome.receipt.authorization_receipts.len(), 1);
    }

    #[test]
    fn default_mode_selects_steel_after_rust_authorization() {
        let mut config = AgentTurnSteelPlanningConfig::comparison_fixture(profile());
        config.profile.rollout_stage = OrchestrationRolloutStage::Default;
        let outcome = plan(&config);
        assert_eq!(outcome.receipt.status, OrchestrationPlanStatus::Authorized);
        assert_eq!(outcome.execution_planner, AgentTurnExecutionPlanner::SteelScheme);
        assert_eq!(outcome.receipt.authorization_receipts.len(), 1);
    }

    #[test]
    fn disabled_profile_stays_rust_native_without_steel_receipt() {
        let mut config = AgentTurnSteelPlanningConfig::comparison_fixture(profile());
        config.profile.enabled = false;
        config.profile.default = false;
        config.profile.rollout_stage = OrchestrationRolloutStage::Disabled;
        let outcome = plan(&config);
        assert_eq!(outcome.execution_planner, AgentTurnExecutionPlanner::RustNative);
        assert!(outcome.receipt.steel_receipt_hash.is_none());
        assert_eq!(outcome.receipt.status, OrchestrationPlanStatus::FallbackUsed);
    }

    #[test]
    fn malformed_plan_uses_receipt_backed_fallback() {
        let mut config = AgentTurnSteelPlanningConfig::comparison_fixture(profile());
        config.steel_plan_payload = "not-a-typed-plan".to_string();
        let outcome = plan(&config);
        assert_eq!(outcome.execution_planner, AgentTurnExecutionPlanner::RustNative);
        assert_eq!(outcome.receipt.status, OrchestrationPlanStatus::FallbackUsed);
        assert!(outcome.receipt.steel_receipt_hash.is_some());
    }

    #[test]
    fn fallback_disabled_blocks_failed_steel_plan() {
        let mut config = AgentTurnSteelPlanningConfig::comparison_fixture(profile());
        config.profile.fallback_mode = OrchestrationFallbackMode::Block;
        config.steel_plan_payload = "not-a-typed-plan".to_string();
        let outcome = plan(&config);
        assert_eq!(outcome.execution_planner, AgentTurnExecutionPlanner::Blocked);
        assert_eq!(outcome.receipt.status, OrchestrationPlanStatus::Blocked);
    }

    #[test]
    fn denied_host_action_does_not_select_steel_execution() {
        let mut config = AgentTurnSteelPlanningConfig::comparison_fixture(profile());
        config.granted_ucan_abilities.clear();
        let outcome = plan(&config);
        assert_eq!(outcome.execution_planner, AgentTurnExecutionPlanner::RustNative);
        assert_eq!(outcome.receipt.status, OrchestrationPlanStatus::Denied);
    }

    #[test]
    fn repeated_receipts_are_stable_and_redacted() {
        let config = AgentTurnSteelPlanningConfig::comparison_fixture(profile());
        let first = plan(&config);
        let second = plan(&config);
        assert_eq!(first.receipt.receipt_hash, second.receipt.receipt_hash);
        assert!(!first.receipt.safe_summary.contains("secret prompt body"));
    }
}
