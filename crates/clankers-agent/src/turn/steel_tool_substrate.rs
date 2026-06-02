use clanker_message::Content;
use clanker_message::MessageId;
use clanker_message::ToolResultMessage;
use clankers_artifacts::ArtifactHash;
use clankers_runtime::DEFAULT_TOOL_SUBSTRATE_CALL_SEAM;
use clankers_runtime::SteelToolExecutorKind;
use clankers_runtime::SteelToolInvocationInput;
use clankers_runtime::SteelToolInvocationReceipt;
use clankers_runtime::SteelToolSubstrateFallbackMode;
use clankers_runtime::SteelToolSubstrateProfile;
use clankers_runtime::SteelToolSubstrateRolloutStage;
use clankers_runtime::SteelToolSubstrateStatus;
use clankers_runtime::plan_tool_invocation_with_steel_or_fallback;
use clankers_runtime::steel_tool_plan_payload;
use serde_json::Value;

use crate::events::AgentEvent;
use crate::tool::Tool;
use crate::tool::ToolExecutionBackend;

const DEFAULT_STEEL_SOURCE: &str = "(host \"steel.host.tool.call\")";

#[derive(Debug, Clone)]
pub struct AgentToolSteelSubstrateConfig {
    pub profile: SteelToolSubstrateProfile,
    pub steel_source: String,
    pub session_capabilities: Vec<String>,
    pub granted_ucan_abilities: Vec<String>,
    pub disabled_actions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentToolSteelSubstrateRolloutStage {
    Disabled,
    Comparison,
    Default,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentToolSteelSubstrateFallbackMode {
    RustNative,
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentToolSteelSubstrateSettings {
    pub enabled: bool,
    pub rollout_stage: Option<AgentToolSteelSubstrateRolloutStage>,
    pub fallback_mode: Option<AgentToolSteelSubstrateFallbackMode>,
    pub session_capabilities: Vec<String>,
    pub granted_ucan_abilities: Vec<String>,
    pub disabled_executors: Vec<String>,
    pub disabled_actions: Vec<String>,
    pub receipt_prefix: Option<String>,
    pub max_input_bytes: Option<u64>,
    pub max_source_bytes: u64,
}

impl Default for AgentToolSteelSubstrateSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            rollout_stage: Some(AgentToolSteelSubstrateRolloutStage::Default),
            fallback_mode: Some(AgentToolSteelSubstrateFallbackMode::RustNative),
            session_capabilities: vec!["steel-tool-substrate".to_string(), "tool-dispatch".to_string()],
            granted_ucan_abilities: vec!["clankers/steel/tool.call".to_string()],
            disabled_executors: Vec::new(),
            disabled_actions: Vec::new(),
            receipt_prefix: None,
            max_input_bytes: Some(200_000),
            max_source_bytes: 4096,
        }
    }
}

impl AgentToolSteelSubstrateSettings {
    fn validate(&self) -> Result<(), SteelToolSubstrateSettingsError> {
        if !self.enabled {
            return Ok(());
        }
        if self.session_capabilities.iter().any(|capability| capability.trim().is_empty()) {
            return Err(SteelToolSubstrateSettingsError::BlankCapability);
        }
        if self.granted_ucan_abilities.iter().any(|ability| ability.trim().is_empty()) {
            return Err(SteelToolSubstrateSettingsError::BlankUcanAbility);
        }
        if self.disabled_executors.iter().any(|executor| executor.trim().is_empty()) {
            return Err(SteelToolSubstrateSettingsError::BlankDisabledExecutor);
        }
        if self.disabled_actions.iter().any(|action| action.trim().is_empty()) {
            return Err(SteelToolSubstrateSettingsError::BlankDisabledAction);
        }
        if matches!(self.max_input_bytes, Some(0)) {
            return Err(SteelToolSubstrateSettingsError::NonPositiveMaxInputBytes);
        }
        if self.max_source_bytes == 0 {
            return Err(SteelToolSubstrateSettingsError::NonPositiveMaxSourceBytes);
        }
        if let Some(prefix) = &self.receipt_prefix
            && !prefix.starts_with("target/")
        {
            return Err(SteelToolSubstrateSettingsError::ReceiptOutsideTarget);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SteelToolSubstrateSettingsError {
    BlankCapability,
    BlankUcanAbility,
    BlankDisabledExecutor,
    BlankDisabledAction,
    NonPositiveMaxInputBytes,
    NonPositiveMaxSourceBytes,
    ReceiptOutsideTarget,
}

impl std::fmt::Display for SteelToolSubstrateSettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BlankCapability => f.write_str("Steel tool substrate session capabilities cannot be blank"),
            Self::BlankUcanAbility => f.write_str("Steel tool substrate UCAN abilities cannot be blank"),
            Self::BlankDisabledExecutor => f.write_str("Steel tool substrate disabled executors cannot be blank"),
            Self::BlankDisabledAction => f.write_str("Steel tool substrate disabled actions cannot be blank"),
            Self::NonPositiveMaxInputBytes => {
                f.write_str("Steel tool substrate `maxInputBytes` must be greater than zero")
            }
            Self::NonPositiveMaxSourceBytes => {
                f.write_str("Steel tool substrate `maxSourceBytes` must be greater than zero")
            }
            Self::ReceiptOutsideTarget => f.write_str("Steel tool substrate `receiptPrefix` must stay under target/"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SteelToolSubstrateActivationError {
    InvalidSettings(String),
    UnknownDisabledExecutor(String),
}

impl std::fmt::Display for SteelToolSubstrateActivationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSettings(message) => write!(f, "invalid Steel tool substrate settings: {message}"),
            Self::UnknownDisabledExecutor(executor) => {
                write!(f, "unknown Steel tool substrate disabled executor `{executor}`")
            }
        }
    }
}

impl std::error::Error for SteelToolSubstrateActivationError {}

pub fn steel_tool_substrate_config_from_settings(
    settings: &AgentToolSteelSubstrateSettings,
) -> Result<Option<AgentToolSteelSubstrateConfig>, SteelToolSubstrateActivationError> {
    if !settings.enabled {
        return Ok(None);
    }
    settings
        .validate()
        .map_err(|error| SteelToolSubstrateActivationError::InvalidSettings(error.to_string()))?;

    let mut profile = SteelToolSubstrateProfile::default_enabled();
    profile.rollout_stage = settings.rollout_stage.map(rollout_stage).unwrap_or(profile.rollout_stage);
    profile.fallback_mode = settings.fallback_mode.map(fallback_mode).unwrap_or(profile.fallback_mode);
    profile.required_session_capabilities = settings.session_capabilities.clone();
    profile.required_ucan_ability = settings
        .granted_ucan_abilities
        .first()
        .cloned()
        .unwrap_or_else(|| profile.required_ucan_ability.clone());
    profile.receipt_prefix = settings.receipt_prefix.clone().unwrap_or_else(|| profile.receipt_prefix.clone());
    profile.max_input_bytes = settings.max_input_bytes.unwrap_or(profile.max_input_bytes);
    profile.runtime_profile.max_source_bytes = settings.max_source_bytes;
    for executor in &settings.disabled_executors {
        let kind = SteelToolExecutorKind::parse(executor)
            .ok_or_else(|| SteelToolSubstrateActivationError::UnknownDisabledExecutor(executor.to_string()))?;
        profile.allowed_executor_kinds.remove(&kind);
    }

    Ok(Some(AgentToolSteelSubstrateConfig {
        profile,
        steel_source: DEFAULT_STEEL_SOURCE.to_string(),
        session_capabilities: settings.session_capabilities.clone(),
        granted_ucan_abilities: settings.granted_ucan_abilities.clone(),
        disabled_actions: settings.disabled_actions.clone(),
    }))
}

fn rollout_stage(stage: AgentToolSteelSubstrateRolloutStage) -> SteelToolSubstrateRolloutStage {
    match stage {
        AgentToolSteelSubstrateRolloutStage::Disabled => SteelToolSubstrateRolloutStage::Disabled,
        AgentToolSteelSubstrateRolloutStage::Comparison => SteelToolSubstrateRolloutStage::Comparison,
        AgentToolSteelSubstrateRolloutStage::Default => SteelToolSubstrateRolloutStage::Default,
        AgentToolSteelSubstrateRolloutStage::Block => SteelToolSubstrateRolloutStage::Block,
    }
}

fn fallback_mode(mode: AgentToolSteelSubstrateFallbackMode) -> SteelToolSubstrateFallbackMode {
    match mode {
        AgentToolSteelSubstrateFallbackMode::RustNative => SteelToolSubstrateFallbackMode::RustNative,
        AgentToolSteelSubstrateFallbackMode::Block => SteelToolSubstrateFallbackMode::Block,
    }
}

pub(crate) fn authorize_tool_invocation(
    config: Option<&AgentToolSteelSubstrateConfig>,
    tool: &dyn Tool,
    call_id: &str,
    tool_name: &str,
    input: &Value,
    emit_event: impl Fn(AgentEvent),
) -> Result<Option<SteelToolInvocationReceipt>, SteelToolInvocationReceipt> {
    let Some(config) = config else {
        return Ok(None);
    };
    let input_bytes = serde_json::to_vec(input).unwrap_or_else(|_| b"serialization-failed".to_vec());
    let input_hash = ArtifactHash::digest(&input_bytes);
    let mut request = SteelToolInvocationInput {
        call_id: call_id.to_string(),
        tool_name: tool_name.to_string(),
        source_label: tool.source().to_string(),
        executor_kind: executor_kind(tool.execution_backend()),
        input_hash,
        input_bytes: input_bytes.len() as u64,
        steel_source: config.steel_source.clone(),
        steel_plan_payload: String::new(),
        session_capabilities: config.session_capabilities.clone(),
        disabled_tools: config.disabled_actions.clone(),
        granted_ucan_abilities: config.granted_ucan_abilities.clone(),
    };
    request.steel_plan_payload = steel_tool_plan_payload(&request);
    let receipt = plan_tool_invocation_with_steel_or_fallback(&config.profile, &request);
    emit_steel_tool_substrate_receipt(emit_event, &receipt);
    if receipt.status == SteelToolSubstrateStatus::Blocked || receipt.status == SteelToolSubstrateStatus::Denied {
        return Err(receipt);
    }
    Ok(Some(receipt))
}

fn executor_kind(backend: ToolExecutionBackend) -> SteelToolExecutorKind {
    match backend {
        ToolExecutionBackend::RustBuiltin => SteelToolExecutorKind::RustBuiltin,
        ToolExecutionBackend::WasmPlugin => SteelToolExecutorKind::WasmPlugin,
        ToolExecutionBackend::StdioPlugin => SteelToolExecutorKind::StdioPlugin,
        ToolExecutionBackend::Subagent => SteelToolExecutorKind::Subagent,
    }
}

fn emit_steel_tool_substrate_receipt(emit_event: impl Fn(AgentEvent), receipt: &SteelToolInvocationReceipt) {
    let message = format!(
        "{DEFAULT_TOOL_SUBSTRATE_CALL_SEAM} receipt status={:?} issue={:?} executor={:?} fallback={:?} tool={} receipt_hash={} plan_hash={}",
        receipt.status,
        receipt.issue,
        receipt.executor_kind,
        receipt.fallback_mode,
        receipt.tool_name,
        receipt.receipt_hash.prefixed(),
        receipt.plan_hash.map_or_else(|| "none".to_string(), ArtifactHash::prefixed),
    );
    emit_event(AgentEvent::SystemMessage { message });
}

pub(crate) fn blocked_receipt_to_tool_result(
    call_id: String,
    tool_name: String,
    receipt: SteelToolInvocationReceipt,
) -> ToolResultMessage {
    ToolResultMessage {
        id: MessageId::generate(),
        call_id,
        tool_name,
        content: vec![Content::Text {
            text: format!("Steel tool substrate blocked host execution: {:?} ({:?})", receipt.issue, receipt.status),
        }],
        is_error: true,
        details: Some(serde_json::json!({
            "steel_tool_substrate": {
                "status": receipt.status,
                "issue": receipt.issue,
                "executor_kind": receipt.executor_kind,
                "receipt_hash": receipt.receipt_hash.prefixed(),
            }
        })),
        timestamp: chrono::Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_enable_all_executor_kinds() {
        let config = steel_tool_substrate_config_from_settings(&AgentToolSteelSubstrateSettings::default())
            .expect("settings valid")
            .expect("enabled by default");
        assert!(config.profile.allowed_executor_kinds.contains(&SteelToolExecutorKind::RustBuiltin));
        assert!(config.profile.allowed_executor_kinds.contains(&SteelToolExecutorKind::WasmPlugin));
        assert!(config.profile.allowed_executor_kinds.contains(&SteelToolExecutorKind::StdioPlugin));
        assert!(config.profile.allowed_executor_kinds.contains(&SteelToolExecutorKind::Subagent));
    }

    #[test]
    fn disabled_executor_is_removed_from_profile() {
        let settings = AgentToolSteelSubstrateSettings {
            disabled_executors: vec!["subagent".to_string()],
            ..AgentToolSteelSubstrateSettings::default()
        };
        let config = steel_tool_substrate_config_from_settings(&settings).expect("settings valid").expect("enabled");
        assert!(!config.profile.allowed_executor_kinds.contains(&SteelToolExecutorKind::Subagent));
    }
}
