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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelAmbientAccessKind {
    Filesystem,
    Shell,
    Git,
    Network,
    Provider,
    Credential,
    Daemon,
    Tui,
    NativeTool,
}

impl SteelAmbientAccessKind {
    #[must_use]
    pub fn all() -> [Self; 9] {
        [
            Self::Filesystem,
            Self::Shell,
            Self::Git,
            Self::Network,
            Self::Provider,
            Self::Credential,
            Self::Daemon,
            Self::Tui,
            Self::NativeTool,
        ]
    }

    #[must_use]
    pub const fn host_function_name(self) -> &'static str {
        match self {
            Self::Filesystem => "steel.ambient.fs",
            Self::Shell => "steel.ambient.shell",
            Self::Git => "steel.ambient.git",
            Self::Network => "steel.ambient.network",
            Self::Provider => "steel.ambient.provider",
            Self::Credential => "steel.ambient.credential",
            Self::Daemon => "steel.ambient.daemon",
            Self::Tui => "steel.ambient.tui",
            Self::NativeTool => "steel.ambient.native_tool",
        }
    }

    #[must_use]
    pub const fn target_resource(self) -> &'static str {
        match self {
            Self::Filesystem => "fs:ambient",
            Self::Shell => "process:shell",
            Self::Git => "git:ambient",
            Self::Network => "network:ambient",
            Self::Provider => "provider:ambient",
            Self::Credential => "credential:ambient",
            Self::Daemon => "daemon:ambient",
            Self::Tui => "tui:ambient",
            Self::NativeTool => "native-tool:ambient",
        }
    }

    #[must_use]
    pub const fn route_hint(self) -> &'static str {
        match self {
            Self::Filesystem => "raw filesystem",
            Self::Shell => "shell command",
            Self::Git => "git operation",
            Self::Network => "network request",
            Self::Provider => "provider call",
            Self::Credential => "credential read",
            Self::Daemon => "daemon access",
            Self::Tui => "tui mutation",
            Self::NativeTool => "native tool",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FakeSteelOrchestrationProfile {
    pub runtime_profile: String,
    pub allowed_host_functions: BTreeSet<String>,
    pub required_session_capabilities: Vec<String>,
    pub default_ucan_ability: String,
    pub receipt_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FakeSteelOrchestrationRequest {
    pub script_id: String,
    pub route_hint: String,
    pub target_resource: String,
    pub requested_host_function: String,
    pub input_summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FakeSteelOrchestrationReceipt {
    pub selected_action: DynamicRuntimeActionEnvelope,
    pub authorization_receipt: DynamicRuntimeActionReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WasmToolExecutionProfile {
    pub runtime_profile: String,
    pub allowed_imports: BTreeSet<String>,
    pub required_session_capabilities: Vec<String>,
    pub required_ucan_ability: String,
    pub max_memory_pages: u32,
    pub max_fuel: u64,
    pub max_time_ms: u64,
    pub input_schema: String,
    pub output_schema: String,
    pub receipt_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WasmToolExecutionRequest {
    pub tool_name: String,
    pub target_resource: String,
    pub required_imports: Vec<String>,
    pub input_summary: String,
    pub requested_memory_pages: u32,
    pub requested_fuel: u64,
    pub requested_time_ms: u64,
    pub input_schema: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WasmToolExecutionStatus {
    Completed,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WasmToolExecutionReceipt {
    pub status: WasmToolExecutionStatus,
    pub authorization_receipt: DynamicRuntimeActionReceipt,
    pub used_imports: Vec<String>,
    pub memory_pages: u32,
    pub fuel: u64,
    pub time_ms: u64,
    pub output_hash: Option<ArtifactHash>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossLayerFixtureReceipt {
    pub nickel_profile_validated: bool,
    pub steel_route_receipt: FakeSteelOrchestrationReceipt,
    pub wasm_execution_receipt: WasmToolExecutionReceipt,
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

#[must_use]
pub fn steel_ambient_access_negative_fixtures() -> Vec<FakeSteelOrchestrationRequest> {
    SteelAmbientAccessKind::all()
        .into_iter()
        .map(|kind| FakeSteelOrchestrationRequest {
            script_id: "ambient-deny-matrix".to_string(),
            route_hint: kind.route_hint().to_string(),
            target_resource: kind.target_resource().to_string(),
            requested_host_function: kind.host_function_name().to_string(),
            input_summary: format!("attempted ambient {} access", kind.route_hint()),
        })
        .collect()
}

#[must_use]
pub fn run_fake_wasm_tool_execution(
    profile: &WasmToolExecutionProfile,
    request: &WasmToolExecutionRequest,
    context: &DynamicRuntimeAuthorizationContext,
) -> WasmToolExecutionReceipt {
    let input_hash = ArtifactHash::digest(request.input_summary.as_bytes());
    let envelope = DynamicRuntimeActionEnvelope {
        schema: DYNAMIC_RUNTIME_ACTION_SCHEMA.to_string(),
        action_id: format!("wasm:{}", route_slug(&request.tool_name)),
        runtime: DynamicRuntimeKind::Wasm,
        runtime_profile: profile.runtime_profile.clone(),
        action_kind: DynamicRuntimeActionKind::Tool,
        action_name: request.tool_name.clone(),
        target_resource: request.target_resource.clone(),
        receipt_destination: format!(
            "{}/{}.json",
            profile.receipt_prefix.trim_end_matches('/'),
            route_slug(&request.tool_name)
        ),
        required_ucan_ability: profile.required_ucan_ability.clone(),
        required_session_capabilities: profile.required_session_capabilities.clone(),
        input_hash,
        input_bytes: request.input_summary.len() as u64,
        redaction: DynamicRuntimeRedactionClass::MetadataOnly,
    };

    let (authorization_receipt, status) = if request.input_schema != profile.input_schema {
        (
            dynamic_runtime_receipt(
                &envelope,
                DynamicRuntimeActionStatus::InvalidEnvelope,
                DynamicRuntimeActionReason::MissingRequiredField,
                "Wasm tool input schema did not match the execution profile".to_string(),
            ),
            WasmToolExecutionStatus::Blocked,
        )
    } else if request.required_imports.iter().any(|import| !profile.allowed_imports.contains(import)) {
        (
            dynamic_runtime_receipt(
                &envelope,
                DynamicRuntimeActionStatus::PolicyDenied,
                DynamicRuntimeActionReason::UnsupportedAction,
                "Wasm tool requested an import not exposed by the profile".to_string(),
            ),
            WasmToolExecutionStatus::Blocked,
        )
    } else if request.requested_memory_pages > profile.max_memory_pages
        || request.requested_fuel > profile.max_fuel
        || request.requested_time_ms > profile.max_time_ms
    {
        (
            dynamic_runtime_receipt(
                &envelope,
                DynamicRuntimeActionStatus::PolicyDenied,
                DynamicRuntimeActionReason::InputTooLarge,
                "Wasm tool exceeded memory, fuel, or time budget".to_string(),
            ),
            WasmToolExecutionStatus::Blocked,
        )
    } else {
        let receipt = authorize_dynamic_runtime_action(&envelope, context);
        let status = if receipt.status == DynamicRuntimeActionStatus::Allowed {
            WasmToolExecutionStatus::Completed
        } else {
            WasmToolExecutionStatus::Blocked
        };
        (receipt, status)
    };

    let output_hash = if status == WasmToolExecutionStatus::Completed {
        Some(ArtifactHash::digest(
            format!("{}:{}:{:?}", request.tool_name, profile.output_schema, authorization_receipt.receipt_hash)
                .as_bytes(),
        ))
    } else {
        None
    };

    WasmToolExecutionReceipt {
        status,
        authorization_receipt,
        used_imports: sorted_unique(request.required_imports.clone()),
        memory_pages: request.requested_memory_pages,
        fuel: request.requested_fuel,
        time_ms: request.requested_time_ms,
        output_hash,
    }
}

#[must_use]
pub fn run_cross_layer_fixture(
    steel_profile: &FakeSteelOrchestrationProfile,
    steel_request: &FakeSteelOrchestrationRequest,
    wasm_profile: &WasmToolExecutionProfile,
    wasm_request: &WasmToolExecutionRequest,
    context: &DynamicRuntimeAuthorizationContext,
) -> CrossLayerFixtureReceipt {
    let steel_route_receipt = run_fake_steel_orchestration(steel_profile, steel_request, context);
    let wasm_execution_receipt =
        if steel_route_receipt.authorization_receipt.status == DynamicRuntimeActionStatus::Allowed {
            run_fake_wasm_tool_execution(wasm_profile, wasm_request, context)
        } else {
            let mut blocked_request = wasm_request.clone();
            blocked_request.input_summary = "blocked before Wasm dispatch".to_string();
            run_fake_wasm_tool_execution(wasm_profile, &blocked_request, context)
        };
    CrossLayerFixtureReceipt {
        nickel_profile_validated: true,
        steel_route_receipt,
        wasm_execution_receipt,
    }
}

pub fn run_fake_steel_orchestration(
    profile: &FakeSteelOrchestrationProfile,
    request: &FakeSteelOrchestrationRequest,
    context: &DynamicRuntimeAuthorizationContext,
) -> FakeSteelOrchestrationReceipt {
    let input_hash = ArtifactHash::digest(request.input_summary.as_bytes());
    let selected_action = DynamicRuntimeActionEnvelope {
        schema: DYNAMIC_RUNTIME_ACTION_SCHEMA.to_string(),
        action_id: format!("steel:{}:{}", route_slug(&request.script_id), route_slug(&request.route_hint)),
        runtime: DynamicRuntimeKind::SteelScheme,
        runtime_profile: profile.runtime_profile.clone(),
        action_kind: DynamicRuntimeActionKind::HostFunction,
        action_name: request.requested_host_function.clone(),
        target_resource: request.target_resource.clone(),
        receipt_destination: format!(
            "{}/{}.json",
            profile.receipt_prefix.trim_end_matches('/'),
            route_slug(&request.route_hint)
        ),
        required_ucan_ability: profile.default_ucan_ability.clone(),
        required_session_capabilities: profile.required_session_capabilities.clone(),
        input_hash,
        input_bytes: request.input_summary.len() as u64,
        redaction: DynamicRuntimeRedactionClass::MetadataOnly,
    };
    let authorization_receipt = if profile.allowed_host_functions.contains(&request.requested_host_function) {
        authorize_dynamic_runtime_action(&selected_action, context)
    } else {
        dynamic_runtime_receipt(
            &selected_action,
            DynamicRuntimeActionStatus::PolicyDenied,
            DynamicRuntimeActionReason::UnsupportedAction,
            "Steel profile did not expose the requested host function".to_string(),
        )
    };
    FakeSteelOrchestrationReceipt {
        selected_action,
        authorization_receipt,
    }
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

fn route_slug(value: &str) -> String {
    let slug: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        "action".to_string()
    } else {
        trimmed.to_string()
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
    fn fake_steel_profile() -> FakeSteelOrchestrationProfile {
        FakeSteelOrchestrationProfile {
            runtime_profile: "steel-orchestrator/default".to_string(),
            allowed_host_functions: BTreeSet::from(["steel.host.propose_mutation".to_string()]),
            required_session_capabilities: vec!["workspace_mutation".to_string(), "steel_host_functions".to_string()],
            default_ucan_ability: "clankers/mutation.propose".to_string(),
            receipt_prefix: "target/polyglot-agent/steel".to_string(),
        }
    }

    fn fake_steel_request() -> FakeSteelOrchestrationRequest {
        FakeSteelOrchestrationRequest {
            script_id: "route-prompt".to_string(),
            route_hint: "propose system prompt".to_string(),
            target_resource: "prompt:system".to_string(),
            requested_host_function: "steel.host.propose_mutation".to_string(),
            input_summary: "route to prompt mutation host function".to_string(),
        }
    }

    #[test]
    fn fake_steel_orchestration_selects_typed_action_without_host_authority() {
        let receipt = run_fake_steel_orchestration(&fake_steel_profile(), &fake_steel_request(), &context());

        assert_eq!(receipt.selected_action.runtime, DynamicRuntimeKind::SteelScheme);
        assert_eq!(receipt.selected_action.action_kind, DynamicRuntimeActionKind::HostFunction);
        assert_eq!(receipt.selected_action.action_name, "steel.host.propose_mutation");
        assert_eq!(receipt.selected_action.target_resource, "prompt:system");
        assert_eq!(receipt.authorization_receipt.status, DynamicRuntimeActionStatus::Allowed);
        assert_eq!(receipt.authorization_receipt.reason, DynamicRuntimeActionReason::Ready);
        assert!(!receipt.authorization_receipt.writes_performed);
        let serialized = serde_json::to_string(&receipt).expect("fake steel receipt json");
        assert!(!serialized.contains("route to prompt mutation host function"));
        assert!(!serialized.contains("Bearer"));
    }

    #[test]
    fn fake_steel_script_change_can_route_but_not_add_host_function() {
        let mut changed_script = fake_steel_request();
        changed_script.route_hint = "try raw write".to_string();
        changed_script.requested_host_function = "steel.host.raw_write".to_string();

        let receipt = run_fake_steel_orchestration(&fake_steel_profile(), &changed_script, &context());

        assert_eq!(receipt.selected_action.action_name, "steel.host.raw_write");
        assert_eq!(receipt.authorization_receipt.status, DynamicRuntimeActionStatus::PolicyDenied);
        assert_eq!(receipt.authorization_receipt.reason, DynamicRuntimeActionReason::UnsupportedAction);
        assert!(!receipt.authorization_receipt.writes_performed);
    }

    #[test]
    fn fake_steel_profile_cannot_bypass_session_or_ucan_gates() {
        let mut missing_ucan = context();
        missing_ucan.granted_ucan_abilities.clear();
        let denied_by_ucan = run_fake_steel_orchestration(&fake_steel_profile(), &fake_steel_request(), &missing_ucan);
        assert_eq!(denied_by_ucan.authorization_receipt.status, DynamicRuntimeActionStatus::UcanDenied);
        assert_eq!(denied_by_ucan.authorization_receipt.reason, DynamicRuntimeActionReason::MissingUcanAbility);

        let mut missing_capability = context();
        missing_capability.session_capabilities.remove("steel_host_functions");
        let denied_by_session =
            run_fake_steel_orchestration(&fake_steel_profile(), &fake_steel_request(), &missing_capability);
        assert_eq!(denied_by_session.authorization_receipt.status, DynamicRuntimeActionStatus::PolicyDenied);
        assert_eq!(
            denied_by_session.authorization_receipt.reason,
            DynamicRuntimeActionReason::MissingSessionCapability
        );
    }

    #[test]
    fn hot_reloaded_steel_script_cannot_enlarge_profile_budget() {
        let mut oversized = fake_steel_request();
        oversized.script_id = "hot-reload-budget".to_string();
        oversized.route_hint = "oversized prompt analysis".to_string();
        oversized.input_summary = "x".repeat(2048);

        let receipt = run_fake_steel_orchestration(&fake_steel_profile(), &oversized, &context());

        assert_eq!(receipt.selected_action.input_bytes, 2048);
        assert_eq!(receipt.authorization_receipt.status, DynamicRuntimeActionStatus::PolicyDenied);
        assert_eq!(receipt.authorization_receipt.reason, DynamicRuntimeActionReason::InputTooLarge);
        assert!(!receipt.authorization_receipt.writes_performed);
    }

    #[test]
    fn hot_reloaded_steel_profile_cannot_gain_new_capabilities_without_session_policy() {
        let mut expanded_profile = fake_steel_profile();
        expanded_profile.required_session_capabilities.push("provider_credentials".to_string());
        expanded_profile.default_ucan_ability = "clankers/provider.call".to_string();
        expanded_profile.allowed_host_functions.insert("steel.host.provider_call".to_string());

        let mut request = fake_steel_request();
        request.script_id = "hot-reload-capability".to_string();
        request.route_hint = "provider call".to_string();
        request.target_resource = "provider:anthropic".to_string();
        request.requested_host_function = "steel.host.provider_call".to_string();

        let receipt = run_fake_steel_orchestration(&expanded_profile, &request, &context());

        assert_eq!(receipt.selected_action.action_name, "steel.host.provider_call");
        assert_eq!(receipt.authorization_receipt.status, DynamicRuntimeActionStatus::PolicyDenied);
        assert_eq!(receipt.authorization_receipt.reason, DynamicRuntimeActionReason::UnsupportedAction);
        assert!(!receipt.authorization_receipt.writes_performed);
    }

    #[test]
    fn hot_reloaded_steel_profile_cannot_gain_ucan_backed_capability_without_session_grant() {
        let mut expanded_context = context();
        expanded_context.allowed_actions.insert("host_function:steel.host.provider_call".to_string());
        expanded_context.granted_ucan_abilities.insert("clankers/provider.call".to_string());

        let mut expanded_profile = fake_steel_profile();
        expanded_profile.required_session_capabilities.push("provider_credentials".to_string());
        expanded_profile.default_ucan_ability = "clankers/provider.call".to_string();
        expanded_profile.allowed_host_functions.insert("steel.host.provider_call".to_string());

        let mut request = fake_steel_request();
        request.route_hint = "provider call".to_string();
        request.target_resource = "provider:anthropic".to_string();
        request.requested_host_function = "steel.host.provider_call".to_string();

        let receipt = run_fake_steel_orchestration(&expanded_profile, &request, &expanded_context);

        assert_eq!(receipt.authorization_receipt.status, DynamicRuntimeActionStatus::PolicyDenied);
        assert_eq!(receipt.authorization_receipt.reason, DynamicRuntimeActionReason::MissingSessionCapability);
        assert!(!receipt.authorization_receipt.writes_performed);
    }

    fn wasm_profile() -> WasmToolExecutionProfile {
        WasmToolExecutionProfile {
            runtime_profile: "wasm-tool/default".to_string(),
            allowed_imports: BTreeSet::from(["clock.monotonic".to_string(), "input.json".to_string()]),
            required_session_capabilities: vec!["wasm_tool_execute".to_string()],
            required_ucan_ability: "clankers/wasm/tool.execute".to_string(),
            max_memory_pages: 8,
            max_fuel: 10_000,
            max_time_ms: 250,
            input_schema: "schema:tool-input/v1".to_string(),
            output_schema: "schema:tool-output/v1".to_string(),
            receipt_prefix: "target/polyglot-agent/wasm".to_string(),
        }
    }

    fn wasm_request() -> WasmToolExecutionRequest {
        WasmToolExecutionRequest {
            tool_name: "wasm.safe_tool".to_string(),
            target_resource: "tool:wasm.safe_tool".to_string(),
            required_imports: vec!["input.json".to_string()],
            input_summary: "bounded generated-code input".to_string(),
            requested_memory_pages: 4,
            requested_fuel: 5_000,
            requested_time_ms: 100,
            input_schema: "schema:tool-input/v1".to_string(),
        }
    }

    fn wasm_context() -> DynamicRuntimeAuthorizationContext {
        DynamicRuntimeAuthorizationContext {
            allowed_runtime_profiles: BTreeSet::from(["wasm-tool/default".to_string()]),
            allowed_actions: BTreeSet::from(["tool:wasm.safe_tool".to_string()]),
            granted_ucan_abilities: BTreeSet::from(["clankers/wasm/tool.execute".to_string()]),
            session_capabilities: BTreeSet::from(["wasm_tool_execute".to_string()]),
            disabled_actions: BTreeSet::new(),
            max_input_bytes: 1024,
        }
    }

    #[test]
    fn fake_wasm_tool_executes_with_explicit_imports_and_budgets() {
        let receipt = run_fake_wasm_tool_execution(&wasm_profile(), &wasm_request(), &wasm_context());

        assert_eq!(receipt.status, WasmToolExecutionStatus::Completed);
        assert_eq!(receipt.authorization_receipt.status, DynamicRuntimeActionStatus::Allowed);
        assert_eq!(receipt.used_imports, vec!["input.json"]);
        assert_eq!(receipt.memory_pages, 4);
        assert_eq!(receipt.fuel, 5_000);
        assert_eq!(receipt.time_ms, 100);
        assert!(receipt.output_hash.is_some());
        assert!(!receipt.authorization_receipt.writes_performed);
    }

    #[test]
    fn fake_wasm_negative_fixtures_fail_closed_before_execution() {
        let profile = wasm_profile();
        let context = wasm_context();

        let mut missing_import = wasm_request();
        missing_import.required_imports.push("network.tcp".to_string());
        let missing_import_receipt = run_fake_wasm_tool_execution(&profile, &missing_import, &context);
        assert_eq!(missing_import_receipt.status, WasmToolExecutionStatus::Blocked);
        assert_eq!(missing_import_receipt.authorization_receipt.reason, DynamicRuntimeActionReason::UnsupportedAction);
        assert!(missing_import_receipt.output_hash.is_none());

        let mut over_budget = wasm_request();
        over_budget.requested_fuel = profile.max_fuel + 1;
        let over_budget_receipt = run_fake_wasm_tool_execution(&profile, &over_budget, &context);
        assert_eq!(over_budget_receipt.status, WasmToolExecutionStatus::Blocked);
        assert_eq!(over_budget_receipt.authorization_receipt.reason, DynamicRuntimeActionReason::InputTooLarge);

        let mut malformed = wasm_request();
        malformed.input_schema = "schema:wrong".to_string();
        let malformed_receipt = run_fake_wasm_tool_execution(&profile, &malformed, &context);
        assert_eq!(malformed_receipt.authorization_receipt.status, DynamicRuntimeActionStatus::InvalidEnvelope);
        assert_eq!(malformed_receipt.authorization_receipt.reason, DynamicRuntimeActionReason::MissingRequiredField);

        let mut missing_capability = context.clone();
        missing_capability.session_capabilities.clear();
        let missing_capability_receipt = run_fake_wasm_tool_execution(&profile, &wasm_request(), &missing_capability);
        assert_eq!(missing_capability_receipt.status, WasmToolExecutionStatus::Blocked);
        assert_eq!(
            missing_capability_receipt.authorization_receipt.reason,
            DynamicRuntimeActionReason::MissingSessionCapability
        );
    }

    #[test]
    fn cross_layer_fixture_routes_steel_to_wasm_with_redacted_receipts() {
        let mut steel_profile = fake_steel_profile();
        steel_profile.allowed_host_functions.insert("steel.host.dispatch_wasm_tool".to_string());
        steel_profile.default_ucan_ability = "clankers/wasm/tool.execute".to_string();
        steel_profile.required_session_capabilities = vec!["wasm_tool_execute".to_string()];

        let steel_request = FakeSteelOrchestrationRequest {
            script_id: "steel-to-wasm".to_string(),
            route_hint: "dispatch wasm".to_string(),
            target_resource: "tool:wasm.safe_tool".to_string(),
            requested_host_function: "steel.host.dispatch_wasm_tool".to_string(),
            input_summary: "choose generated code tool with private prompt".to_string(),
        };
        let mut context = wasm_context();
        context.allowed_runtime_profiles.insert("steel-orchestrator/default".to_string());
        context.allowed_actions.insert("host_function:steel.host.dispatch_wasm_tool".to_string());

        let receipt =
            run_cross_layer_fixture(&steel_profile, &steel_request, &wasm_profile(), &wasm_request(), &context);

        assert!(receipt.nickel_profile_validated);
        assert_eq!(receipt.steel_route_receipt.authorization_receipt.status, DynamicRuntimeActionStatus::Allowed);
        assert_eq!(receipt.wasm_execution_receipt.status, WasmToolExecutionStatus::Completed);
        let serialized = serde_json::to_string(&receipt).expect("cross-layer receipt json");
        assert!(!serialized.contains("private prompt"));
        assert!(!serialized.contains("bounded generated-code input"));
        assert!(!serialized.contains("Bearer"));
        assert!(!serialized.contains("token"));
    }

    #[test]
    fn cross_layer_policy_and_ucan_denials_fail_closed() {
        let profile = wasm_profile();
        let request = wasm_request();

        let mut no_ucan = wasm_context();
        no_ucan.granted_ucan_abilities.clear();
        let no_ucan_receipt = run_fake_wasm_tool_execution(&profile, &request, &no_ucan);
        assert_eq!(no_ucan_receipt.authorization_receipt.status, DynamicRuntimeActionStatus::UcanDenied);
        assert_eq!(no_ucan_receipt.authorization_receipt.reason, DynamicRuntimeActionReason::MissingUcanAbility);
        assert_eq!(no_ucan_receipt.status, WasmToolExecutionStatus::Blocked);

        let mut ucan_but_no_policy = wasm_context();
        ucan_but_no_policy.allowed_actions.clear();
        let policy_receipt = run_fake_wasm_tool_execution(&profile, &request, &ucan_but_no_policy);
        assert_eq!(policy_receipt.authorization_receipt.status, DynamicRuntimeActionStatus::PolicyDenied);
        assert_eq!(policy_receipt.authorization_receipt.reason, DynamicRuntimeActionReason::UnsupportedAction);
        assert_eq!(policy_receipt.status, WasmToolExecutionStatus::Blocked);
    }

    #[test]
    fn steel_ambient_access_matrix_fails_before_host_effects() {
        let profile = fake_steel_profile();
        let context = context();
        let denied = steel_ambient_access_negative_fixtures();

        assert_eq!(denied.len(), SteelAmbientAccessKind::all().len());
        for request in denied {
            let receipt = run_fake_steel_orchestration(&profile, &request, &context);
            assert_eq!(
                receipt.authorization_receipt.status,
                DynamicRuntimeActionStatus::PolicyDenied,
                "{} should be policy denied",
                receipt.selected_action.action_name
            );
            assert_eq!(receipt.authorization_receipt.reason, DynamicRuntimeActionReason::UnsupportedAction);
            assert!(!receipt.authorization_receipt.writes_performed);
            assert!(!profile.allowed_host_functions.contains(&receipt.selected_action.action_name));
            assert!(receipt.selected_action.target_resource.contains(':'));
        }
    }

    #[test]
    fn steel_ambient_access_matrix_does_not_leak_raw_attempts() {
        let profile = fake_steel_profile();
        let context = context();

        for request in steel_ambient_access_negative_fixtures() {
            let raw_summary = request.input_summary.clone();
            let receipt = run_fake_steel_orchestration(&profile, &request, &context);
            let serialized = serde_json::to_string(&receipt).expect("ambient receipt json");
            assert!(!serialized.contains(&raw_summary));
            assert!(!serialized.contains("Bearer"));
            assert!(!serialized.contains("password"));
            assert!(!serialized.contains("token"));
        }
    }
}
