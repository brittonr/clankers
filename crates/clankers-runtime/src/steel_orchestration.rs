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
pub use clanker_message::OrchestrationCandidate;
pub use clanker_message::OrchestrationDecision;
pub use clanker_message::OrchestrationFallbackMode;
pub use clanker_message::OrchestrationIssueCode;
pub use clanker_message::OrchestrationPlan;
pub use clanker_message::OrchestrationPlanStatus;
pub use clanker_message::OrchestrationPlannerKind;
pub use clanker_message::OrchestrationRolloutStage;
pub use clanker_message::RustNativeFallbackStatus;
pub use clanker_message::SteelTurnExecutionStatus;
pub use clanker_message::SteelTurnPlanningAuthorityReason;
pub use clanker_message::SteelTurnPlanningAuthorityStatus;
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
use crate::steel_runtime::SteelHostCallOutcome;
#[cfg(test)]
use crate::steel_runtime::SteelHostCallReceipt;
use crate::steel_runtime::SteelHostFunctionRegistration;
use crate::steel_runtime::SteelRuntimeProfile;
use crate::steel_runtime::SteelRuntimeReasonCode;
use crate::steel_runtime::SteelRuntimeReceipt;
use crate::steel_runtime::SteelRuntimeRequest;
use crate::steel_runtime::SteelRuntimeStatusCode;
use crate::steel_runtime::evaluate_steel_request;

pub const STEEL_ORCHESTRATION_PLAN_SCHEMA: &str = "clankers.steel_orchestration.plan.v1";
pub const STEEL_ORCHESTRATION_RECEIPT_SCHEMA: &str = "clankers.steel_orchestration.receipt.v1";
pub const STEEL_TURN_EXECUTION_HOST_CALL_SCHEMA: &str = "clankers.steel_turn_execution.host_call.v1";
pub const STEEL_TURN_EXECUTION_RECEIPT_SCHEMA: &str = "clankers.steel_turn_execution.receipt.v1";
pub const DEFAULT_TURN_PLANNING_SEAM: &str = "steel.host.plan_turn";
pub const DEFAULT_TURN_EXECUTION_SEAM: &str = "steel.host.execute_turn";
pub const DEFAULT_TURN_EXECUTION_SOURCE: &str = "(host \"steel.host.execute_turn\")";
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
    pub execution_required_session_capabilities: Vec<String>,
    pub execution_required_ucan_ability: String,
    pub receipt_prefix: String,
    pub max_input_bytes: u64,
}

impl SteelOrchestrationProfile {
    #[must_use]
    pub fn comparison_default(script_hash: ArtifactHash, policy_hash: ArtifactHash) -> Self {
        let mut allowed_host_actions = BTreeSet::new();
        allowed_host_actions.insert(DEFAULT_TURN_PLANNING_SEAM.to_string());
        allowed_host_actions.insert(DEFAULT_TURN_EXECUTION_SEAM.to_string());
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
            execution_required_session_capabilities: vec![
                "steel-orchestration".to_string(),
                "turn-execution".to_string(),
            ],
            execution_required_ucan_ability: "clankers/steel/orchestrate.execute_turn".to_string(),
            receipt_prefix: DEFAULT_RECEIPT_PREFIX.to_string(),
            max_input_bytes: 8192,
        }
    }
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
pub struct SteelTurnPlanHostCallPayload {
    pub schema: String,
    pub decision_id: String,
    pub action_name: String,
    pub target_resource: String,
    pub decision_class: String,
}

impl SteelTurnPlanHostCallPayload {
    #[must_use]
    pub fn new(
        decision_id: impl Into<String>,
        action_name: impl Into<String>,
        target_resource: impl Into<String>,
        decision_class: impl Into<String>,
    ) -> Self {
        Self {
            schema: STEEL_ORCHESTRATION_PLAN_SCHEMA.to_string(),
            decision_id: decision_id.into(),
            action_name: action_name.into(),
            target_resource: target_resource.into(),
            decision_class: decision_class.into(),
        }
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("Steel turn planning host-call payload serializes")
    }
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelTurnExecutionInput {
    pub turn_id: String,
    pub target_resource: String,
    pub plan_receipt_hash: ArtifactHash,
    pub plan_hash: Option<ArtifactHash>,
    pub prompt_hash: ArtifactHash,
    pub host_runner: String,
    pub session_capabilities: Vec<String>,
    pub disabled_actions: Vec<String>,
    pub granted_ucan_abilities: Vec<String>,
    #[serde(default)]
    pub ucan_authority_grants: Vec<SteelTurnPlanningAuthorityGrant>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelTurnExecutionHostCallPayload {
    pub schema: String,
    pub seam: String,
    pub plan_receipt_hash: ArtifactHash,
    pub host_runner: String,
    pub input_hash: ArtifactHash,
}

impl SteelTurnExecutionHostCallPayload {
    #[must_use]
    pub fn new(input: &SteelTurnExecutionInput, input_hash: ArtifactHash) -> Self {
        Self {
            schema: STEEL_TURN_EXECUTION_HOST_CALL_SCHEMA.to_string(),
            seam: DEFAULT_TURN_EXECUTION_SEAM.to_string(),
            plan_receipt_hash: input.plan_receipt_hash,
            host_runner: input.host_runner.clone(),
            input_hash,
        }
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("Steel execute-turn host-call payload serializes")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelTurnExecutionHostCallReceipt {
    pub schema: String,
    pub seam: String,
    pub source_hash: ArtifactHash,
    pub runtime_receipt_hash: ArtifactHash,
    pub runtime_status: SteelRuntimeStatusCode,
    pub runtime_reason: SteelRuntimeReasonCode,
    pub host_call_outcome: Option<SteelHostCallOutcome>,
    pub payload_hash: Option<ArtifactHash>,
    pub payload_valid: bool,
    pub safe_summary: String,
    pub receipt_hash: ArtifactHash,
}

impl SteelTurnExecutionHostCallReceipt {
    #[must_use]
    pub fn is_allowed(&self) -> bool {
        self.runtime_status == SteelRuntimeStatusCode::Succeeded
            && self.runtime_reason == SteelRuntimeReasonCode::Ok
            && self.host_call_outcome == Some(SteelHostCallOutcome::Approved)
            && self.payload_valid
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelTurnExecutionReceipt {
    pub schema: String,
    pub status: SteelTurnExecutionStatus,
    pub seam: String,
    pub executor: OrchestrationPlannerKind,
    pub host_runner: String,
    pub plan_receipt_hash: ArtifactHash,
    pub plan_hash: Option<ArtifactHash>,
    pub input_hash: ArtifactHash,
    pub input_bytes: u64,
    pub host_call_receipt: SteelTurnExecutionHostCallReceipt,
    pub authorization_receipt: DynamicRuntimeActionReceipt,
    pub redactions: Vec<String>,
    pub receipt_hash: ArtifactHash,
}

impl SteelTurnExecutionReceipt {
    #[must_use]
    pub const fn is_allowed(&self) -> bool {
        matches!(self.status, SteelTurnExecutionStatus::Authorized)
    }
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
        return steel_authority_denied_receipt(profile, &basalt_request, fallback_decision_class, authority_receipt);
    }

    let evaluation = evaluate_steel_plan(profile, input, &basalt_request);
    let (steel_plan, steel_receipt_hash) = match evaluation {
        SteelPlanEvaluation::Ready(steel_plan, steel_receipt_hash) => (steel_plan, steel_receipt_hash),
        SteelPlanEvaluation::Failed(error) => {
            return fallback_or_block(
                profile,
                input,
                &fallback_plan,
                Some(error.steel_receipt_hash),
                error.issue_code,
                error.safe_summary,
                fallback_decision_class,
                Some(error.evidence),
            );
        }
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
pub fn authorize_steel_turn_execution(
    profile: &SteelOrchestrationProfile,
    input: &SteelTurnExecutionInput,
) -> SteelTurnExecutionReceipt {
    let input_bytes = stable_execution_input_bytes(input);
    let input_hash = ArtifactHash::digest(&input_bytes);
    let envelope = DynamicRuntimeActionEnvelope {
        schema: DYNAMIC_RUNTIME_ACTION_SCHEMA.to_string(),
        action_id: format!("steel-execute:{}", route_slug(&input.turn_id)),
        runtime: DynamicRuntimeKind::SteelScheme,
        runtime_profile: profile.runtime_profile.name.clone(),
        action_kind: DynamicRuntimeActionKind::HostFunction,
        action_name: DEFAULT_TURN_EXECUTION_SEAM.to_string(),
        target_resource: input.target_resource.clone(),
        receipt_destination: format!(
            "{}/steel-execute-{}.json",
            profile.receipt_prefix.trim_end_matches('/'),
            route_slug(&input.turn_id)
        ),
        required_ucan_ability: profile.execution_required_ucan_ability.clone(),
        required_session_capabilities: profile.execution_required_session_capabilities.clone(),
        input_hash,
        input_bytes: input_bytes.len() as u64,
        redaction: DynamicRuntimeRedactionClass::MetadataOnly,
    };
    let context = dynamic_authorization_context(
        profile,
        &input.session_capabilities,
        &input.granted_ucan_abilities,
        &input.ucan_authority_grants,
        &input.disabled_actions,
    );
    let host_call_receipt = evaluate_steel_execution_host_call(profile, input, input_hash);
    let authorization_receipt = authorize_dynamic_runtime_action(&envelope, &context);
    let status =
        if host_call_receipt.is_allowed() && authorization_receipt.status == DynamicRuntimeActionStatus::Allowed {
            SteelTurnExecutionStatus::Authorized
        } else {
            SteelTurnExecutionStatus::Denied
        };
    steel_turn_execution_receipt(
        profile,
        input,
        input_hash,
        input_bytes.len() as u64,
        host_call_receipt,
        authorization_receipt,
        status,
    )
}

fn evaluate_steel_execution_host_call(
    profile: &SteelOrchestrationProfile,
    input: &SteelTurnExecutionInput,
    input_hash: ArtifactHash,
) -> SteelTurnExecutionHostCallReceipt {
    let payload = steel_execution_host_call_payload(input, input_hash);
    let host_functions = if profile.allowed_host_actions.contains(DEFAULT_TURN_EXECUTION_SEAM) {
        vec![SteelHostFunctionRegistration {
            name: DEFAULT_TURN_EXECUTION_SEAM.to_string(),
            required_capability: execution_host_call_capability(profile),
            output: payload,
        }]
    } else {
        Vec::new()
    };
    let request = SteelRuntimeRequest {
        profile: profile.runtime_profile.clone(),
        source: DEFAULT_TURN_EXECUTION_SOURCE.to_string(),
        session_capabilities: input.session_capabilities.clone(),
        disabled_tools: input.disabled_actions.clone(),
        host_functions,
        receipt_destination: format!("{}/steel-execute-runtime.json", profile.receipt_prefix.trim_end_matches('/')),
    };
    let runtime_receipt = evaluate_steel_request(&request);
    steel_execution_host_call_receipt(&request, input, &runtime_receipt)
}

fn execution_host_call_capability(profile: &SteelOrchestrationProfile) -> String {
    profile
        .execution_required_session_capabilities
        .iter()
        .find(|capability| capability.as_str() != "steel-orchestration")
        .cloned()
        .or_else(|| profile.execution_required_session_capabilities.first().cloned())
        .unwrap_or_else(|| "turn-execution".to_string())
}

fn steel_execution_host_call_payload(input: &SteelTurnExecutionInput, input_hash: ArtifactHash) -> String {
    SteelTurnExecutionHostCallPayload::new(input, input_hash).to_json()
}

fn steel_execution_host_call_receipt(
    request: &SteelRuntimeRequest,
    input: &SteelTurnExecutionInput,
    runtime_receipt: &SteelRuntimeReceipt,
) -> SteelTurnExecutionHostCallReceipt {
    let output = runtime_receipt.output.as_deref();
    let payload_valid = output.is_some_and(|payload| steel_execution_host_call_payload_is_valid(input, payload));
    let host_call_outcome = runtime_receipt.host_calls.first().map(|call| call.outcome.clone());
    let mut receipt = SteelTurnExecutionHostCallReceipt {
        schema: STEEL_TURN_EXECUTION_HOST_CALL_SCHEMA.to_string(),
        seam: DEFAULT_TURN_EXECUTION_SEAM.to_string(),
        source_hash: ArtifactHash::digest(request.source.as_bytes()),
        runtime_receipt_hash: runtime_receipt.receipt_hash(),
        runtime_status: runtime_receipt.status.clone(),
        runtime_reason: runtime_receipt.reason_code.clone(),
        host_call_outcome,
        payload_hash: output.map(|payload| ArtifactHash::digest(payload.as_bytes())),
        payload_valid,
        safe_summary: steel_execution_host_call_summary(runtime_receipt, payload_valid),
        receipt_hash: ArtifactHash::digest(b"pending"),
    };
    receipt.receipt_hash = steel_execution_host_call_receipt_hash(&receipt);
    receipt
}

fn steel_execution_host_call_payload_is_valid(input: &SteelTurnExecutionInput, payload: &str) -> bool {
    let Ok(parsed) = serde_json::from_str::<SteelTurnExecutionHostCallPayload>(payload) else {
        return false;
    };
    parsed.schema == STEEL_TURN_EXECUTION_HOST_CALL_SCHEMA
        && parsed.seam == DEFAULT_TURN_EXECUTION_SEAM
        && parsed.plan_receipt_hash == input.plan_receipt_hash
        && parsed.host_runner == input.host_runner
        && parsed.input_hash == ArtifactHash::digest(&stable_execution_input_bytes(input))
}

fn steel_execution_host_call_summary(runtime_receipt: &SteelRuntimeReceipt, payload_valid: bool) -> String {
    if runtime_receipt.status != SteelRuntimeStatusCode::Succeeded {
        return "Steel execute_turn host call denied before Rust host runner".to_string();
    }
    if !payload_valid {
        return "Steel execute_turn host call returned malformed execution payload".to_string();
    }
    "Steel execute_turn host call approved typed Rust host runner request".to_string()
}

fn steel_execution_host_call_receipt_hash(receipt: &SteelTurnExecutionHostCallReceipt) -> ArtifactHash {
    let mut material = receipt.clone();
    material.receipt_hash = ArtifactHash::digest(b"omitted");
    let bytes = serde_json::to_vec(&material).expect("Steel execution host-call receipt serializes");
    ArtifactHash::digest(&bytes)
}

fn stable_execution_input_bytes(input: &SteelTurnExecutionInput) -> Vec<u8> {
    #[derive(Serialize)]
    struct ExecutionInputHashMaterial<'a> {
        turn_id: &'a str,
        target_resource: &'a str,
        plan_receipt_hash: ArtifactHash,
        plan_hash: Option<ArtifactHash>,
        prompt_hash: ArtifactHash,
        host_runner: &'a str,
    }
    let material = ExecutionInputHashMaterial {
        turn_id: &input.turn_id,
        target_resource: &input.target_resource,
        plan_receipt_hash: input.plan_receipt_hash,
        plan_hash: input.plan_hash,
        prompt_hash: input.prompt_hash,
        host_runner: &input.host_runner,
    };
    serde_json::to_vec(&material).expect("Steel execution input material serializes")
}

fn steel_turn_execution_receipt(
    profile: &SteelOrchestrationProfile,
    input: &SteelTurnExecutionInput,
    input_hash: ArtifactHash,
    input_bytes: u64,
    host_call_receipt: SteelTurnExecutionHostCallReceipt,
    authorization_receipt: DynamicRuntimeActionReceipt,
    status: SteelTurnExecutionStatus,
) -> SteelTurnExecutionReceipt {
    let mut receipt = SteelTurnExecutionReceipt {
        schema: STEEL_TURN_EXECUTION_RECEIPT_SCHEMA.to_string(),
        status,
        seam: DEFAULT_TURN_EXECUTION_SEAM.to_string(),
        executor: OrchestrationPlannerKind::SteelScheme,
        host_runner: input.host_runner.clone(),
        plan_receipt_hash: input.plan_receipt_hash,
        plan_hash: input.plan_hash,
        input_hash,
        input_bytes,
        host_call_receipt,
        authorization_receipt,
        redactions: vec![
            "raw_prompt".to_string(),
            "provider_payload".to_string(),
            "compact_ucan".to_string(),
            "raw_proof".to_string(),
            "credential".to_string(),
            "script_source".to_string(),
            "tool_body".to_string(),
        ],
        receipt_hash: ArtifactHash::digest(b"pending"),
    };
    receipt.receipt_hash = steel_turn_execution_receipt_hash(profile, &receipt);
    receipt
}

fn steel_turn_execution_receipt_hash(
    profile: &SteelOrchestrationProfile,
    receipt: &SteelTurnExecutionReceipt,
) -> ArtifactHash {
    #[derive(Serialize)]
    struct ReceiptHashMaterial<'a> {
        schema: &'a str,
        profile_name: &'a str,
        seam: &'a str,
        status: SteelTurnExecutionStatus,
        executor: OrchestrationPlannerKind,
        host_runner: &'a str,
        plan_receipt_hash: ArtifactHash,
        plan_hash: Option<ArtifactHash>,
        input_hash: ArtifactHash,
        input_bytes: u64,
        host_call_receipt_hash: ArtifactHash,
        authorization_receipt_hash: ArtifactHash,
    }
    let material = ReceiptHashMaterial {
        schema: &receipt.schema,
        profile_name: &profile.name,
        seam: &receipt.seam,
        status: receipt.status,
        executor: receipt.executor,
        host_runner: &receipt.host_runner,
        plan_receipt_hash: receipt.plan_receipt_hash,
        plan_hash: receipt.plan_hash,
        input_hash: receipt.input_hash,
        input_bytes: receipt.input_bytes,
        host_call_receipt_hash: receipt.host_call_receipt.receipt_hash,
        authorization_receipt_hash: receipt.authorization_receipt.receipt_hash,
    };
    let bytes = serde_json::to_vec(&material).expect("Steel execution receipt material serializes");
    ArtifactHash::digest(&bytes)
}

struct SteelPlanEvaluationError {
    steel_receipt_hash: ArtifactHash,
    issue_code: OrchestrationIssueCode,
    safe_summary: &'static str,
    evidence: BasaltSteelContractEvidence,
}

enum SteelPlanEvaluation {
    Ready(OrchestrationPlan, ArtifactHash),
    Failed(SteelPlanEvaluationError),
}

fn steel_authority_denied_receipt(
    profile: &SteelOrchestrationProfile,
    basalt_request: &SteelEvaluationRequest,
    fallback_decision_class: Option<String>,
    authority_receipt: SteelTurnPlanningAuthorityReceipt,
) -> OrchestrationPlanReceipt {
    orchestration_receipt(
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
            basalt_request,
            "Steel turn-planning UCAN authority denied before evaluation",
        )),
        Some(authority_receipt),
    )
}

fn evaluate_steel_plan(
    profile: &SteelOrchestrationProfile,
    input: &TurnPlanningInput,
    basalt_request: &SteelEvaluationRequest,
) -> SteelPlanEvaluation {
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
        return SteelPlanEvaluation::Failed(steel_runtime_failure_error(
            basalt_request,
            &steel_receipt,
            steel_receipt_hash,
        ));
    }
    let Some(output) = steel_receipt.output.as_deref() else {
        return SteelPlanEvaluation::Failed(steel_plan_error(SteelPlanErrorInput {
            basalt_request,
            steel_receipt_hash,
            issue_code: OrchestrationIssueCode::MalformedPlan,
            safe_summary: "Steel planner returned no typed plan payload",
            evidence_reason: "Steel runtime returned no typed plan payload",
        }));
    };
    let Ok(steel_plan) = parse_steel_plan_payload(profile, input, output) else {
        return SteelPlanEvaluation::Failed(steel_plan_error(SteelPlanErrorInput {
            basalt_request,
            steel_receipt_hash,
            issue_code: OrchestrationIssueCode::MalformedPlan,
            safe_summary: "Steel planner output did not match the typed plan schema",
            evidence_reason: "Steel planner output did not match the typed plan schema",
        }));
    };
    SteelPlanEvaluation::Ready(steel_plan, steel_receipt_hash)
}

fn steel_runtime_failure_error(
    basalt_request: &SteelEvaluationRequest,
    steel_receipt: &SteelRuntimeReceipt,
    steel_receipt_hash: ArtifactHash,
) -> SteelPlanEvaluationError {
    let issue_code = if steel_receipt.reason_code == SteelRuntimeReasonCode::UnsupportedExpression {
        OrchestrationIssueCode::MalformedPlan
    } else {
        OrchestrationIssueCode::ScriptEvaluationFailed
    };
    steel_plan_error(SteelPlanErrorInput {
        basalt_request,
        steel_receipt_hash,
        issue_code,
        safe_summary: "Steel script evaluation failed before a typed plan was authorized",
        evidence_reason: "Steel runtime did not return a Basalt receipt",
    })
}

struct SteelPlanErrorInput<'a> {
    basalt_request: &'a SteelEvaluationRequest,
    steel_receipt_hash: ArtifactHash,
    issue_code: OrchestrationIssueCode,
    safe_summary: &'static str,
    evidence_reason: &'static str,
}

fn steel_plan_error(input: SteelPlanErrorInput<'_>) -> SteelPlanEvaluationError {
    SteelPlanEvaluationError {
        steel_receipt_hash: input.steel_receipt_hash,
        issue_code: input.issue_code,
        safe_summary: input.safe_summary,
        evidence: BasaltSteelContractEvidence::request_only(input.basalt_request, input.evidence_reason),
    }
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
    let parsed = serde_json::from_str::<SteelTurnPlanHostCallPayload>(payload).map_err(|_| ())?;
    if parsed.schema != STEEL_ORCHESTRATION_PLAN_SCHEMA {
        return Err(());
    }
    let candidate = input
        .candidate_actions
        .iter()
        .find(|candidate| {
            candidate.decision_id == parsed.decision_id
                && candidate.action_name == parsed.action_name
                && candidate.target_resource == parsed.target_resource
        })
        .ok_or(())?;
    let mut selected = candidate.clone();
    selected.decision_class = parsed.decision_class;
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
        max_input_bytes: u64_to_usize_saturating(profile.max_input_bytes),
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

fn u64_to_usize_saturating(value: u64) -> usize {
    match usize::try_from(value) {
        Ok(converted) => converted,
        Err(_) => usize::MAX,
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
        authority_receipt(SteelTurnPlanningAuthorityStatus::Denied, reason, AuthorityReceiptInput {
            requested_resource,
            requested_ability,
            grant,
            grant_count: grants.len(),
            basalt_reason,
        })
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
    let basalt_authority = BasaltAuthority {
        resource: authority_basalt_resource(requested_resource),
        ability: authority_basalt_ability(requested_ability),
    };
    match basalt_enforce(&authority_policy(&basalt_authority), &authority_request(&basalt_authority)) {
        Ok(receipt) if receipt.is_allowed() => authority_receipt(
            SteelTurnPlanningAuthorityStatus::Allowed,
            SteelTurnPlanningAuthorityReason::Allowed,
            AuthorityReceiptInput {
                requested_resource,
                requested_ability,
                grant: Some(grant),
                grant_count: grants.len(),
                basalt_reason: Some(receipt.reason().to_string()),
            },
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

struct BasaltAuthority {
    resource: String,
    ability: String,
}

fn authority_policy(authority: &BasaltAuthority) -> BasaltPolicy {
    BasaltPolicy {
        schema_version: basalt::SUPPORTED_SCHEMA_VERSION.to_string(),
        contracts: std::collections::BTreeMap::from([(
            STEEL_TURN_PLANNING_AUTHORITY_CONTRACT.to_string(),
            BasaltContractPolicy {
                id: STEEL_TURN_PLANNING_AUTHORITY_CONTRACT.to_string(),
                description: "Authorize reviewed Steel turn-planning invocation".to_string(),
                resource_prefixes: vec![authority.resource.clone()],
                abilities: vec![authority.ability.clone()],
            },
        )]),
        backends: std::collections::BTreeMap::new(),
    }
}

fn authority_request(authority: &BasaltAuthority) -> BasaltEnforcementRequest {
    BasaltEnforcementRequest::new(
        STEEL_TURN_PLANNING_AUTHORITY_CONTRACT,
        authority.resource.clone(),
        authority.ability.clone(),
    )
    .with_capability(BasaltCapabilityGrant::new(authority.resource.clone(), authority.ability.clone()))
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

struct AuthorityReceiptInput<'a> {
    requested_resource: &'a str,
    requested_ability: &'a str,
    grant: Option<&'a SteelTurnPlanningAuthorityGrant>,
    grant_count: usize,
    basalt_reason: Option<String>,
}

fn authority_receipt(
    status: SteelTurnPlanningAuthorityStatus,
    reason: SteelTurnPlanningAuthorityReason,
    input: AuthorityReceiptInput<'_>,
) -> SteelTurnPlanningAuthorityReceipt {
    let mut receipt = SteelTurnPlanningAuthorityReceipt {
        schema: STEEL_TURN_PLANNING_AUTHORITY_SCHEMA.to_string(),
        status,
        reason,
        seam: DEFAULT_TURN_PLANNING_SEAM.to_string(),
        resource: route_slug(input.requested_resource),
        ability: input.requested_ability.to_string(),
        audience: input
            .grant
            .map(|grant| route_slug(&grant.audience))
            .unwrap_or_else(|| route_slug(STEEL_TURN_PLANNING_AUDIENCE)),
        proof_reference: input.grant.and_then(|grant| grant.proof_reference.as_deref()).map(route_slug),
        grant_count: input.grant_count,
        caveat_classes: input
            .grant
            .map(|grant| grant.caveats.iter().map(|caveat| route_slug(caveat)).collect())
            .unwrap_or_default(),
        basalt_reason: input.basalt_reason.map(|reason| route_slug(&reason)),
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
    dynamic_authorization_context(
        profile,
        &input.session_capabilities,
        &input.granted_ucan_abilities,
        &input.ucan_authority_grants,
        &input.disabled_actions,
    )
}

fn dynamic_authorization_context(
    profile: &SteelOrchestrationProfile,
    session_capabilities: &[String],
    granted_ucan_abilities: &[String],
    ucan_authority_grants: &[SteelTurnPlanningAuthorityGrant],
    disabled_actions: &[String],
) -> DynamicRuntimeAuthorizationContext {
    let allowed_actions = profile
        .allowed_host_actions
        .iter()
        .map(|action| format!("host_function:{action}"))
        .collect::<BTreeSet<_>>();
    let mut granted_ucan_abilities = granted_ucan_abilities.iter().cloned().collect::<BTreeSet<_>>();
    granted_ucan_abilities.extend(ucan_authority_grants.iter().map(|grant| grant.ability.clone()));
    DynamicRuntimeAuthorizationContext {
        allowed_runtime_profiles: BTreeSet::from([profile.runtime_profile.name.clone()]),
        allowed_actions,
        granted_ucan_abilities,
        session_capabilities: session_capabilities.iter().cloned().collect(),
        disabled_actions: disabled_actions.iter().cloned().collect(),
        max_input_bytes: profile.max_input_bytes,
    }
}

// Receipt construction mirrors the stable wire schema fields to keep hash material explicit.
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
        SteelTurnPlanHostCallPayload::new(
            "first",
            DEFAULT_TURN_PLANNING_SEAM,
            "turn:first",
            "steel_selected_tool_candidate_ordering",
        )
        .to_json()
    }

    fn legacy_delimited_payload() -> String {
        format!(
            "{}|first|{}|turn:first|steel_selected_tool_candidate_ordering",
            STEEL_ORCHESTRATION_PLAN_SCHEMA, DEFAULT_TURN_PLANNING_SEAM
        )
    }

    fn execution_input(plan_receipt: &OrchestrationPlanReceipt) -> SteelTurnExecutionInput {
        SteelTurnExecutionInput {
            turn_id: "session-1:1".to_string(),
            target_resource: "session:session-1".to_string(),
            plan_receipt_hash: plan_receipt.receipt_hash,
            plan_hash: plan_receipt.plan_hash,
            prompt_hash: ArtifactHash::digest(b"prompt"),
            host_runner: "RustHostRunner".to_string(),
            session_capabilities: vec!["steel-orchestration".to_string(), "turn-execution".to_string()],
            disabled_actions: Vec::new(),
            granted_ucan_abilities: vec!["clankers/steel/orchestrate.execute_turn".to_string()],
            ucan_authority_grants: Vec::new(),
        }
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
    fn execute_turn_authority_requires_execution_capability_and_ucan() {
        let profile = profile();
        let plan_receipt = plan_turn_with_steel_or_fallback(&profile, &input_with_payload(valid_payload()));
        let allowed = authorize_steel_turn_execution(&profile, &execution_input(&plan_receipt));
        assert_eq!(allowed.schema, STEEL_TURN_EXECUTION_RECEIPT_SCHEMA);
        assert_eq!(allowed.status, SteelTurnExecutionStatus::Authorized);
        assert_eq!(allowed.seam, DEFAULT_TURN_EXECUTION_SEAM);
        assert_eq!(allowed.authorization_receipt.status, DynamicRuntimeActionStatus::Allowed);
        assert!(allowed.host_call_receipt.is_allowed());
        assert_eq!(allowed.host_call_receipt.runtime_status, SteelRuntimeStatusCode::Succeeded);
        assert_eq!(allowed.host_call_receipt.runtime_reason, SteelRuntimeReasonCode::Ok);
        assert_eq!(allowed.host_call_receipt.host_call_outcome, Some(SteelHostCallOutcome::Approved));
        assert_eq!(allowed.authorization_receipt.reason, DynamicRuntimeActionReason::Ready);
        assert_eq!(allowed.authorization_receipt.required_ucan_ability, "clankers/steel/orchestrate.execute_turn");
        assert_eq!(allowed.authorization_receipt.required_session_capabilities, vec![
            "steel-orchestration",
            "turn-execution"
        ]);
        assert!(allowed.redactions.contains(&"raw_prompt".to_string()));
        assert!(allowed.receipt_hash.prefixed().starts_with("b3:"));
    }

    #[test]
    fn execute_turn_authority_denies_missing_ucan_or_disabled_action_before_host_runner() {
        let profile = profile();
        let plan_receipt = plan_turn_with_steel_or_fallback(&profile, &input_with_payload(valid_payload()));
        let mut missing_ucan = execution_input(&plan_receipt);
        missing_ucan.granted_ucan_abilities.clear();
        let denied_ucan = authorize_steel_turn_execution(&profile, &missing_ucan);
        assert_eq!(denied_ucan.status, SteelTurnExecutionStatus::Denied);
        assert!(denied_ucan.host_call_receipt.is_allowed());
        assert_eq!(denied_ucan.authorization_receipt.status, DynamicRuntimeActionStatus::UcanDenied);
        assert_eq!(denied_ucan.authorization_receipt.reason, DynamicRuntimeActionReason::MissingUcanAbility);

        let mut missing_capability = execution_input(&plan_receipt);
        missing_capability.session_capabilities.retain(|capability| capability != "turn-execution");
        let denied_capability = authorize_steel_turn_execution(&profile, &missing_capability);
        assert_eq!(denied_capability.status, SteelTurnExecutionStatus::Denied);
        assert!(!denied_capability.host_call_receipt.is_allowed());
        assert_eq!(denied_capability.host_call_receipt.runtime_status, SteelRuntimeStatusCode::Denied);
        assert_eq!(denied_capability.host_call_receipt.runtime_reason, SteelRuntimeReasonCode::MissingHostCapability);
        assert_eq!(denied_capability.authorization_receipt.status, DynamicRuntimeActionStatus::PolicyDenied);
        assert_eq!(
            denied_capability.authorization_receipt.reason,
            DynamicRuntimeActionReason::MissingSessionCapability
        );

        let mut disabled = execution_input(&plan_receipt);
        disabled.disabled_actions = vec![DEFAULT_TURN_EXECUTION_SEAM.to_string()];
        let disabled_receipt = authorize_steel_turn_execution(&profile, &disabled);
        assert_eq!(disabled_receipt.status, SteelTurnExecutionStatus::Denied);
        assert!(!disabled_receipt.host_call_receipt.is_allowed());
        assert_eq!(disabled_receipt.host_call_receipt.runtime_reason, SteelRuntimeReasonCode::DisabledHostFunction);
        assert_eq!(disabled_receipt.authorization_receipt.status, DynamicRuntimeActionStatus::Disabled);
        assert_eq!(disabled_receipt.authorization_receipt.reason, DynamicRuntimeActionReason::DisabledAction);
    }

    #[test]
    fn execute_turn_host_call_rejects_malformed_payload_before_authorized_status() {
        let profile = profile();
        let plan_receipt = plan_turn_with_steel_or_fallback(&profile, &input_with_payload(valid_payload()));
        let input = execution_input(&plan_receipt);
        let request = SteelRuntimeRequest {
            profile: profile.runtime_profile.clone(),
            source: DEFAULT_TURN_EXECUTION_SOURCE.to_string(),
            session_capabilities: input.session_capabilities.clone(),
            disabled_tools: Vec::new(),
            host_functions: Vec::new(),
            receipt_destination: "target/steel-execute-runtime.json".to_string(),
        };
        let runtime_receipt = SteelRuntimeReceipt {
            schema: crate::steel_runtime::STEEL_RUNTIME_RECEIPT_SCHEMA.to_string(),
            status: SteelRuntimeStatusCode::Succeeded,
            reason_code: SteelRuntimeReasonCode::Ok,
            safe_message: "fixture malformed payload".to_string(),
            profile_name: profile.runtime_profile.name.clone(),
            source_hash: ArtifactHash::digest(DEFAULT_TURN_EXECUTION_SOURCE.as_bytes()),
            output_hash: Some(ArtifactHash::digest(b"malformed")),
            output: Some("malformed".to_string()),
            host_calls: vec![SteelHostCallReceipt {
                name: DEFAULT_TURN_EXECUTION_SEAM.to_string(),
                outcome: SteelHostCallOutcome::Approved,
                safe_message: "approved but malformed".to_string(),
            }],
            redactions: vec!["source".to_string()],
            steps_used: 1,
            ambient_authority: false,
            sandbox_claim: "fixture".to_string(),
        };
        let host_call = steel_execution_host_call_receipt(&request, &input, &runtime_receipt);
        assert!(!host_call.is_allowed());
        assert!(!host_call.payload_valid);
        assert!(host_call.safe_summary.contains("malformed"));
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
        for payload in ["not-a-plan".to_string(), legacy_delimited_payload()] {
            let receipt = plan_turn_with_steel_or_fallback(&profile(), &input_with_payload(payload));
            assert_eq!(receipt.status, OrchestrationPlanStatus::FallbackUsed);
            assert_eq!(receipt.issue_code, OrchestrationIssueCode::MalformedPlan);
            assert_eq!(receipt.planner, OrchestrationPlannerKind::RustNative);
            assert_eq!(receipt.fallback_status, RustNativeFallbackStatus::Used);
        }
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
        let mut input = input_with_payload(
            SteelTurnPlanHostCallPayload::new("first", "steel.host.provider", "turn:first", "bad-provider").to_json(),
        );
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
