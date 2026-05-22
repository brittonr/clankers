//! Steel Scheme default orchestration planner seam.
//!
//! Steel is a trusted planner/requester at this seam. It returns a typed plan
//! through the Clankers-owned Steel runtime wrapper; Rust parses that plan,
//! authorizes every dynamic-runtime action envelope, and owns fallback receipts.
//! No caller in CLI/daemon/TUI/provider/tool-host code should construct Steel
//! interpreter internals directly.

use std::collections::BTreeSet;

use clankers_artifacts::ArtifactHash;
use serde::Deserialize;
use serde::Serialize;

use crate::dynamic_runtime::DYNAMIC_RUNTIME_ACTION_SCHEMA;
use crate::dynamic_runtime::DynamicRuntimeActionEnvelope;
use crate::dynamic_runtime::DynamicRuntimeActionKind;
#[cfg(test)]
use crate::dynamic_runtime::DynamicRuntimeActionReason;
use crate::dynamic_runtime::DynamicRuntimeActionReceipt;
use crate::dynamic_runtime::DynamicRuntimeActionStatus;
use crate::dynamic_runtime::DynamicRuntimeAuthorizationContext;
use crate::dynamic_runtime::DynamicRuntimeKind;
use crate::dynamic_runtime::DynamicRuntimeRedactionClass;
use crate::dynamic_runtime::authorize_dynamic_runtime_action;
use crate::steel_runtime::SteelHostFunctionRegistration;
use crate::steel_runtime::SteelRuntimeProfile;
use crate::steel_runtime::SteelRuntimeReasonCode;
use crate::steel_runtime::SteelRuntimeRequest;
use crate::steel_runtime::SteelRuntimeStatusCode;
use crate::steel_runtime::evaluate_steel_request;

pub const STEEL_ORCHESTRATION_PLAN_SCHEMA: &str = "clankers.steel_orchestration.plan.v1";
pub const STEEL_ORCHESTRATION_RECEIPT_SCHEMA: &str = "clankers.steel_orchestration.receipt.v1";
pub const DEFAULT_TURN_PLANNING_SEAM: &str = "steel.host.plan_turn";
const DEFAULT_RECEIPT_PREFIX: &str = "target/steel-default-orchestration";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelOrchestrationProfile {
    pub name: String,
    pub enabled: bool,
    pub default: bool,
    pub planning_seam: String,
    pub rollout_stage: OrchestrationRolloutStage,
    pub fallback_mode: OrchestrationFallbackMode,
    pub script_id: String,
    pub script_hash: ArtifactHash,
    pub policy_hash: ArtifactHash,
    pub runtime_profile: SteelRuntimeProfile,
    pub allowed_host_actions: BTreeSet<String>,
    pub required_session_capabilities: Vec<String>,
    pub required_ucan_ability: String,
    pub receipt_prefix: String,
    pub max_input_bytes: u64,
}

impl SteelOrchestrationProfile {
    #[must_use]
    pub fn comparison_default(script_hash: ArtifactHash, policy_hash: ArtifactHash) -> Self {
        let mut allowed_host_actions = BTreeSet::new();
        allowed_host_actions.insert(DEFAULT_TURN_PLANNING_SEAM.to_string());
        Self {
            name: "steel-plan-turn-default".to_string(),
            enabled: true,
            default: true,
            planning_seam: DEFAULT_TURN_PLANNING_SEAM.to_string(),
            rollout_stage: OrchestrationRolloutStage::Comparison,
            fallback_mode: OrchestrationFallbackMode::RustNative,
            script_id: "default-plan-turn-v1".to_string(),
            script_hash,
            policy_hash,
            runtime_profile: SteelRuntimeProfile::default_deny(),
            allowed_host_actions,
            required_session_capabilities: vec!["steel-orchestration".to_string(), "turn-planning".to_string()],
            required_ucan_ability: "clankers/steel/orchestrate.plan_turn".to_string(),
            receipt_prefix: DEFAULT_RECEIPT_PREFIX.to_string(),
            max_input_bytes: 8192,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrchestrationRolloutStage {
    Disabled,
    Comparison,
    Default,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrchestrationFallbackMode {
    RustNative,
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnPlanningInput {
    pub turn_id: String,
    pub prompt_hash: ArtifactHash,
    pub prompt_bytes: u64,
    pub candidate_actions: Vec<OrchestrationCandidate>,
    pub steel_source: String,
    pub steel_plan_payload: String,
    pub session_capabilities: Vec<String>,
    pub disabled_actions: Vec<String>,
    pub granted_ucan_abilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchestrationCandidate {
    pub decision_id: String,
    pub decision_class: String,
    pub action_name: String,
    pub target_resource: String,
    pub required_ucan_ability: String,
    pub required_session_capabilities: Vec<String>,
    pub input_hash: ArtifactHash,
    pub input_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchestrationPlan {
    pub schema: String,
    pub seam: String,
    pub planner: OrchestrationPlannerKind,
    pub decisions: Vec<OrchestrationDecision>,
    pub plan_hash: ArtifactHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchestrationDecision {
    pub decision_id: String,
    pub decision_class: String,
    pub action: DynamicRuntimeActionEnvelope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrchestrationPlannerKind {
    SteelScheme,
    RustNative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrchestrationPlanStatus {
    Authorized,
    Denied,
    FallbackUsed,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OrchestrationIssueCode {
    Ok,
    SteelDisabled,
    UnsupportedSeam,
    ScriptEvaluationFailed,
    MalformedPlan,
    FallbackDisabled,
    NoCandidateActions,
    UnauthorizedAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchestrationPlanReceipt {
    pub schema: String,
    pub status: OrchestrationPlanStatus,
    pub issue_code: OrchestrationIssueCode,
    pub seam: String,
    pub profile_name: String,
    pub planner: OrchestrationPlannerKind,
    pub rollout_stage: OrchestrationRolloutStage,
    pub fallback_status: RustNativeFallbackStatus,
    pub script_id: String,
    pub script_hash: ArtifactHash,
    pub policy_hash: ArtifactHash,
    pub plan_hash: Option<ArtifactHash>,
    pub steel_receipt_hash: Option<ArtifactHash>,
    pub rust_native_decision_class: Option<String>,
    pub authorization_receipts: Vec<DynamicRuntimeActionReceipt>,
    pub safe_summary: String,
    pub redactions: Vec<String>,
    pub receipt_hash: ArtifactHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RustNativeFallbackStatus {
    NotNeeded,
    Used,
    Disabled,
    Unavailable,
}

#[must_use]
pub fn plan_turn_with_steel_or_fallback(
    profile: &SteelOrchestrationProfile,
    input: &TurnPlanningInput,
) -> OrchestrationPlanReceipt {
    let fallback_plan = rust_native_turn_plan(profile, input);
    let fallback_decision_class = fallback_plan.decisions.first().map(|decision| decision.decision_class.clone());

    if !profile.enabled || !profile.default {
        return authorize_plan_receipt(
            profile,
            &fallback_plan,
            OrchestrationPlannerKind::RustNative,
            RustNativeFallbackStatus::Used,
            None,
            fallback_decision_class,
            OrchestrationIssueCode::SteelDisabled,
            "Steel orchestration disabled; Rust-native planner selected",
            input,
        );
    }
    if profile.planning_seam != DEFAULT_TURN_PLANNING_SEAM {
        return fallback_or_block(
            profile,
            input,
            &fallback_plan,
            None,
            OrchestrationIssueCode::UnsupportedSeam,
            "selected Steel planning seam is not implemented by this adapter",
            fallback_decision_class,
        );
    }

    let steel_request = SteelRuntimeRequest {
        profile: profile.runtime_profile.clone(),
        source: input.steel_source.clone(),
        session_capabilities: input.session_capabilities.clone(),
        disabled_tools: input.disabled_actions.clone(),
        host_functions: vec![SteelHostFunctionRegistration {
            name: DEFAULT_TURN_PLANNING_SEAM.to_string(),
            required_capability: "steel-orchestration".to_string(),
            output: input.steel_plan_payload.clone(),
        }],
        receipt_destination: format!("{}/steel-runtime.json", profile.receipt_prefix.trim_end_matches('/')),
    };
    let steel_receipt = evaluate_steel_request(&steel_request);
    let steel_receipt_hash = steel_receipt.receipt_hash();
    if steel_receipt.status != SteelRuntimeStatusCode::Succeeded {
        let issue = if steel_receipt.reason_code == SteelRuntimeReasonCode::UnsupportedExpression {
            OrchestrationIssueCode::MalformedPlan
        } else {
            OrchestrationIssueCode::ScriptEvaluationFailed
        };
        return fallback_or_block(
            profile,
            input,
            &fallback_plan,
            Some(steel_receipt_hash),
            issue,
            "Steel script evaluation failed before a typed plan was authorized",
            fallback_decision_class,
        );
    }
    let Some(output) = steel_receipt.output.as_deref() else {
        return fallback_or_block(
            profile,
            input,
            &fallback_plan,
            Some(steel_receipt_hash),
            OrchestrationIssueCode::MalformedPlan,
            "Steel planner returned no typed plan payload",
            fallback_decision_class,
        );
    };
    let Ok(steel_plan) = parse_steel_plan_payload(profile, input, output) else {
        return fallback_or_block(
            profile,
            input,
            &fallback_plan,
            Some(steel_receipt_hash),
            OrchestrationIssueCode::MalformedPlan,
            "Steel planner output did not match the typed plan schema",
            fallback_decision_class,
        );
    };
    authorize_plan_receipt(
        profile,
        &steel_plan,
        OrchestrationPlannerKind::SteelScheme,
        RustNativeFallbackStatus::NotNeeded,
        Some(steel_receipt_hash),
        fallback_decision_class,
        OrchestrationIssueCode::Ok,
        "Steel plan parsed as typed data; Rust authorization receipts decide effects",
        input,
    )
}

#[must_use]
pub fn rust_native_turn_plan(profile: &SteelOrchestrationProfile, input: &TurnPlanningInput) -> OrchestrationPlan {
    let decision = input
        .candidate_actions
        .iter()
        .find(|candidate| !input.disabled_actions.contains(&candidate.action_name))
        .or_else(|| input.candidate_actions.first())
        .map(|candidate| decision_from_candidate(profile, input, candidate, OrchestrationPlannerKind::RustNative));
    build_plan(&profile.planning_seam, OrchestrationPlannerKind::RustNative, decision.into_iter().collect())
}

fn parse_steel_plan_payload(
    profile: &SteelOrchestrationProfile,
    input: &TurnPlanningInput,
    payload: &str,
) -> Result<OrchestrationPlan, ()> {
    let mut parts = payload.split('|');
    let schema = parts.next().ok_or(())?;
    let decision_id = parts.next().ok_or(())?;
    let action_name = parts.next().ok_or(())?;
    let target_resource = parts.next().ok_or(())?;
    let decision_class = parts.next().ok_or(())?;
    if parts.next().is_some() || schema != STEEL_ORCHESTRATION_PLAN_SCHEMA {
        return Err(());
    }
    let candidate = input
        .candidate_actions
        .iter()
        .find(|candidate| {
            candidate.decision_id == decision_id
                && candidate.action_name == action_name
                && candidate.target_resource == target_resource
        })
        .ok_or(())?;
    let mut selected = candidate.clone();
    selected.decision_class = decision_class.to_string();
    Ok(build_plan(&profile.planning_seam, OrchestrationPlannerKind::SteelScheme, vec![
        decision_from_candidate(profile, input, &selected, OrchestrationPlannerKind::SteelScheme),
    ]))
}

fn decision_from_candidate(
    profile: &SteelOrchestrationProfile,
    input: &TurnPlanningInput,
    candidate: &OrchestrationCandidate,
    planner: OrchestrationPlannerKind,
) -> OrchestrationDecision {
    let action_id = format!(
        "{}:{}:{}",
        match planner {
            OrchestrationPlannerKind::SteelScheme => "steel",
            OrchestrationPlannerKind::RustNative => "rust",
        },
        route_slug(&input.turn_id),
        route_slug(&candidate.decision_id)
    );
    OrchestrationDecision {
        decision_id: candidate.decision_id.clone(),
        decision_class: candidate.decision_class.clone(),
        action: DynamicRuntimeActionEnvelope {
            schema: DYNAMIC_RUNTIME_ACTION_SCHEMA.to_string(),
            action_id,
            runtime: DynamicRuntimeKind::SteelScheme,
            runtime_profile: profile.runtime_profile.name.clone(),
            action_kind: DynamicRuntimeActionKind::HostFunction,
            action_name: candidate.action_name.clone(),
            target_resource: candidate.target_resource.clone(),
            receipt_destination: format!(
                "{}/{}.json",
                profile.receipt_prefix.trim_end_matches('/'),
                route_slug(&candidate.decision_id)
            ),
            required_ucan_ability: candidate.required_ucan_ability.clone(),
            required_session_capabilities: candidate.required_session_capabilities.clone(),
            input_hash: candidate.input_hash,
            input_bytes: candidate.input_bytes,
            redaction: DynamicRuntimeRedactionClass::MetadataOnly,
        },
    }
}

fn build_plan(
    seam: &str,
    planner: OrchestrationPlannerKind,
    decisions: Vec<OrchestrationDecision>,
) -> OrchestrationPlan {
    let mut plan = OrchestrationPlan {
        schema: STEEL_ORCHESTRATION_PLAN_SCHEMA.to_string(),
        seam: seam.to_string(),
        planner,
        decisions,
        plan_hash: ArtifactHash::digest(b"pending"),
    };
    plan.plan_hash = plan_hash(&plan);
    plan
}

fn authorize_plan_receipt(
    profile: &SteelOrchestrationProfile,
    plan: &OrchestrationPlan,
    planner: OrchestrationPlannerKind,
    fallback_status: RustNativeFallbackStatus,
    steel_receipt_hash: Option<ArtifactHash>,
    rust_native_decision_class: Option<String>,
    prior_issue: OrchestrationIssueCode,
    safe_summary: &str,
    input: &TurnPlanningInput,
) -> OrchestrationPlanReceipt {
    if plan.decisions.is_empty() {
        return orchestration_receipt(
            profile,
            OrchestrationPlanStatus::Blocked,
            OrchestrationIssueCode::NoCandidateActions,
            planner,
            fallback_status,
            None,
            steel_receipt_hash,
            rust_native_decision_class,
            Vec::new(),
            "no candidate actions were available for planning".to_string(),
        );
    }
    let context = authorization_context(profile, input);
    let authorization_receipts = plan
        .decisions
        .iter()
        .map(|decision| authorize_dynamic_runtime_action(&decision.action, &context))
        .collect::<Vec<_>>();
    let authorized = authorization_receipts.iter().all(|receipt| receipt.status == DynamicRuntimeActionStatus::Allowed);
    let (status, issue_code) = if authorized {
        let status = if fallback_status == RustNativeFallbackStatus::Used {
            OrchestrationPlanStatus::FallbackUsed
        } else {
            OrchestrationPlanStatus::Authorized
        };
        (status, prior_issue)
    } else {
        (OrchestrationPlanStatus::Denied, OrchestrationIssueCode::UnauthorizedAction)
    };
    orchestration_receipt(
        profile,
        status,
        issue_code,
        planner,
        fallback_status,
        Some(plan.plan_hash),
        steel_receipt_hash,
        rust_native_decision_class,
        authorization_receipts,
        safe_summary.to_string(),
    )
}

fn fallback_or_block(
    profile: &SteelOrchestrationProfile,
    input: &TurnPlanningInput,
    fallback_plan: &OrchestrationPlan,
    steel_receipt_hash: Option<ArtifactHash>,
    issue_code: OrchestrationIssueCode,
    safe_summary: &str,
    rust_native_decision_class: Option<String>,
) -> OrchestrationPlanReceipt {
    match profile.fallback_mode {
        OrchestrationFallbackMode::RustNative => authorize_plan_receipt(
            profile,
            fallback_plan,
            OrchestrationPlannerKind::RustNative,
            RustNativeFallbackStatus::Used,
            steel_receipt_hash,
            rust_native_decision_class,
            issue_code,
            safe_summary,
            input,
        ),
        OrchestrationFallbackMode::Block => orchestration_receipt(
            profile,
            OrchestrationPlanStatus::Blocked,
            OrchestrationIssueCode::FallbackDisabled,
            OrchestrationPlannerKind::SteelScheme,
            RustNativeFallbackStatus::Disabled,
            None,
            steel_receipt_hash,
            rust_native_decision_class,
            Vec::new(),
            safe_summary.to_string(),
        ),
    }
}

fn authorization_context(
    profile: &SteelOrchestrationProfile,
    input: &TurnPlanningInput,
) -> DynamicRuntimeAuthorizationContext {
    let allowed_actions = profile
        .allowed_host_actions
        .iter()
        .map(|action| format!("host_function:{action}"))
        .collect::<BTreeSet<_>>();
    DynamicRuntimeAuthorizationContext {
        allowed_runtime_profiles: BTreeSet::from([profile.runtime_profile.name.clone()]),
        allowed_actions,
        granted_ucan_abilities: input.granted_ucan_abilities.iter().cloned().collect(),
        session_capabilities: input.session_capabilities.iter().cloned().collect(),
        disabled_actions: input.disabled_actions.iter().cloned().collect(),
        max_input_bytes: profile.max_input_bytes,
    }
}

fn orchestration_receipt(
    profile: &SteelOrchestrationProfile,
    status: OrchestrationPlanStatus,
    issue_code: OrchestrationIssueCode,
    planner: OrchestrationPlannerKind,
    fallback_status: RustNativeFallbackStatus,
    plan_hash: Option<ArtifactHash>,
    steel_receipt_hash: Option<ArtifactHash>,
    rust_native_decision_class: Option<String>,
    authorization_receipts: Vec<DynamicRuntimeActionReceipt>,
    safe_summary: String,
) -> OrchestrationPlanReceipt {
    let material = OrchestrationReceiptHashMaterial {
        schema: STEEL_ORCHESTRATION_RECEIPT_SCHEMA,
        status,
        issue_code,
        seam: &profile.planning_seam,
        profile_name: &profile.name,
        planner,
        rollout_stage: profile.rollout_stage,
        fallback_status,
        script_id: &profile.script_id,
        script_hash: profile.script_hash,
        policy_hash: profile.policy_hash,
        plan_hash,
        steel_receipt_hash,
        rust_native_decision_class: rust_native_decision_class.as_deref(),
        authorization_receipt_hashes: authorization_receipts.iter().map(|receipt| receipt.receipt_hash).collect(),
        safe_summary: &safe_summary,
    };
    let bytes = serde_json::to_vec(&material).expect("Steel orchestration receipt material serializes");
    let receipt_hash = ArtifactHash::digest(&bytes);
    OrchestrationPlanReceipt {
        schema: STEEL_ORCHESTRATION_RECEIPT_SCHEMA.to_string(),
        status,
        issue_code,
        seam: profile.planning_seam.clone(),
        profile_name: profile.name.clone(),
        planner,
        rollout_stage: profile.rollout_stage,
        fallback_status,
        script_id: profile.script_id.clone(),
        script_hash: profile.script_hash,
        policy_hash: profile.policy_hash,
        plan_hash,
        steel_receipt_hash,
        rust_native_decision_class,
        authorization_receipts,
        safe_summary,
        redactions: vec![
            "raw_prompt".to_string(),
            "provider_payload".to_string(),
            "compact_ucan".to_string(),
            "raw_proof".to_string(),
            "credential".to_string(),
            "script_source".to_string(),
        ],
        receipt_hash,
    }
}

#[derive(Serialize)]
struct OrchestrationReceiptHashMaterial<'a> {
    schema: &'a str,
    status: OrchestrationPlanStatus,
    issue_code: OrchestrationIssueCode,
    seam: &'a str,
    profile_name: &'a str,
    planner: OrchestrationPlannerKind,
    rollout_stage: OrchestrationRolloutStage,
    fallback_status: RustNativeFallbackStatus,
    script_id: &'a str,
    script_hash: ArtifactHash,
    policy_hash: ArtifactHash,
    plan_hash: Option<ArtifactHash>,
    steel_receipt_hash: Option<ArtifactHash>,
    rust_native_decision_class: Option<&'a str>,
    authorization_receipt_hashes: Vec<ArtifactHash>,
    safe_summary: &'a str,
}

fn plan_hash(plan: &OrchestrationPlan) -> ArtifactHash {
    #[derive(Serialize)]
    struct PlanHashMaterial<'a> {
        schema: &'a str,
        seam: &'a str,
        planner: OrchestrationPlannerKind,
        decisions: &'a [OrchestrationDecision],
    }
    let material = PlanHashMaterial {
        schema: &plan.schema,
        seam: &plan.seam,
        planner: plan.planner,
        decisions: &plan.decisions,
    };
    let bytes = serde_json::to_vec(&material).expect("Steel orchestration plan material serializes");
    ArtifactHash::digest(&bytes)
}

fn route_slug(input: &str) -> String {
    input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> SteelOrchestrationProfile {
        SteelOrchestrationProfile::comparison_default(ArtifactHash::digest(b"script"), ArtifactHash::digest(b"policy"))
    }

    fn candidate(action_name: &str) -> OrchestrationCandidate {
        OrchestrationCandidate {
            decision_id: "first".to_string(),
            decision_class: "tool_candidate_ordering".to_string(),
            action_name: action_name.to_string(),
            target_resource: "turn:first".to_string(),
            required_ucan_ability: "clankers/steel/orchestrate.plan_turn".to_string(),
            required_session_capabilities: vec!["steel-orchestration".to_string(), "turn-planning".to_string()],
            input_hash: ArtifactHash::digest(b"candidate"),
            input_bytes: 32,
        }
    }

    fn input_with_payload(payload: String) -> TurnPlanningInput {
        TurnPlanningInput {
            turn_id: "turn-1".to_string(),
            prompt_hash: ArtifactHash::digest(b"prompt"),
            prompt_bytes: 128,
            candidate_actions: vec![candidate(DEFAULT_TURN_PLANNING_SEAM)],
            steel_source: "(host \"steel.host.plan_turn\")".to_string(),
            steel_plan_payload: payload,
            session_capabilities: vec!["steel-orchestration".to_string(), "turn-planning".to_string()],
            disabled_actions: Vec::new(),
            granted_ucan_abilities: vec!["clankers/steel/orchestrate.plan_turn".to_string()],
        }
    }

    fn valid_payload() -> String {
        format!(
            "{}|first|{}|turn:first|steel_selected_tool_candidate_ordering",
            STEEL_ORCHESTRATION_PLAN_SCHEMA, DEFAULT_TURN_PLANNING_SEAM
        )
    }

    #[test]
    fn steel_plan_is_default_but_effects_cross_dynamic_runtime_authorization() {
        let receipt = plan_turn_with_steel_or_fallback(&profile(), &input_with_payload(valid_payload()));
        assert_eq!(receipt.status, OrchestrationPlanStatus::Authorized);
        assert_eq!(receipt.planner, OrchestrationPlannerKind::SteelScheme);
        assert_eq!(receipt.fallback_status, RustNativeFallbackStatus::NotNeeded);
        assert_eq!(receipt.authorization_receipts[0].status, DynamicRuntimeActionStatus::Allowed);
        assert!(!receipt.authorization_receipts[0].writes_performed);
        assert!(receipt.plan_hash.is_some());
        assert!(receipt.steel_receipt_hash.is_some());
    }

    #[test]
    fn disabled_profile_uses_rust_native_planner_without_steel_claim() {
        let mut profile = profile();
        profile.enabled = false;
        let receipt = plan_turn_with_steel_or_fallback(&profile, &input_with_payload(valid_payload()));
        assert_eq!(receipt.status, OrchestrationPlanStatus::FallbackUsed);
        assert_eq!(receipt.issue_code, OrchestrationIssueCode::SteelDisabled);
        assert_eq!(receipt.planner, OrchestrationPlannerKind::RustNative);
        assert_eq!(receipt.fallback_status, RustNativeFallbackStatus::Used);
        assert!(receipt.steel_receipt_hash.is_none());
    }

    #[test]
    fn malformed_steel_plan_falls_back_only_when_policy_allows() {
        let receipt = plan_turn_with_steel_or_fallback(&profile(), &input_with_payload("not-a-plan".to_string()));
        assert_eq!(receipt.status, OrchestrationPlanStatus::FallbackUsed);
        assert_eq!(receipt.issue_code, OrchestrationIssueCode::MalformedPlan);
        assert_eq!(receipt.planner, OrchestrationPlannerKind::RustNative);
        assert_eq!(receipt.fallback_status, RustNativeFallbackStatus::Used);
    }

    #[test]
    fn fallback_disabled_blocks_after_script_failure() {
        let mut profile = profile();
        profile.fallback_mode = OrchestrationFallbackMode::Block;
        let mut input = input_with_payload(valid_payload());
        input.steel_source = "(write-file \"/tmp/x\" \"blocked\")".to_string();
        let receipt = plan_turn_with_steel_or_fallback(&profile, &input);
        assert_eq!(receipt.status, OrchestrationPlanStatus::Blocked);
        assert_eq!(receipt.issue_code, OrchestrationIssueCode::FallbackDisabled);
        assert_eq!(receipt.fallback_status, RustNativeFallbackStatus::Disabled);
        assert!(receipt.authorization_receipts.is_empty());
    }

    #[test]
    fn unknown_host_action_is_denied_before_any_effect() {
        let mut input = input_with_payload(format!(
            "{}|first|steel.host.provider|turn:first|bad-provider",
            STEEL_ORCHESTRATION_PLAN_SCHEMA
        ));
        input.candidate_actions = vec![candidate("steel.host.provider")];
        let receipt = plan_turn_with_steel_or_fallback(&profile(), &input);
        assert_eq!(receipt.status, OrchestrationPlanStatus::Denied);
        assert_eq!(receipt.issue_code, OrchestrationIssueCode::UnauthorizedAction);
        assert_eq!(receipt.authorization_receipts[0].reason, DynamicRuntimeActionReason::UnsupportedAction);
        assert!(!receipt.authorization_receipts[0].writes_performed);
    }

    #[test]
    fn disabled_action_and_missing_authority_fail_closed() {
        let mut input = input_with_payload(valid_payload());
        input.disabled_actions = vec![DEFAULT_TURN_PLANNING_SEAM.to_string()];
        let disabled = plan_turn_with_steel_or_fallback(&profile(), &input);
        assert_eq!(disabled.status, OrchestrationPlanStatus::Denied);
        assert_eq!(disabled.issue_code, OrchestrationIssueCode::UnauthorizedAction);

        let mut input = input_with_payload(valid_payload());
        input.granted_ucan_abilities.clear();
        let denied = plan_turn_with_steel_or_fallback(&profile(), &input);
        assert_eq!(denied.status, OrchestrationPlanStatus::Denied);
        assert_eq!(denied.authorization_receipts[0].status, DynamicRuntimeActionStatus::UcanDenied);
    }

    #[test]
    fn over_budget_and_ambient_attempts_do_not_gain_authority_by_hot_reload() {
        let mut limited_profile = profile();
        limited_profile.max_input_bytes = 1;
        let over_budget = plan_turn_with_steel_or_fallback(&limited_profile, &input_with_payload(valid_payload()));
        assert_eq!(over_budget.status, OrchestrationPlanStatus::Denied);
        assert_eq!(over_budget.authorization_receipts[0].reason, DynamicRuntimeActionReason::InputTooLarge);

        let mut input = input_with_payload(valid_payload());
        input.steel_source = "(http-get \"https://provider.invalid\")".to_string();
        let ambient = plan_turn_with_steel_or_fallback(&profile(), &input);
        assert_eq!(ambient.status, OrchestrationPlanStatus::FallbackUsed);
        assert_eq!(ambient.issue_code, OrchestrationIssueCode::ScriptEvaluationFailed);
    }

    #[test]
    fn comparison_receipts_are_deterministic() {
        let profile = profile();
        let input = input_with_payload(valid_payload());
        let first = plan_turn_with_steel_or_fallback(&profile, &input);
        let second = plan_turn_with_steel_or_fallback(&profile, &input);
        assert_eq!(first.receipt_hash, second.receipt_hash);
        assert_eq!(first.rust_native_decision_class, Some("tool_candidate_ordering".to_string()));
    }
}
