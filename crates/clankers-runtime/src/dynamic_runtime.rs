//! Typed action envelope for dynamic runtimes.
//!
//! Steel Scheme and Wasm code are requesters at this seam. They provide a
//! typed envelope describing the desired host function/tool, target resource,
//! runtime profile, authority, and receipt destination. Rust evaluates that
//! envelope before any host effect and emits a safe receipt that does not carry
//! raw prompts, provider payloads, compact UCAN tokens, credentials, or large
//! bodies.

use std::collections::BTreeSet;

use clankers_artifacts::ArtifactHash;
use serde::Deserialize;
use serde::Serialize;

pub const DYNAMIC_RUNTIME_ACTION_SCHEMA: &str = "clankers.dynamic_runtime.action.v1";
pub const DYNAMIC_RUNTIME_RECEIPT_SCHEMA: &str = "clankers.dynamic_runtime.action_receipt.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicRuntimeKind {
    SteelScheme,
    Wasm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicRuntimeActionKind {
    HostFunction,
    Tool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicRuntimeRedactionClass {
    PublicSummary,
    MetadataOnly,
    SecretBearing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicRuntimeActionEnvelope {
    pub schema: String,
    pub action_id: String,
    pub runtime: DynamicRuntimeKind,
    pub runtime_profile: String,
    pub action_kind: DynamicRuntimeActionKind,
    pub action_name: String,
    pub target_resource: String,
    pub receipt_destination: String,
    pub required_ucan_ability: String,
    pub required_session_capabilities: Vec<String>,
    pub input_hash: ArtifactHash,
    pub input_bytes: u64,
    pub redaction: DynamicRuntimeRedactionClass,
}

impl DynamicRuntimeActionEnvelope {
    #[must_use]
    pub fn stable_action_key(&self) -> String {
        format!("{}:{}", action_kind_tag(self.action_kind), self.action_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicRuntimeAuthorizationContext {
    pub allowed_runtime_profiles: BTreeSet<String>,
    pub allowed_actions: BTreeSet<String>,
    pub granted_ucan_abilities: BTreeSet<String>,
    pub session_capabilities: BTreeSet<String>,
    pub disabled_actions: BTreeSet<String>,
    pub max_input_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicRuntimeActionStatus {
    Allowed,
    PolicyDenied,
    UcanDenied,
    Disabled,
    InvalidEnvelope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicRuntimeActionReason {
    Ready,
    InvalidSchema,
    MissingRequiredField,
    UnsupportedRuntimeProfile,
    UnsupportedAction,
    DisabledAction,
    MissingSessionCapability,
    MissingUcanAbility,
    SecretBearingInput,
    InputTooLarge,
    UnsafeReceiptDestination,
    UnsafeTargetResource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicRuntimeActionReceipt {
    pub schema: String,
    pub action_id: String,
    pub runtime: DynamicRuntimeKind,
    pub runtime_profile: String,
    pub action_kind: DynamicRuntimeActionKind,
    pub action_name: String,
    pub target_resource: String,
    pub receipt_destination: String,
    pub status: DynamicRuntimeActionStatus,
    pub reason: DynamicRuntimeActionReason,
    pub safe_summary: String,
    pub required_ucan_ability: String,
    pub required_session_capabilities: Vec<String>,
    pub input_hash: ArtifactHash,
    pub input_bytes: u64,
    pub writes_performed: bool,
    pub receipt_hash: ArtifactHash,
}

#[derive(Serialize)]
struct DynamicRuntimeReceiptHashMaterial<'a> {
    schema: &'a str,
    action_id: &'a str,
    runtime: DynamicRuntimeKind,
    runtime_profile: &'a str,
    action_kind: DynamicRuntimeActionKind,
    action_name: &'a str,
    target_resource: &'a str,
    receipt_destination: &'a str,
    status: DynamicRuntimeActionStatus,
    reason: DynamicRuntimeActionReason,
    safe_summary: &'a str,
    required_ucan_ability: &'a str,
    required_session_capabilities: &'a [String],
    input_hash: ArtifactHash,
    input_bytes: u64,
    writes_performed: bool,
}

#[must_use]
pub fn authorize_dynamic_runtime_action(
    envelope: &DynamicRuntimeActionEnvelope,
    context: &DynamicRuntimeAuthorizationContext,
) -> DynamicRuntimeActionReceipt {
    let (status, reason, safe_summary) = dynamic_runtime_decision(envelope, context);
    dynamic_runtime_receipt(envelope, status, reason, safe_summary)
}

fn dynamic_runtime_decision(
    envelope: &DynamicRuntimeActionEnvelope,
    context: &DynamicRuntimeAuthorizationContext,
) -> (DynamicRuntimeActionStatus, DynamicRuntimeActionReason, String) {
    if envelope.schema != DYNAMIC_RUNTIME_ACTION_SCHEMA {
        return invalid(DynamicRuntimeActionReason::InvalidSchema, "unsupported dynamic-runtime action schema");
    }
    if is_blank(&envelope.action_id)
        || is_blank(&envelope.runtime_profile)
        || is_blank(&envelope.action_name)
        || is_blank(&envelope.target_resource)
        || is_blank(&envelope.receipt_destination)
        || is_blank(&envelope.required_ucan_ability)
    {
        return invalid(
            DynamicRuntimeActionReason::MissingRequiredField,
            "dynamic-runtime action is missing required metadata",
        );
    }
    if !safe_resource(&envelope.target_resource) {
        return invalid(
            DynamicRuntimeActionReason::UnsafeTargetResource,
            "target resource must be a scoped logical resource",
        );
    }
    if !safe_receipt_destination(&envelope.receipt_destination) {
        return invalid(
            DynamicRuntimeActionReason::UnsafeReceiptDestination,
            "receipt destination must stay under target/",
        );
    }
    if envelope.redaction == DynamicRuntimeRedactionClass::SecretBearing {
        return invalid(
            DynamicRuntimeActionReason::SecretBearingInput,
            "secret-bearing action inputs are not accepted at this seam",
        );
    }
    if envelope.input_bytes > context.max_input_bytes {
        return (
            DynamicRuntimeActionStatus::PolicyDenied,
            DynamicRuntimeActionReason::InputTooLarge,
            "dynamic-runtime action input exceeds profile budget".to_string(),
        );
    }
    if !context.allowed_runtime_profiles.contains(&envelope.runtime_profile) {
        return (
            DynamicRuntimeActionStatus::PolicyDenied,
            DynamicRuntimeActionReason::UnsupportedRuntimeProfile,
            "runtime profile is not allowed by policy".to_string(),
        );
    }
    let action_key = envelope.stable_action_key();
    if !context.allowed_actions.contains(&action_key) {
        return (
            DynamicRuntimeActionStatus::PolicyDenied,
            DynamicRuntimeActionReason::UnsupportedAction,
            "requested action is not allowed by policy".to_string(),
        );
    }
    if context.disabled_actions.contains(&action_key) || context.disabled_actions.contains(&envelope.action_name) {
        return (
            DynamicRuntimeActionStatus::Disabled,
            DynamicRuntimeActionReason::DisabledAction,
            "requested action is disabled for this session".to_string(),
        );
    }
    if envelope
        .required_session_capabilities
        .iter()
        .any(|capability| !context.session_capabilities.contains(capability))
    {
        return (
            DynamicRuntimeActionStatus::PolicyDenied,
            DynamicRuntimeActionReason::MissingSessionCapability,
            "session lacks a required capability".to_string(),
        );
    }
    if !context.granted_ucan_abilities.contains(&envelope.required_ucan_ability) {
        return (
            DynamicRuntimeActionStatus::UcanDenied,
            DynamicRuntimeActionReason::MissingUcanAbility,
            "matching UCAN authority was not delegated".to_string(),
        );
    }
    (
        DynamicRuntimeActionStatus::Allowed,
        DynamicRuntimeActionReason::Ready,
        "dynamic-runtime action authorized; no host effect has run".to_string(),
    )
}

fn dynamic_runtime_receipt(
    envelope: &DynamicRuntimeActionEnvelope,
    status: DynamicRuntimeActionStatus,
    reason: DynamicRuntimeActionReason,
    safe_summary: String,
) -> DynamicRuntimeActionReceipt {
    let required_session_capabilities = sorted_unique(envelope.required_session_capabilities.clone());
    let material = DynamicRuntimeReceiptHashMaterial {
        schema: DYNAMIC_RUNTIME_RECEIPT_SCHEMA,
        action_id: &envelope.action_id,
        runtime: envelope.runtime,
        runtime_profile: &envelope.runtime_profile,
        action_kind: envelope.action_kind,
        action_name: &envelope.action_name,
        target_resource: &envelope.target_resource,
        receipt_destination: &envelope.receipt_destination,
        status,
        reason,
        safe_summary: &safe_summary,
        required_ucan_ability: &envelope.required_ucan_ability,
        required_session_capabilities: &required_session_capabilities,
        input_hash: envelope.input_hash,
        input_bytes: envelope.input_bytes,
        writes_performed: false,
    };
    let bytes = serde_json::to_vec(&material).expect("dynamic runtime receipt material serializes");
    DynamicRuntimeActionReceipt {
        schema: DYNAMIC_RUNTIME_RECEIPT_SCHEMA.to_string(),
        action_id: envelope.action_id.clone(),
        runtime: envelope.runtime,
        runtime_profile: envelope.runtime_profile.clone(),
        action_kind: envelope.action_kind,
        action_name: envelope.action_name.clone(),
        target_resource: envelope.target_resource.clone(),
        receipt_destination: envelope.receipt_destination.clone(),
        status,
        reason,
        safe_summary,
        required_ucan_ability: envelope.required_ucan_ability.clone(),
        required_session_capabilities,
        input_hash: envelope.input_hash,
        input_bytes: envelope.input_bytes,
        writes_performed: false,
        receipt_hash: ArtifactHash::digest(&bytes),
    }
}

fn invalid(
    reason: DynamicRuntimeActionReason,
    safe_summary: &str,
) -> (DynamicRuntimeActionStatus, DynamicRuntimeActionReason, String) {
    (DynamicRuntimeActionStatus::InvalidEnvelope, reason, safe_summary.to_string())
}

fn is_blank(value: &str) -> bool {
    value.trim().is_empty()
}

fn safe_resource(value: &str) -> bool {
    !value.contains("..") && value.contains(':') && !value.contains("//") && !value.contains('\0')
}

fn safe_receipt_destination(value: &str) -> bool {
    value.starts_with("target/") && !value.contains("..") && !value.contains('\0')
}

fn sorted_unique(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn action_kind_tag(kind: DynamicRuntimeActionKind) -> &'static str {
    match kind {
        DynamicRuntimeActionKind::HostFunction => "host_function",
        DynamicRuntimeActionKind::Tool => "tool",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope() -> DynamicRuntimeActionEnvelope {
        let input = br#"{"intent":"update prompt"}"#;
        DynamicRuntimeActionEnvelope {
            schema: DYNAMIC_RUNTIME_ACTION_SCHEMA.to_string(),
            action_id: "action-1".to_string(),
            runtime: DynamicRuntimeKind::SteelScheme,
            runtime_profile: "steel-orchestrator/default".to_string(),
            action_kind: DynamicRuntimeActionKind::HostFunction,
            action_name: "steel.host.propose_mutation".to_string(),
            target_resource: "prompt:system".to_string(),
            receipt_destination: "target/polyglot-agent/action-1.json".to_string(),
            required_ucan_ability: "clankers/mutation.propose".to_string(),
            required_session_capabilities: vec!["workspace_mutation".to_string(), "steel_host_functions".to_string()],
            input_hash: ArtifactHash::digest(input),
            input_bytes: input.len() as u64,
            redaction: DynamicRuntimeRedactionClass::MetadataOnly,
        }
    }

    fn context() -> DynamicRuntimeAuthorizationContext {
        DynamicRuntimeAuthorizationContext {
            allowed_runtime_profiles: BTreeSet::from([
                "steel-orchestrator/default".to_string(),
                "wasm-tool/default".to_string(),
            ]),
            allowed_actions: BTreeSet::from([
                "host_function:steel.host.propose_mutation".to_string(),
                "tool:wasm.safe_tool".to_string(),
            ]),
            granted_ucan_abilities: BTreeSet::from(["clankers/mutation.propose".to_string()]),
            session_capabilities: BTreeSet::from([
                "workspace_mutation".to_string(),
                "steel_host_functions".to_string(),
            ]),
            disabled_actions: BTreeSet::new(),
            max_input_bytes: 1024,
        }
    }

    #[test]
    fn steel_host_function_envelope_can_be_authorized_without_side_effects() {
        let receipt = authorize_dynamic_runtime_action(&envelope(), &context());

        assert_eq!(receipt.status, DynamicRuntimeActionStatus::Allowed);
        assert_eq!(receipt.reason, DynamicRuntimeActionReason::Ready);
        assert!(!receipt.writes_performed);
        assert_eq!(receipt.action_name, "steel.host.propose_mutation");
        assert_eq!(receipt.required_session_capabilities, vec!["steel_host_functions", "workspace_mutation"]);
        let serialized = serde_json::to_string(&receipt).expect("receipt json");
        assert!(!serialized.contains("update prompt"));
        assert!(!serialized.contains("Bearer"));
    }

    #[test]
    fn wasm_tool_envelope_uses_same_authorization_seam() {
        let mut request = envelope();
        request.runtime = DynamicRuntimeKind::Wasm;
        request.runtime_profile = "wasm-tool/default".to_string();
        request.action_kind = DynamicRuntimeActionKind::Tool;
        request.action_name = "wasm.safe_tool".to_string();
        request.target_resource = "tool:wasm.safe_tool".to_string();
        request.required_ucan_ability = "clankers/mutation.propose".to_string();

        let receipt = authorize_dynamic_runtime_action(&request, &context());

        assert_eq!(receipt.status, DynamicRuntimeActionStatus::Allowed);
        assert_eq!(receipt.runtime, DynamicRuntimeKind::Wasm);
        assert_eq!(receipt.action_kind, DynamicRuntimeActionKind::Tool);
        assert!(!receipt.writes_performed);
    }

    #[test]
    fn policy_denial_precedes_ucan_when_action_is_not_allowed() {
        let mut request = envelope();
        request.action_name = "steel.host.raw_write".to_string();
        request.required_ucan_ability = "clankers/raw.write".to_string();
        let receipt = authorize_dynamic_runtime_action(&request, &context());

        assert_eq!(receipt.status, DynamicRuntimeActionStatus::PolicyDenied);
        assert_eq!(receipt.reason, DynamicRuntimeActionReason::UnsupportedAction);
        assert!(!receipt.writes_performed);
    }

    #[test]
    fn ucan_denial_blocks_policy_allowed_action_without_side_effects() {
        let mut context = context();
        context.granted_ucan_abilities.clear();
        let receipt = authorize_dynamic_runtime_action(&envelope(), &context);

        assert_eq!(receipt.status, DynamicRuntimeActionStatus::UcanDenied);
        assert_eq!(receipt.reason, DynamicRuntimeActionReason::MissingUcanAbility);
        assert!(!receipt.writes_performed);
    }

    #[test]
    fn disabled_actions_and_missing_session_capabilities_fail_closed() {
        let mut disabled = context();
        disabled.disabled_actions.insert("host_function:steel.host.propose_mutation".to_string());
        let disabled_receipt = authorize_dynamic_runtime_action(&envelope(), &disabled);
        assert_eq!(disabled_receipt.status, DynamicRuntimeActionStatus::Disabled);
        assert_eq!(disabled_receipt.reason, DynamicRuntimeActionReason::DisabledAction);

        let mut missing_capability = context();
        missing_capability.session_capabilities.remove("workspace_mutation");
        let missing_receipt = authorize_dynamic_runtime_action(&envelope(), &missing_capability);
        assert_eq!(missing_receipt.status, DynamicRuntimeActionStatus::PolicyDenied);
        assert_eq!(missing_receipt.reason, DynamicRuntimeActionReason::MissingSessionCapability);
    }

    #[test]
    fn invalid_envelope_rejects_secret_or_unsafe_material_before_policy() {
        let mut secret = envelope();
        secret.redaction = DynamicRuntimeRedactionClass::SecretBearing;
        let secret_receipt = authorize_dynamic_runtime_action(&secret, &context());
        assert_eq!(secret_receipt.status, DynamicRuntimeActionStatus::InvalidEnvelope);
        assert_eq!(secret_receipt.reason, DynamicRuntimeActionReason::SecretBearingInput);

        let mut unsafe_target = envelope();
        unsafe_target.target_resource = "../prompts/system".to_string();
        let unsafe_target_receipt = authorize_dynamic_runtime_action(&unsafe_target, &context());
        assert_eq!(unsafe_target_receipt.status, DynamicRuntimeActionStatus::InvalidEnvelope);
        assert_eq!(unsafe_target_receipt.reason, DynamicRuntimeActionReason::UnsafeTargetResource);

        let mut unsafe_receipt = envelope();
        unsafe_receipt.receipt_destination = "/tmp/receipt.json".to_string();
        let unsafe_receipt_result = authorize_dynamic_runtime_action(&unsafe_receipt, &context());
        assert_eq!(unsafe_receipt_result.status, DynamicRuntimeActionStatus::InvalidEnvelope);
        assert_eq!(unsafe_receipt_result.reason, DynamicRuntimeActionReason::UnsafeReceiptDestination);
    }
}
