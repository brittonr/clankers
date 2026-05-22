//! Steel Scheme default orchestration planner seam.
//!
//! Steel is a trusted planner/requester at this seam. It returns a typed plan
//! through the Clankers-owned Steel runtime wrapper; Rust parses that plan,
//! authorizes every dynamic-runtime action envelope, and owns fallback receipts.
//! No caller in CLI/daemon/TUI/provider/tool-host code should construct Steel
//! interpreter internals directly.

use std::collections::BTreeSet;

use basalt::CallableContractDescriptor;
use basalt::CapabilityGrant as BasaltCapabilityGrant;
use basalt::ContractBackendKind;
use basalt::ContractEffectClass;
use basalt::ContractEnvelope;
use basalt::ContractPolicy as BasaltContractPolicy;
use basalt::EnforcementRequest as BasaltEnforcementRequest;
use basalt::Policy as BasaltPolicy;
use basalt::SteelEvaluationReceipt;
use basalt::SteelEvaluationRequest;
use basalt::enforce as basalt_enforce;
use basalt::steel_receipt_hash as basalt_steel_receipt_hash;
use basalt::steel_request_hash as basalt_steel_request_hash;
use basalt::validate_steel_evaluation_receipt;
use basalt::validate_steel_evaluation_request;
use chrono::DateTime;
use chrono::Utc;
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
const BASALT_STEEL_INPUT_SCHEMA: &str = "clankers.steel_orchestration.input.v1";
const BASALT_STEEL_CONTRACT_VERSION: &str = "clankers.steel_orchestration.contract.v1";
const STEEL_TURN_PLANNING_AUTHORITY_SCHEMA: &str = "clankers.steel_turn_planning.ucan_authority.v1";
const STEEL_TURN_PLANNING_AUTHORITY_CONTRACT: &str = "steel-turn-planning";
const STEEL_TURN_PLANNING_AUDIENCE: &str = "clankers:agent-turn-planning";

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
    #[serde(default)]
    pub ucan_authority_grants: Vec<SteelTurnPlanningAuthorityGrant>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelTurnPlanningAuthorityGrant {
    pub resource: String,
    pub ability: String,
    pub audience: String,
    #[serde(default)]
    pub proof_reference: Option<String>,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub revoked: bool,
    #[serde(default)]
    pub caveats: Vec<String>,
}

impl SteelTurnPlanningAuthorityGrant {
    #[must_use]
    pub fn reviewed(resource: impl Into<String>, ability: impl Into<String>) -> Self {
        Self {
            resource: resource.into(),
            ability: ability.into(),
            audience: STEEL_TURN_PLANNING_AUDIENCE.to_string(),
            proof_reference: Some("settings-grant".to_string()),
            expires_at: None,
            revoked: false,
            caveats: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelTurnPlanningAuthorityStatus {
    Allowed,
    Denied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SteelTurnPlanningAuthorityReason {
    Allowed,
    MissingGrant,
    ExpiredGrant,
    RevokedGrant,
    WrongAudience,
    WrongResource,
    WrongAbility,
    UnknownCaveat,
    OverbroadGrant,
    BasaltDenied,
    BasaltError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelTurnPlanningAuthorityReceipt {
    pub schema: String,
    pub status: SteelTurnPlanningAuthorityStatus,
    pub reason: SteelTurnPlanningAuthorityReason,
    pub seam: String,
    pub resource: String,
    pub ability: String,
    pub audience: String,
    pub proof_reference: Option<String>,
    pub grant_count: usize,
    pub caveat_classes: Vec<String>,
    pub basalt_reason: Option<String>,
    pub receipt_hash: ArtifactHash,
}

impl SteelTurnPlanningAuthorityReceipt {
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        matches!(self.status, SteelTurnPlanningAuthorityStatus::Allowed)
    }
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
    BasaltRequestInvalid,
    BasaltReceiptInvalid,
    UcanAuthorityDenied,
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
    pub basalt_request_schema: Option<String>,
    pub basalt_request_hash: Option<String>,
    pub basalt_receipt_schema: Option<String>,
    pub basalt_receipt_hash: Option<String>,
    pub basalt_receipt_valid: bool,
    pub basalt_receipt_reason: Option<String>,
    pub ucan_authority_receipt: Option<SteelTurnPlanningAuthorityReceipt>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasaltSteelContractEvidence {
    pub request_schema: String,
    pub request_hash: String,
    pub receipt_schema: String,
    pub receipt_hash: Option<String>,
    pub receipt_valid: bool,
    pub receipt_reason: String,
}

impl BasaltSteelContractEvidence {
    fn request_only(request: &SteelEvaluationRequest, reason: impl Into<String>) -> Self {
        Self {
            request_schema: request.envelope.contract_version.clone(),
            request_hash: basalt_steel_request_hash(request),
            receipt_schema: request.envelope.receipt_schema_version.clone(),
            receipt_hash: None,
            receipt_valid: false,
            receipt_reason: reason.into(),
        }
    }
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
            None,
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
            None,
        );
    }

    let basalt_request = basalt_steel_evaluation_request(profile, input);
    let basalt_request_receipt = validate_steel_evaluation_request(&basalt_request);
    if !basalt_request_receipt.accepted {
        let evidence =
            BasaltSteelContractEvidence::request_only(&basalt_request, basalt_request_receipt.reason.clone());
        return fallback_or_block(
            profile,
            input,
            &fallback_plan,
            None,
            OrchestrationIssueCode::BasaltRequestInvalid,
            "Basalt Steel evaluation request failed validation before contract-backed planning",
            fallback_decision_class,
            Some(evidence),
        );
    }

    let authority_receipt = authorize_steel_turn_planning_invocation(profile, input);
    if !authority_receipt.is_allowed() {
        return orchestration_receipt(
            profile,
            OrchestrationPlanStatus::Blocked,
            OrchestrationIssueCode::UcanAuthorityDenied,
            OrchestrationPlannerKind::SteelScheme,
            RustNativeFallbackStatus::Disabled,
            None,
            None,
            fallback_decision_class,
            Vec::new(),
            "UCAN authority denied Steel turn-planning before Steel or provider execution".to_string(),
            Some(BasaltSteelContractEvidence::request_only(
                &basalt_request,
                "Steel turn-planning UCAN authority denied before evaluation",
            )),
            Some(authority_receipt),
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
            Some(BasaltSteelContractEvidence::request_only(
                &basalt_request,
                "Steel runtime did not return a Basalt receipt",
            )),
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
            Some(BasaltSteelContractEvidence::request_only(
                &basalt_request,
                "Steel runtime returned no typed plan payload",
            )),
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
            Some(BasaltSteelContractEvidence::request_only(
                &basalt_request,
                "Steel planner output did not match the typed plan schema",
            )),
        );
    };
    let basalt_evidence =
        basalt_contract_evidence(profile, input, &basalt_request, true, "Steel plan parsed as typed data");
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
        Some(basalt_evidence),
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
    basalt_evidence: Option<BasaltSteelContractEvidence>,
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
            basalt_evidence,
            None,
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
    let authority_receipt = (planner == OrchestrationPlannerKind::SteelScheme)
        .then(|| authorize_steel_turn_planning_invocation(profile, input));
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
        basalt_evidence,
        authority_receipt,
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
    basalt_evidence: Option<BasaltSteelContractEvidence>,
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
            basalt_evidence,
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
            basalt_evidence,
            None,
        ),
    }
}

fn basalt_steel_evaluation_request(
    profile: &SteelOrchestrationProfile,
    input: &TurnPlanningInput,
) -> SteelEvaluationRequest {
    let required_capability = basalt_required_capability(profile, input);
    SteelEvaluationRequest {
        envelope: ContractEnvelope::new(
            ContractBackendKind::Steel.as_str(),
            DEFAULT_TURN_PLANNING_SEAM,
            BASALT_STEEL_CONTRACT_VERSION,
            profile.script_hash.prefixed(),
            BASALT_STEEL_INPUT_SCHEMA,
            STEEL_ORCHESTRATION_PLAN_SCHEMA,
            "basalt.steel.receipt.v1",
        ),
        input: serde_json::json!({
            "seam": DEFAULT_TURN_PLANNING_SEAM,
            "turn_id": route_slug(&input.turn_id),
            "prompt_hash": input.prompt_hash.prefixed(),
            "prompt_bytes": input.prompt_bytes,
            "candidate_count": input.candidate_actions.len(),
            "disabled_action_count": input.disabled_actions.len(),
            "script_hash": profile.script_hash.prefixed(),
            "policy_hash": profile.policy_hash.prefixed(),
        }),
        max_input_bytes: profile.max_input_bytes as usize,
        callable: Some(CallableContractDescriptor {
            callable_id: DEFAULT_TURN_PLANNING_SEAM.to_string(),
            arity: 1,
            argument_contracts: vec![BASALT_STEEL_INPUT_SCHEMA.to_string()],
            return_contract: STEEL_ORCHESTRATION_PLAN_SCHEMA.to_string(),
            required_capabilities: vec![required_capability],
            effect_class: ContractEffectClass::HostEffect,
            content_hash: profile.script_hash.prefixed(),
            redaction_class: "metadata_only".to_string(),
        }),
        requested_host_capability: None,
    }
}

fn basalt_contract_evidence(
    profile: &SteelOrchestrationProfile,
    input: &TurnPlanningInput,
    request: &SteelEvaluationRequest,
    allowed: bool,
    reason: &str,
) -> BasaltSteelContractEvidence {
    let required_capability = basalt_required_capability(profile, input);
    let checked_capabilities =
        if input.granted_ucan_abilities.iter().any(|ability| ability == &profile.required_ucan_ability) {
            vec![required_capability.clone()]
        } else {
            Vec::new()
        };
    let mut receipt = SteelEvaluationReceipt {
        backend: ContractBackendKind::Steel.as_str().to_string(),
        contract_id: DEFAULT_TURN_PLANNING_SEAM.to_string(),
        normalized_source_hash: profile.script_hash.prefixed(),
        request_hash: basalt_steel_request_hash(request),
        allowed,
        reason: reason.to_string(),
        caveat_evidence: std::collections::BTreeMap::from([
            ("seam".to_string(), DEFAULT_TURN_PLANNING_SEAM.to_string()),
            ("profile".to_string(), profile.name.clone()),
            ("rollout_stage".to_string(), format!("{:?}", profile.rollout_stage)),
        ]),
        effect_class: ContractEffectClass::HostEffect,
        required_capabilities: vec![required_capability],
        checked_capabilities,
        receipt_hash: None,
    };
    let receipt_hash = basalt_steel_receipt_hash(&receipt);
    receipt.receipt_hash = Some(receipt_hash.clone());
    let validation = validate_steel_evaluation_receipt(&receipt);
    BasaltSteelContractEvidence {
        request_schema: request.envelope.contract_version.clone(),
        request_hash: receipt.request_hash,
        receipt_schema: request.envelope.receipt_schema_version.clone(),
        receipt_hash: Some(receipt_hash),
        receipt_valid: validation.accepted,
        receipt_reason: validation.reason,
    }
}

fn basalt_required_capability(profile: &SteelOrchestrationProfile, input: &TurnPlanningInput) -> BasaltCapabilityGrant {
    let resource = input
        .candidate_actions
        .first()
        .map(|candidate| candidate.target_resource.as_str())
        .unwrap_or("session:unknown");
    BasaltCapabilityGrant::new(resource, profile.required_ucan_ability.as_str())
}

fn authorize_steel_turn_planning_invocation(
    profile: &SteelOrchestrationProfile,
    input: &TurnPlanningInput,
) -> SteelTurnPlanningAuthorityReceipt {
    let requested_resource = input
        .candidate_actions
        .first()
        .map(|candidate| candidate.target_resource.as_str())
        .unwrap_or("session:unknown");
    let requested_ability = profile.required_ucan_ability.as_str();
    let grants = effective_authority_grants(profile, input, requested_resource);
    let denied = |reason, grant: Option<&SteelTurnPlanningAuthorityGrant>, basalt_reason: Option<String>| {
        authority_receipt(
            SteelTurnPlanningAuthorityStatus::Denied,
            reason,
            requested_resource,
            requested_ability,
            grant,
            grants.len(),
            basalt_reason,
        )
    };
    let Some(grant) = grants.first() else {
        return denied(SteelTurnPlanningAuthorityReason::MissingGrant, None, None);
    };
    if grant.revoked {
        return denied(SteelTurnPlanningAuthorityReason::RevokedGrant, Some(grant), None);
    }
    if grant.expires_at.is_some_and(|expires_at| expires_at <= Utc::now()) {
        return denied(SteelTurnPlanningAuthorityReason::ExpiredGrant, Some(grant), None);
    }
    if grant.audience != STEEL_TURN_PLANNING_AUDIENCE {
        return denied(SteelTurnPlanningAuthorityReason::WrongAudience, Some(grant), None);
    }
    if grant.ability != requested_ability {
        return denied(SteelTurnPlanningAuthorityReason::WrongAbility, Some(grant), None);
    }
    if grant.resource != requested_resource {
        let reason = if requested_resource.starts_with(grant.resource.as_str()) {
            SteelTurnPlanningAuthorityReason::OverbroadGrant
        } else {
            SteelTurnPlanningAuthorityReason::WrongResource
        };
        return denied(reason, Some(grant), None);
    }
    if grant.caveats.iter().any(|caveat| caveat != "metadata_only") {
        return denied(SteelTurnPlanningAuthorityReason::UnknownCaveat, Some(grant), None);
    }
    let basalt_resource = authority_basalt_resource(requested_resource);
    let basalt_ability = authority_basalt_ability(requested_ability);
    match basalt_enforce(
        &authority_policy(&basalt_resource, &basalt_ability),
        &authority_request(&basalt_resource, &basalt_ability),
    ) {
        Ok(receipt) if receipt.is_allowed() => authority_receipt(
            SteelTurnPlanningAuthorityStatus::Allowed,
            SteelTurnPlanningAuthorityReason::Allowed,
            requested_resource,
            requested_ability,
            Some(grant),
            grants.len(),
            Some(receipt.reason().to_string()),
        ),
        Ok(receipt) => {
            denied(SteelTurnPlanningAuthorityReason::BasaltDenied, Some(grant), Some(receipt.reason().to_string()))
        }
        Err(error) => denied(SteelTurnPlanningAuthorityReason::BasaltError, Some(grant), Some(error.to_string())),
    }
}

fn effective_authority_grants(
    profile: &SteelOrchestrationProfile,
    input: &TurnPlanningInput,
    requested_resource: &str,
) -> Vec<SteelTurnPlanningAuthorityGrant> {
    if !input.ucan_authority_grants.is_empty() {
        return input.ucan_authority_grants.clone();
    }
    if input.granted_ucan_abilities.iter().any(|ability| ability == &profile.required_ucan_ability) {
        return vec![SteelTurnPlanningAuthorityGrant::reviewed(
            requested_resource,
            profile.required_ucan_ability.clone(),
        )];
    }
    Vec::new()
}

fn authority_policy(resource: &str, ability: &str) -> BasaltPolicy {
    BasaltPolicy {
        schema_version: basalt::SUPPORTED_SCHEMA_VERSION.to_string(),
        contracts: std::collections::BTreeMap::from([(
            STEEL_TURN_PLANNING_AUTHORITY_CONTRACT.to_string(),
            BasaltContractPolicy {
                id: STEEL_TURN_PLANNING_AUTHORITY_CONTRACT.to_string(),
                description: "Authorize reviewed Steel turn-planning invocation".to_string(),
                resource_prefixes: vec![resource.to_string()],
                abilities: vec![ability.to_string()],
            },
        )]),
        backends: std::collections::BTreeMap::new(),
    }
}

fn authority_request(basalt_resource: &str, basalt_ability: &str) -> BasaltEnforcementRequest {
    BasaltEnforcementRequest::new(
        STEEL_TURN_PLANNING_AUTHORITY_CONTRACT,
        basalt_resource.to_string(),
        basalt_ability.to_string(),
    )
    .with_capability(BasaltCapabilityGrant::new(basalt_resource.to_string(), basalt_ability.to_string()))
}

fn authority_basalt_resource(resource: &str) -> String {
    format!("urn:clankers:steel-turn-planning:{}", route_slug(resource))
}

fn authority_basalt_ability(ability: &str) -> String {
    route_slug(ability)
        .replace('.', "_")
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

fn authority_receipt(
    status: SteelTurnPlanningAuthorityStatus,
    reason: SteelTurnPlanningAuthorityReason,
    requested_resource: &str,
    requested_ability: &str,
    grant: Option<&SteelTurnPlanningAuthorityGrant>,
    grant_count: usize,
    basalt_reason: Option<String>,
) -> SteelTurnPlanningAuthorityReceipt {
    let mut receipt = SteelTurnPlanningAuthorityReceipt {
        schema: STEEL_TURN_PLANNING_AUTHORITY_SCHEMA.to_string(),
        status,
        reason,
        seam: DEFAULT_TURN_PLANNING_SEAM.to_string(),
        resource: route_slug(requested_resource),
        ability: requested_ability.to_string(),
        audience: grant
            .map(|grant| route_slug(&grant.audience))
            .unwrap_or_else(|| route_slug(STEEL_TURN_PLANNING_AUDIENCE)),
        proof_reference: grant.and_then(|grant| grant.proof_reference.as_deref()).map(route_slug),
        grant_count,
        caveat_classes: grant
            .map(|grant| grant.caveats.iter().map(|caveat| route_slug(caveat)).collect())
            .unwrap_or_default(),
        basalt_reason: basalt_reason.map(|reason| route_slug(&reason)),
        receipt_hash: ArtifactHash::digest(b"pending"),
    };
    receipt.receipt_hash = authority_receipt_hash(&receipt);
    receipt
}

fn authority_receipt_hash(receipt: &SteelTurnPlanningAuthorityReceipt) -> ArtifactHash {
    let mut material = receipt.clone();
    material.receipt_hash = ArtifactHash::digest(b"omitted");
    let bytes = serde_json::to_vec(&material).expect("Steel authority receipt serializes");
    ArtifactHash::digest(&bytes)
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
    let mut granted_ucan_abilities = input.granted_ucan_abilities.iter().cloned().collect::<BTreeSet<_>>();
    granted_ucan_abilities.extend(input.ucan_authority_grants.iter().map(|grant| grant.ability.clone()));
    DynamicRuntimeAuthorizationContext {
        allowed_runtime_profiles: BTreeSet::from([profile.runtime_profile.name.clone()]),
        allowed_actions,
        granted_ucan_abilities,
        session_capabilities: input.session_capabilities.iter().cloned().collect(),
        disabled_actions: input.disabled_actions.iter().cloned().collect(),
        max_input_bytes: profile.max_input_bytes,
    }
}

#[allow(clippy::too_many_arguments)]
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
    basalt_evidence: Option<BasaltSteelContractEvidence>,
    authority_receipt: Option<SteelTurnPlanningAuthorityReceipt>,
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
        basalt_request_hash: basalt_evidence.as_ref().map(|evidence| evidence.request_hash.as_str()),
        basalt_receipt_hash: basalt_evidence.as_ref().and_then(|evidence| evidence.receipt_hash.as_deref()),
        basalt_receipt_valid: basalt_evidence.as_ref().is_some_and(|evidence| evidence.receipt_valid),
        ucan_authority_receipt_hash: authority_receipt.as_ref().map(|receipt| receipt.receipt_hash),
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
        basalt_request_schema: basalt_evidence.as_ref().map(|evidence| evidence.request_schema.clone()),
        basalt_request_hash: basalt_evidence.as_ref().map(|evidence| evidence.request_hash.clone()),
        basalt_receipt_schema: basalt_evidence.as_ref().map(|evidence| evidence.receipt_schema.clone()),
        basalt_receipt_hash: basalt_evidence.as_ref().and_then(|evidence| evidence.receipt_hash.clone()),
        basalt_receipt_valid: basalt_evidence.as_ref().is_some_and(|evidence| evidence.receipt_valid),
        basalt_receipt_reason: basalt_evidence.as_ref().map(|evidence| evidence.receipt_reason.clone()),
        ucan_authority_receipt: authority_receipt,
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
    basalt_request_hash: Option<&'a str>,
    basalt_receipt_hash: Option<&'a str>,
    basalt_receipt_valid: bool,
    ucan_authority_receipt_hash: Option<ArtifactHash>,
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
            ucan_authority_grants: Vec::new(),
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
        assert_eq!(receipt.basalt_request_schema.as_deref(), Some(BASALT_STEEL_CONTRACT_VERSION));
        assert!(receipt.basalt_request_hash.as_deref().is_some_and(|hash| hash.starts_with("blake3:")));
        assert_eq!(receipt.basalt_receipt_schema.as_deref(), Some("basalt.steel.receipt.v1"));
        assert!(receipt.basalt_receipt_hash.as_deref().is_some_and(|hash| hash.starts_with("blake3:")));
        assert!(receipt.basalt_receipt_valid);
        assert_eq!(receipt.basalt_receipt_reason.as_deref(), Some("accepted"));
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
        assert_eq!(denied.status, OrchestrationPlanStatus::Blocked);
        assert_eq!(denied.issue_code, OrchestrationIssueCode::UcanAuthorityDenied);
        assert!(denied.authorization_receipts.is_empty());
        let authority = denied.ucan_authority_receipt.as_ref().expect("authority receipt");
        assert_eq!(authority.status, SteelTurnPlanningAuthorityStatus::Denied);
        assert_eq!(authority.reason, SteelTurnPlanningAuthorityReason::MissingGrant);
        assert_eq!(authority.proof_reference, None);
    }

    fn authority_grant() -> SteelTurnPlanningAuthorityGrant {
        SteelTurnPlanningAuthorityGrant::reviewed("turn:first", "clankers/steel/orchestrate.plan_turn")
    }

    #[test]
    fn explicit_ucan_authority_grant_allows_steel_planning() {
        let mut input = input_with_payload(valid_payload());
        input.granted_ucan_abilities.clear();
        input.ucan_authority_grants = vec![authority_grant()];
        let receipt = plan_turn_with_steel_or_fallback(&profile(), &input);
        assert_eq!(receipt.status, OrchestrationPlanStatus::Authorized);
        assert_eq!(receipt.planner, OrchestrationPlannerKind::SteelScheme);
        let authority = receipt.ucan_authority_receipt.as_ref().expect("authority receipt");
        assert_eq!(authority.status, SteelTurnPlanningAuthorityStatus::Allowed);
        assert_eq!(authority.reason, SteelTurnPlanningAuthorityReason::Allowed);
        assert!(authority.receipt_hash.prefixed().starts_with("b3:"));
        assert_eq!(authority.proof_reference.as_deref(), Some("settings-grant"));
    }

    #[test]
    fn expired_revoked_and_wrong_scope_authority_grants_fail_closed_before_dynamic_action() {
        let mut expired = input_with_payload(valid_payload());
        expired.granted_ucan_abilities.clear();
        let mut expired_grant = authority_grant();
        expired_grant.expires_at = Some(Utc::now() - chrono::Duration::seconds(1));
        expired.ucan_authority_grants = vec![expired_grant];
        let expired_receipt = plan_turn_with_steel_or_fallback(&profile(), &expired);
        assert_eq!(expired_receipt.status, OrchestrationPlanStatus::Blocked);
        assert_eq!(expired_receipt.issue_code, OrchestrationIssueCode::UcanAuthorityDenied);
        assert!(expired_receipt.authorization_receipts.is_empty());
        assert_eq!(
            expired_receipt.ucan_authority_receipt.as_ref().map(|receipt| receipt.reason),
            Some(SteelTurnPlanningAuthorityReason::ExpiredGrant)
        );

        let mut revoked = input_with_payload(valid_payload());
        revoked.granted_ucan_abilities.clear();
        let mut revoked_grant = authority_grant();
        revoked_grant.revoked = true;
        revoked.ucan_authority_grants = vec![revoked_grant];
        let revoked_receipt = plan_turn_with_steel_or_fallback(&profile(), &revoked);
        assert_eq!(revoked_receipt.status, OrchestrationPlanStatus::Blocked);
        assert_eq!(
            revoked_receipt.ucan_authority_receipt.as_ref().map(|receipt| receipt.reason),
            Some(SteelTurnPlanningAuthorityReason::RevokedGrant)
        );

        let mut wrong_scope = input_with_payload(valid_payload());
        wrong_scope.granted_ucan_abilities.clear();
        let mut wrong_scope_grant = authority_grant();
        wrong_scope_grant.resource = "turn:other".to_string();
        wrong_scope.ucan_authority_grants = vec![wrong_scope_grant];
        let wrong_scope_receipt = plan_turn_with_steel_or_fallback(&profile(), &wrong_scope);
        assert_eq!(wrong_scope_receipt.status, OrchestrationPlanStatus::Blocked);
        assert_eq!(
            wrong_scope_receipt.ucan_authority_receipt.as_ref().map(|receipt| receipt.reason),
            Some(SteelTurnPlanningAuthorityReason::WrongResource)
        );
        assert!(wrong_scope_receipt.steel_receipt_hash.is_none());
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
