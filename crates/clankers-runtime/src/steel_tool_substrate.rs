//! Steel-mediated tool/plugin/subagent invocation substrate DTOs.
//!
//! Steel proposes typed invocation plans through the constrained runtime wrapper.
//! Rust remains the authority boundary and the executor for every effect.

use std::collections::BTreeSet;

use clankers_artifacts::ArtifactHash;
use serde::Deserialize;
use serde::Serialize;

use crate::steel_runtime::SteelHostFunctionRegistration;
use crate::steel_runtime::SteelRuntimeProfile;
use crate::steel_runtime::SteelRuntimeReasonCode;
use crate::steel_runtime::SteelRuntimeRequest;
use crate::steel_runtime::SteelRuntimeStatusCode;
use crate::steel_runtime::evaluate_steel_request;

pub const STEEL_TOOL_SUBSTRATE_PLAN_SCHEMA: &str = "clankers.steel_tool_substrate.plan.v1";
pub const STEEL_TOOL_SUBSTRATE_RECEIPT_SCHEMA: &str = "clankers.steel_tool_substrate.receipt.v1";
pub const DEFAULT_TOOL_SUBSTRATE_CALL_SEAM: &str = "steel.host.tool.call";
pub const DEFAULT_TOOL_SUBSTRATE_LIST_SEAM: &str = "steel.host.tool.list";
const DEFAULT_RECEIPT_PREFIX: &str = "target/steel-tool-plugin-substrate";
const DEFAULT_REQUIRED_CAPABILITY: &str = "steel-tool-substrate";
const DEFAULT_REQUIRED_UCAN_ABILITY: &str = "clankers/steel/tool.call";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelToolExecutorKind {
    RustBuiltin,
    WasmPlugin,
    StdioPlugin,
    Subagent,
}

impl SteelToolExecutorKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RustBuiltin => "rust_builtin",
            Self::WasmPlugin => "wasm_plugin",
            Self::StdioPlugin => "stdio_plugin",
            Self::Subagent => "subagent",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "rust_builtin" => Some(Self::RustBuiltin),
            "wasm_plugin" => Some(Self::WasmPlugin),
            "stdio_plugin" => Some(Self::StdioPlugin),
            "subagent" => Some(Self::Subagent),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelToolSubstrateRolloutStage {
    Disabled,
    Comparison,
    Default,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelToolSubstrateFallbackMode {
    RustNative,
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelToolSubstrateProfile {
    pub name: String,
    pub enabled: bool,
    pub rollout_stage: SteelToolSubstrateRolloutStage,
    pub fallback_mode: SteelToolSubstrateFallbackMode,
    pub runtime_profile: SteelRuntimeProfile,
    pub allowed_host_actions: BTreeSet<String>,
    pub allowed_executor_kinds: BTreeSet<SteelToolExecutorKind>,
    pub required_session_capabilities: Vec<String>,
    pub required_ucan_ability: String,
    pub receipt_prefix: String,
    pub max_input_bytes: u64,
}

impl SteelToolSubstrateProfile {
    #[must_use]
    pub fn default_enabled() -> Self {
        let mut allowed_host_actions = BTreeSet::new();
        allowed_host_actions.insert(DEFAULT_TOOL_SUBSTRATE_CALL_SEAM.to_string());
        allowed_host_actions.insert(DEFAULT_TOOL_SUBSTRATE_LIST_SEAM.to_string());
        let allowed_executor_kinds = BTreeSet::from([
            SteelToolExecutorKind::RustBuiltin,
            SteelToolExecutorKind::WasmPlugin,
            SteelToolExecutorKind::StdioPlugin,
            SteelToolExecutorKind::Subagent,
        ]);
        Self {
            name: "steel-tool-plugin-substrate-default".to_string(),
            enabled: true,
            rollout_stage: SteelToolSubstrateRolloutStage::Default,
            fallback_mode: SteelToolSubstrateFallbackMode::RustNative,
            runtime_profile: SteelRuntimeProfile::default_deny(),
            allowed_host_actions,
            allowed_executor_kinds,
            required_session_capabilities: vec![DEFAULT_REQUIRED_CAPABILITY.to_string(), "tool-dispatch".to_string()],
            required_ucan_ability: DEFAULT_REQUIRED_UCAN_ABILITY.to_string(),
            receipt_prefix: DEFAULT_RECEIPT_PREFIX.to_string(),
            max_input_bytes: 200_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelToolInvocationInput {
    pub call_id: String,
    pub tool_name: String,
    pub source_label: String,
    pub executor_kind: SteelToolExecutorKind,
    pub input_hash: ArtifactHash,
    pub input_bytes: u64,
    pub steel_source: String,
    pub steel_plan_payload: String,
    pub session_capabilities: Vec<String>,
    pub disabled_tools: Vec<String>,
    pub granted_ucan_abilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelToolInvocationPlan {
    pub schema: String,
    pub call_id: String,
    pub tool_name: String,
    pub source_label: String,
    pub executor_kind: SteelToolExecutorKind,
    pub input_hash: ArtifactHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelToolSubstrateStatus {
    Authorized,
    FallbackUsed,
    Blocked,
    Denied,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SteelToolSubstrateIssue {
    Ok,
    Disabled,
    ComparisonMode,
    ExecutorKindDenied,
    ToolDisabled,
    InputTooLarge,
    MissingSessionCapability,
    MissingUcanAbility,
    RuntimeFailed,
    MalformedPlan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelToolInvocationReceipt {
    pub schema: String,
    pub status: SteelToolSubstrateStatus,
    pub issue: SteelToolSubstrateIssue,
    pub safe_message: String,
    pub rollout_stage: SteelToolSubstrateRolloutStage,
    pub fallback_mode: SteelToolSubstrateFallbackMode,
    pub executor_kind: SteelToolExecutorKind,
    pub call_id: String,
    pub tool_name: String,
    pub source_label: String,
    pub profile_name: String,
    pub input_hash: ArtifactHash,
    pub output_hash: Option<ArtifactHash>,
    pub steel_runtime_receipt_hash: Option<ArtifactHash>,
    pub plan_hash: Option<ArtifactHash>,
    pub receipt_hash: ArtifactHash,
}

#[must_use]
pub fn steel_tool_plan_payload(input: &SteelToolInvocationInput) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}",
        STEEL_TOOL_SUBSTRATE_PLAN_SCHEMA,
        input.call_id,
        input.tool_name,
        input.source_label,
        input.executor_kind.as_str(),
        input.input_hash.prefixed(),
    )
}

#[must_use]
pub fn plan_tool_invocation_with_steel_or_fallback(
    profile: &SteelToolSubstrateProfile,
    input: &SteelToolInvocationInput,
) -> SteelToolInvocationReceipt {
    if !profile.enabled || profile.rollout_stage == SteelToolSubstrateRolloutStage::Disabled {
        return receipt(ReceiptInput {
            profile,
            input,
            status: SteelToolSubstrateStatus::FallbackUsed,
            issue: SteelToolSubstrateIssue::Disabled,
            message: "Steel tool substrate disabled; Rust-native direct dispatch selected",
            steel_runtime_receipt_hash: None,
            plan: None,
        });
    }
    if profile.rollout_stage == SteelToolSubstrateRolloutStage::Comparison {
        return receipt(ReceiptInput {
            profile,
            input,
            status: SteelToolSubstrateStatus::FallbackUsed,
            issue: SteelToolSubstrateIssue::ComparisonMode,
            message: "Steel tool substrate comparison receipt recorded; Rust-native direct dispatch selected",
            steel_runtime_receipt_hash: None,
            plan: None,
        });
    }
    if !profile.allowed_executor_kinds.contains(&input.executor_kind) {
        return fallback_or_block(profile, input, SteelToolSubstrateIssue::ExecutorKindDenied, None, None);
    }
    if input
        .disabled_tools
        .iter()
        .any(|item| item == &input.tool_name || item == input.executor_kind.as_str())
    {
        return fallback_or_block(profile, input, SteelToolSubstrateIssue::ToolDisabled, None, None);
    }
    if input.input_bytes > profile.max_input_bytes {
        return fallback_or_block(profile, input, SteelToolSubstrateIssue::InputTooLarge, None, None);
    }
    if !profile
        .required_session_capabilities
        .iter()
        .all(|required| input.session_capabilities.iter().any(|available| available == required))
    {
        return fallback_or_block(profile, input, SteelToolSubstrateIssue::MissingSessionCapability, None, None);
    }
    if !input.granted_ucan_abilities.iter().any(|ability| ability == &profile.required_ucan_ability) {
        return fallback_or_block(profile, input, SteelToolSubstrateIssue::MissingUcanAbility, None, None);
    }

    let steel_request = SteelRuntimeRequest {
        profile: profile.runtime_profile.clone(),
        source: input.steel_source.clone(),
        session_capabilities: input.session_capabilities.clone(),
        disabled_tools: input.disabled_tools.clone(),
        host_functions: vec![SteelHostFunctionRegistration {
            name: DEFAULT_TOOL_SUBSTRATE_CALL_SEAM.to_string(),
            required_capability: DEFAULT_REQUIRED_CAPABILITY.to_string(),
            output: input.steel_plan_payload.clone(),
        }],
        receipt_destination: format!("{}/steel-runtime.json", profile.receipt_prefix.trim_end_matches('/')),
    };
    let steel_receipt = evaluate_steel_request(&steel_request);
    let steel_runtime_receipt_hash = Some(steel_receipt.receipt_hash());
    if steel_receipt.status != SteelRuntimeStatusCode::Succeeded
        || steel_receipt.reason_code != SteelRuntimeReasonCode::Ok
    {
        return fallback_or_block(
            profile,
            input,
            SteelToolSubstrateIssue::RuntimeFailed,
            steel_runtime_receipt_hash,
            None,
        );
    }
    let Some(output) = steel_receipt.output.as_deref() else {
        return fallback_or_block(
            profile,
            input,
            SteelToolSubstrateIssue::MalformedPlan,
            steel_runtime_receipt_hash,
            None,
        );
    };
    let Some(plan) = parse_plan(output) else {
        return fallback_or_block(
            profile,
            input,
            SteelToolSubstrateIssue::MalformedPlan,
            steel_runtime_receipt_hash,
            None,
        );
    };
    if !plan_matches_input(&plan, input) {
        return fallback_or_block(
            profile,
            input,
            SteelToolSubstrateIssue::MalformedPlan,
            steel_runtime_receipt_hash,
            Some(plan),
        );
    }
    receipt(ReceiptInput {
        profile,
        input,
        status: SteelToolSubstrateStatus::Authorized,
        issue: SteelToolSubstrateIssue::Ok,
        message: "Steel tool substrate authorized typed plan; Rust executor owns host effect",
        steel_runtime_receipt_hash,
        plan: Some(plan),
    })
}

fn fallback_or_block(
    profile: &SteelToolSubstrateProfile,
    input: &SteelToolInvocationInput,
    issue: SteelToolSubstrateIssue,
    steel_runtime_receipt_hash: Option<ArtifactHash>,
    plan: Option<SteelToolInvocationPlan>,
) -> SteelToolInvocationReceipt {
    let status = match profile.fallback_mode {
        SteelToolSubstrateFallbackMode::RustNative
            if profile.rollout_stage != SteelToolSubstrateRolloutStage::Block =>
        {
            SteelToolSubstrateStatus::FallbackUsed
        }
        SteelToolSubstrateFallbackMode::RustNative | SteelToolSubstrateFallbackMode::Block => {
            SteelToolSubstrateStatus::Blocked
        }
    };
    receipt(ReceiptInput {
        profile,
        input,
        status,
        issue,
        message: "Steel tool substrate did not authorize a typed plan before host execution",
        steel_runtime_receipt_hash,
        plan,
    })
}

fn parse_plan(payload: &str) -> Option<SteelToolInvocationPlan> {
    let mut parts = payload.split('|');
    let schema = parts.next()?;
    let call_id = parts.next()?;
    let tool_name = parts.next()?;
    let source_label = parts.next()?;
    let executor_kind = SteelToolExecutorKind::parse(parts.next()?)?;
    let input_hash = parts.next()?.parse::<ArtifactHash>().ok()?;
    if parts.next().is_some() || schema != STEEL_TOOL_SUBSTRATE_PLAN_SCHEMA {
        return None;
    }
    Some(SteelToolInvocationPlan {
        schema: schema.to_string(),
        call_id: call_id.to_string(),
        tool_name: tool_name.to_string(),
        source_label: source_label.to_string(),
        executor_kind,
        input_hash,
    })
}

fn plan_matches_input(plan: &SteelToolInvocationPlan, input: &SteelToolInvocationInput) -> bool {
    plan.call_id == input.call_id
        && plan.tool_name == input.tool_name
        && plan.source_label == input.source_label
        && plan.executor_kind == input.executor_kind
        && plan.input_hash == input.input_hash
}

struct ReceiptInput<'a> {
    profile: &'a SteelToolSubstrateProfile,
    input: &'a SteelToolInvocationInput,
    status: SteelToolSubstrateStatus,
    issue: SteelToolSubstrateIssue,
    message: &'static str,
    steel_runtime_receipt_hash: Option<ArtifactHash>,
    plan: Option<SteelToolInvocationPlan>,
}

fn receipt(input: ReceiptInput<'_>) -> SteelToolInvocationReceipt {
    let plan_hash = input.plan.as_ref().map(|plan| {
        let bytes = serde_json::to_vec(plan).expect("Steel tool plan serializes");
        ArtifactHash::digest(&bytes)
    });
    let output_hash = input
        .plan
        .as_ref()
        .map(|plan| ArtifactHash::digest(format!("{}:{}", plan.call_id, plan.tool_name).as_bytes()));
    let mut receipt = SteelToolInvocationReceipt {
        schema: STEEL_TOOL_SUBSTRATE_RECEIPT_SCHEMA.to_string(),
        status: input.status,
        issue: input.issue,
        safe_message: input.message.to_string(),
        rollout_stage: input.profile.rollout_stage,
        fallback_mode: input.profile.fallback_mode,
        executor_kind: input.input.executor_kind,
        call_id: input.input.call_id.clone(),
        tool_name: input.input.tool_name.clone(),
        source_label: input.input.source_label.clone(),
        profile_name: input.profile.name.clone(),
        input_hash: input.input.input_hash,
        output_hash,
        steel_runtime_receipt_hash: input.steel_runtime_receipt_hash,
        plan_hash,
        receipt_hash: ArtifactHash::digest(b"pending"),
    };
    let bytes = serde_json::to_vec(&receipt).expect("Steel tool receipt serializes");
    receipt.receipt_hash = ArtifactHash::digest(&bytes);
    receipt
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(kind: SteelToolExecutorKind) -> SteelToolInvocationInput {
        let input_hash = ArtifactHash::digest(br#"{"path":"README.md"}"#);
        let mut item = SteelToolInvocationInput {
            call_id: "call-1".to_string(),
            tool_name: "read".to_string(),
            source_label: "built-in".to_string(),
            executor_kind: kind,
            input_hash,
            input_bytes: 20,
            steel_source: format!("(host \"{DEFAULT_TOOL_SUBSTRATE_CALL_SEAM}\")"),
            steel_plan_payload: String::new(),
            session_capabilities: vec![DEFAULT_REQUIRED_CAPABILITY.to_string(), "tool-dispatch".to_string()],
            disabled_tools: Vec::new(),
            granted_ucan_abilities: vec![DEFAULT_REQUIRED_UCAN_ABILITY.to_string()],
        };
        item.steel_plan_payload = steel_tool_plan_payload(&item);
        item
    }

    #[test]
    fn authorizes_matching_typed_plan() {
        let receipt = plan_tool_invocation_with_steel_or_fallback(
            &SteelToolSubstrateProfile::default_enabled(),
            &input(SteelToolExecutorKind::RustBuiltin),
        );
        assert_eq!(receipt.status, SteelToolSubstrateStatus::Authorized);
        assert_eq!(receipt.executor_kind, SteelToolExecutorKind::RustBuiltin);
        assert!(receipt.steel_runtime_receipt_hash.is_some());
        assert!(receipt.plan_hash.is_some());
    }

    #[test]
    fn mismatched_plan_falls_back_without_authorization() {
        let mut request = input(SteelToolExecutorKind::RustBuiltin);
        request.steel_plan_payload = request.steel_plan_payload.replace("read", "write");
        let receipt =
            plan_tool_invocation_with_steel_or_fallback(&SteelToolSubstrateProfile::default_enabled(), &request);
        assert_eq!(receipt.status, SteelToolSubstrateStatus::FallbackUsed);
        assert_eq!(receipt.issue, SteelToolSubstrateIssue::MalformedPlan);
    }

    #[test]
    fn block_mode_blocks_denied_executor() {
        let mut profile = SteelToolSubstrateProfile::default_enabled();
        profile.fallback_mode = SteelToolSubstrateFallbackMode::Block;
        profile.allowed_executor_kinds.remove(&SteelToolExecutorKind::Subagent);
        let receipt = plan_tool_invocation_with_steel_or_fallback(&profile, &input(SteelToolExecutorKind::Subagent));
        assert_eq!(receipt.status, SteelToolSubstrateStatus::Blocked);
        assert_eq!(receipt.issue, SteelToolSubstrateIssue::ExecutorKindDenied);
    }
}
