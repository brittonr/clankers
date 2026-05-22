use std::collections::BTreeSet;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use clankers_artifacts::ArtifactHash;
use clankers_config::SteelTurnPlanningFallbackMode;
use clankers_config::SteelTurnPlanningRolloutStage;
use clankers_config::SteelTurnPlanningSettings;
use clankers_provider::message::AgentMessage;
use clankers_runtime::DEFAULT_TURN_PLANNING_SEAM;
use clankers_runtime::OrchestrationCandidate;
use clankers_runtime::OrchestrationFallbackMode;
use clankers_runtime::OrchestrationPlanReceipt;
use clankers_runtime::OrchestrationPlanStatus;
use clankers_runtime::OrchestrationRolloutStage;
use clankers_runtime::STEEL_ORCHESTRATION_PLAN_SCHEMA;
use clankers_runtime::SteelOrchestrationProfile;
use clankers_runtime::SteelRuntimeProfile;
use clankers_runtime::SteelTurnPlanningAuthorityGrant;
use clankers_runtime::TurnPlanningInput;
use clankers_runtime::plan_turn_with_steel_or_fallback;
use serde::Deserialize;
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
    pub ucan_authority_grants: Vec<SteelTurnPlanningAuthorityGrant>,
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
            ucan_authority_grants: Vec::new(),
            disabled_actions: Vec::new(),
            profile,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SteelTurnPlanningActivationError {
    InvalidSettings(String),
    MissingProfilePath,
    MissingScriptPath,
    ReadProfile { path: PathBuf, message: String },
    ReadScript { path: PathBuf, message: String },
    InvalidProfileJson(String),
    UnsupportedProfileSchema(String),
    UnsupportedSeam(String),
    UnsupportedHostAction(String),
    InvalidScriptHash(String),
    ScriptHashMismatch { expected: String, actual: String },
    InvalidProfileHash(String),
    ProfileHashMismatch { expected: String, actual: String },
    ScriptTooLarge { actual: u64, max: u64 },
    EmptyScript,
    EmptyHash,
    MissingSessionCapability(String),
    MissingUcanAbility(String),
    DisabledRequiredAction(String),
    ReceiptOutsideTarget(String),
}

impl std::fmt::Display for SteelTurnPlanningActivationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSettings(message) => write!(f, "invalid Steel turn planning settings: {message}"),
            Self::MissingProfilePath => f.write_str("Steel turn planning profile path missing"),
            Self::MissingScriptPath => f.write_str("Steel turn planning script path missing"),
            Self::ReadProfile { path, message } => {
                write!(f, "failed to read Steel profile {}: {message}", path.display())
            }
            Self::ReadScript { path, message } => {
                write!(f, "failed to read Steel script {}: {message}", path.display())
            }
            Self::InvalidProfileJson(message) => write!(f, "invalid Steel profile JSON: {message}"),
            Self::UnsupportedProfileSchema(schema) => write!(f, "unsupported Steel profile schema `{schema}`"),
            Self::UnsupportedSeam(seam) => write!(f, "unsupported Steel planning seam `{seam}`"),
            Self::UnsupportedHostAction(action) => write!(f, "unsupported Steel host action `{action}`"),
            Self::InvalidScriptHash(hash) => write!(f, "invalid Steel script hash `{hash}`"),
            Self::ScriptHashMismatch { expected, actual } => {
                write!(f, "Steel script hash mismatch: expected {expected}, got {actual}")
            }
            Self::InvalidProfileHash(hash) => write!(f, "invalid Steel profile hash `{hash}`"),
            Self::ProfileHashMismatch { expected, actual } => {
                write!(f, "Steel profile hash mismatch: expected {expected}, got {actual}")
            }
            Self::ScriptTooLarge { actual, max } => write!(f, "Steel script too large: {actual} bytes exceeds {max}"),
            Self::EmptyScript => f.write_str("Steel script cannot be empty"),
            Self::EmptyHash => f.write_str("Steel profile/script hashes cannot be empty"),
            Self::MissingSessionCapability(capability) => write!(f, "missing Steel session capability `{capability}`"),
            Self::MissingUcanAbility(ability) => write!(f, "missing Steel UCAN ability `{ability}`"),
            Self::DisabledRequiredAction(action) => write!(f, "required Steel host action `{action}` is disabled"),
            Self::ReceiptOutsideTarget(prefix) => write!(f, "Steel receipt prefix `{prefix}` must stay under target/"),
        }
    }
}

impl std::error::Error for SteelTurnPlanningActivationError {}

pub fn steel_turn_planning_config_from_settings(
    settings: &SteelTurnPlanningSettings,
    base_dir: &Path,
) -> Result<Option<AgentTurnSteelPlanningConfig>, SteelTurnPlanningActivationError> {
    if !settings.enabled {
        return Ok(None);
    }
    settings
        .validate()
        .map_err(|error| SteelTurnPlanningActivationError::InvalidSettings(error.to_string()))?;
    let profile_path = resolve_config_path(
        base_dir,
        settings.profile_path.as_deref().ok_or(SteelTurnPlanningActivationError::MissingProfilePath)?,
    );
    let script_path = resolve_config_path(
        base_dir,
        settings.script_path.as_deref().ok_or(SteelTurnPlanningActivationError::MissingScriptPath)?,
    );
    let profile_bytes =
        std::fs::read(&profile_path).map_err(|error| SteelTurnPlanningActivationError::ReadProfile {
            path: profile_path.clone(),
            message: error.to_string(),
        })?;
    let script_source =
        std::fs::read_to_string(&script_path).map_err(|error| SteelTurnPlanningActivationError::ReadScript {
            path: script_path.clone(),
            message: error.to_string(),
        })?;
    if script_source.trim().is_empty() {
        return Err(SteelTurnPlanningActivationError::EmptyScript);
    }
    let script_bytes = script_source.as_bytes();
    if script_bytes.len() as u64 > settings.max_source_bytes {
        return Err(SteelTurnPlanningActivationError::ScriptTooLarge {
            actual: script_bytes.len() as u64,
            max: settings.max_source_bytes,
        });
    }
    let script_hash = verify_optional_hash(settings.script_blake3.as_deref(), script_bytes, HashKind::Script)?;
    let policy_hash = verify_optional_hash(settings.profile_blake3.as_deref(), &profile_bytes, HashKind::Profile)?;
    let profile_export: NickelSteelOrchestrationProfile = serde_json::from_slice(&profile_bytes)
        .map_err(|error| SteelTurnPlanningActivationError::InvalidProfileJson(error.to_string()))?;
    let profile = runtime_profile_from_export(settings, &profile_export, script_hash, policy_hash)?;
    ensure_session_authority(settings, &profile)?;
    Ok(Some(AgentTurnSteelPlanningConfig {
        steel_plan_payload: format!(
            "{STEEL_ORCHESTRATION_PLAN_SCHEMA}|{AGENT_TURN_DECISION_ID}|{DEFAULT_TURN_PLANNING_SEAM}|session:fixture|{AGENT_TURN_DECISION_CLASS}"
        ),
        steel_source: script_source,
        session_capabilities: settings.session_capabilities.clone(),
        granted_ucan_abilities: settings.granted_ucan_abilities.clone(),
        ucan_authority_grants: authority_grants_from_settings(settings),
        disabled_actions: settings.disabled_actions.clone(),
        profile,
    }))
}

#[derive(Debug, Deserialize)]
struct NickelSteelOrchestrationProfile {
    schema: String,
    name: String,
    enabled: bool,
    #[serde(default)]
    default: bool,
    planning_seam: String,
    rollout_stage: OrchestrationRolloutStage,
    fallback_mode: OrchestrationFallbackMode,
    script: NickelSteelScriptBinding,
    runtime_budget: NickelSteelRuntimeBudget,
    allowed_host_actions: Vec<NickelSteelHostAction>,
    receipt_policy: NickelSteelReceiptPolicy,
}

#[derive(Debug, Deserialize)]
struct NickelSteelScriptBinding {
    id: String,
}

#[derive(Debug, Deserialize)]
struct NickelSteelRuntimeBudget {
    max_input_bytes: u64,
}

#[derive(Debug, Deserialize)]
struct NickelSteelHostAction {
    name: String,
    #[serde(default)]
    required_session_capabilities: Vec<String>,
    ucan_ability: String,
}

#[derive(Debug, Deserialize)]
struct NickelSteelReceiptPolicy {
    destination_prefix: String,
}

#[derive(Clone, Copy)]
enum HashKind {
    Script,
    Profile,
}

fn verify_optional_hash(
    expected: Option<&str>,
    bytes: &[u8],
    kind: HashKind,
) -> Result<ArtifactHash, SteelTurnPlanningActivationError> {
    let actual = ArtifactHash::digest(bytes);
    let Some(expected) = expected else {
        return Ok(actual);
    };
    if expected.trim().is_empty() {
        return Err(SteelTurnPlanningActivationError::EmptyHash);
    }
    let parsed = expected.parse::<ArtifactHash>().map_err(|_| match kind {
        HashKind::Script => SteelTurnPlanningActivationError::InvalidScriptHash(expected.to_string()),
        HashKind::Profile => SteelTurnPlanningActivationError::InvalidProfileHash(expected.to_string()),
    })?;
    if parsed != actual {
        return Err(match kind {
            HashKind::Script => SteelTurnPlanningActivationError::ScriptHashMismatch {
                expected: parsed.prefixed(),
                actual: actual.prefixed(),
            },
            HashKind::Profile => SteelTurnPlanningActivationError::ProfileHashMismatch {
                expected: parsed.prefixed(),
                actual: actual.prefixed(),
            },
        });
    }
    Ok(actual)
}

fn runtime_profile_from_export(
    settings: &SteelTurnPlanningSettings,
    export: &NickelSteelOrchestrationProfile,
    script_hash: ArtifactHash,
    policy_hash: ArtifactHash,
) -> Result<SteelOrchestrationProfile, SteelTurnPlanningActivationError> {
    if export.schema != "clankers.steel_default_orchestration.profile.v1" {
        return Err(SteelTurnPlanningActivationError::UnsupportedProfileSchema(export.schema.clone()));
    }
    let seam = settings.planning_seam.as_deref().unwrap_or(&export.planning_seam);
    if seam != DEFAULT_TURN_PLANNING_SEAM {
        return Err(SteelTurnPlanningActivationError::UnsupportedSeam(seam.to_string()));
    }
    let receipt_prefix = settings.receipt_prefix.as_deref().unwrap_or(&export.receipt_policy.destination_prefix);
    if !receipt_prefix.starts_with("target/") {
        return Err(SteelTurnPlanningActivationError::ReceiptOutsideTarget(receipt_prefix.to_string()));
    }
    let allowed_host_actions = allowed_host_actions(&export.allowed_host_actions)?;
    let required_action = export
        .allowed_host_actions
        .iter()
        .find(|action| action.name == DEFAULT_TURN_PLANNING_SEAM)
        .ok_or_else(|| {
        SteelTurnPlanningActivationError::UnsupportedHostAction("missing steel.host.plan_turn".to_string())
    })?;
    Ok(SteelOrchestrationProfile {
        name: export.name.clone(),
        enabled: settings.enabled && export.enabled,
        default: export.default,
        planning_seam: DEFAULT_TURN_PLANNING_SEAM.to_string(),
        rollout_stage: settings.rollout_stage.map_or(export.rollout_stage, rollout_stage_to_runtime),
        fallback_mode: settings.fallback_mode.map_or(export.fallback_mode, fallback_mode_to_runtime),
        script_id: export.script.id.clone(),
        script_hash,
        policy_hash,
        runtime_profile: SteelRuntimeProfile::default_deny(),
        allowed_host_actions,
        required_session_capabilities: required_action.required_session_capabilities.clone(),
        required_ucan_ability: required_action.ucan_ability.clone(),
        receipt_prefix: receipt_prefix.to_string(),
        max_input_bytes: settings.max_input_bytes.unwrap_or(export.runtime_budget.max_input_bytes),
    })
}

fn allowed_host_actions(
    actions: &[NickelSteelHostAction],
) -> Result<BTreeSet<String>, SteelTurnPlanningActivationError> {
    let mut allowed = BTreeSet::new();
    for action in actions {
        if action.name != DEFAULT_TURN_PLANNING_SEAM {
            return Err(SteelTurnPlanningActivationError::UnsupportedHostAction(action.name.clone()));
        }
        allowed.insert(action.name.clone());
    }
    if !allowed.contains(DEFAULT_TURN_PLANNING_SEAM) {
        return Err(SteelTurnPlanningActivationError::UnsupportedHostAction(
            "missing steel.host.plan_turn".to_string(),
        ));
    }
    Ok(allowed)
}

fn ensure_session_authority(
    settings: &SteelTurnPlanningSettings,
    profile: &SteelOrchestrationProfile,
) -> Result<(), SteelTurnPlanningActivationError> {
    for capability in &profile.required_session_capabilities {
        if !settings.session_capabilities.iter().any(|available| available == capability) {
            return Err(SteelTurnPlanningActivationError::MissingSessionCapability(capability.clone()));
        }
    }
    if !settings.granted_ucan_abilities.iter().any(|available| available == &profile.required_ucan_ability)
        && settings.ucan_authority_grants.is_empty()
    {
        return Err(SteelTurnPlanningActivationError::MissingUcanAbility(profile.required_ucan_ability.clone()));
    }
    if settings.disabled_actions.iter().any(|action| action == DEFAULT_TURN_PLANNING_SEAM) {
        return Err(SteelTurnPlanningActivationError::DisabledRequiredAction(DEFAULT_TURN_PLANNING_SEAM.to_string()));
    }
    Ok(())
}

fn authority_grants_from_settings(settings: &SteelTurnPlanningSettings) -> Vec<SteelTurnPlanningAuthorityGrant> {
    settings
        .ucan_authority_grants
        .iter()
        .map(|grant| SteelTurnPlanningAuthorityGrant {
            resource: grant.resource.clone(),
            ability: grant.ability.clone(),
            audience: grant.audience.clone(),
            proof_reference: grant.proof_reference.clone(),
            expires_at: grant.expires_at,
            revoked: grant.revoked,
            caveats: grant.caveats.clone(),
        })
        .collect()
}

fn rollout_stage_to_runtime(stage: SteelTurnPlanningRolloutStage) -> OrchestrationRolloutStage {
    match stage {
        SteelTurnPlanningRolloutStage::Disabled => OrchestrationRolloutStage::Disabled,
        SteelTurnPlanningRolloutStage::Comparison => OrchestrationRolloutStage::Comparison,
        SteelTurnPlanningRolloutStage::Default => OrchestrationRolloutStage::Default,
    }
}

fn fallback_mode_to_runtime(mode: SteelTurnPlanningFallbackMode) -> OrchestrationFallbackMode {
    match mode {
        SteelTurnPlanningFallbackMode::RustNative => OrchestrationFallbackMode::RustNative,
        SteelTurnPlanningFallbackMode::Block => OrchestrationFallbackMode::Block,
    }
}

fn resolve_config_path(base_dir: &Path, path: &str) -> PathBuf {
    let raw = PathBuf::from(path);
    if raw.is_absolute() { raw } else { base_dir.join(raw) }
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
    let authority = receipt
        .ucan_authority_receipt
        .as_ref()
        .map(|authority| format!(" ucan_authority={:?} ucan_reason={:?}", authority.status, authority.reason))
        .unwrap_or_default();
    let message = format!(
        "steel.host.plan_turn receipt status={:?} issue={:?} mode={:?} planner={:?} fallback={:?} receipt_hash={} plan_hash={}{}",
        receipt.status,
        receipt.issue_code,
        receipt.rollout_stage,
        receipt.planner,
        receipt.fallback_status,
        receipt.receipt_hash.prefixed(),
        receipt.plan_hash.map_or_else(|| "none".to_string(), ArtifactHash::prefixed),
        authority,
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
        ucan_authority_grants: request.config.ucan_authority_grants.clone(),
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
    use clankers_config::SteelTurnPlanningAuthorityGrantSettings;
    use clankers_config::SteelTurnPlanningFallbackMode;
    use clankers_config::SteelTurnPlanningRolloutStage;
    use clankers_config::SteelTurnPlanningSettings;
    use clankers_runtime::OrchestrationFallbackMode;
    use clankers_runtime::OrchestrationPlanStatus;
    use clankers_runtime::OrchestrationRolloutStage;
    use clankers_runtime::SteelOrchestrationProfile;

    use super::*;

    fn profile() -> SteelOrchestrationProfile {
        SteelOrchestrationProfile::comparison_default(ArtifactHash::digest(b"script"), ArtifactHash::digest(b"policy"))
    }

    fn fixture_files() -> (tempfile::TempDir, SteelTurnPlanningSettings) {
        let temp = tempfile::tempdir().unwrap();
        let script = "(host \"steel.host.plan_turn\")\n";
        let script_hash = ArtifactHash::digest(script.as_bytes()).prefixed();
        let profile = serde_json::json!({
            "schema": "clankers.steel_default_orchestration.profile.v1",
            "name": "steel-plan-turn-test",
            "enabled": true,
            "default": true,
            "planning_seam": "steel.host.plan_turn",
            "rollout_stage": "comparison",
            "fallback_mode": "rust_native",
            "script": {"id": "default-plan-turn-v1"},
            "runtime_budget": {"max_input_bytes": 8192},
            "allowed_host_actions": [{
                "name": "steel.host.plan_turn",
                "required_session_capabilities": ["steel-orchestration", "turn-planning"],
                "ucan_ability": "clankers/steel/orchestrate.plan_turn"
            }],
            "receipt_policy": {"destination_prefix": "target/steel-turn-planning-config-activation"}
        });
        let profile_text = serde_json::to_string_pretty(&profile).unwrap();
        std::fs::write(temp.path().join("profile.json"), profile_text.as_bytes()).unwrap();
        std::fs::write(temp.path().join("plan.scm"), script).unwrap();
        let settings = SteelTurnPlanningSettings {
            enabled: true,
            profile_path: Some("profile.json".to_string()),
            script_path: Some("plan.scm".to_string()),
            script_blake3: Some(script_hash),
            profile_blake3: Some(ArtifactHash::digest(profile_text.as_bytes()).prefixed()),
            rollout_stage: Some(SteelTurnPlanningRolloutStage::Comparison),
            fallback_mode: Some(SteelTurnPlanningFallbackMode::RustNative),
            planning_seam: None,
            session_capabilities: vec!["steel-orchestration".to_string(), "turn-planning".to_string()],
            granted_ucan_abilities: vec!["clankers/steel/orchestrate.plan_turn".to_string()],
            ucan_authority_grants: vec![SteelTurnPlanningAuthorityGrantSettings {
                resource: "session:session-fixture".to_string(),
                ability: "clankers/steel/orchestrate.plan_turn".to_string(),
                audience: "clankers:agent-turn-planning".to_string(),
                proof_reference: Some("settings-grant".to_string()),
                expires_at: None,
                revoked: false,
                caveats: vec!["metadata_only".to_string()],
            }],
            disabled_actions: Vec::new(),
            receipt_prefix: Some("target/steel-turn-planning-config-activation".to_string()),
            max_input_bytes: None,
            max_source_bytes: 4096,
        };
        (temp, settings)
    }

    #[test]
    fn settings_activation_disabled_by_default() {
        let config = steel_turn_planning_config_from_settings(&SteelTurnPlanningSettings::default(), Path::new("."))
            .expect("disabled settings are valid");
        assert!(config.is_none());
    }

    #[test]
    fn settings_activation_builds_comparison_config_from_profile_and_script() {
        let (temp, settings) = fixture_files();
        let config = steel_turn_planning_config_from_settings(&settings, temp.path())
            .expect("settings should activate")
            .expect("enabled config present");
        assert_eq!(config.profile.rollout_stage, OrchestrationRolloutStage::Comparison);
        assert_eq!(config.profile.planning_seam, DEFAULT_TURN_PLANNING_SEAM);
        assert_eq!(config.profile.receipt_prefix, "target/steel-turn-planning-config-activation");
        assert_eq!(config.session_capabilities, vec!["steel-orchestration", "turn-planning"]);
        assert_eq!(config.ucan_authority_grants.len(), 1);
        assert_eq!(config.ucan_authority_grants[0].resource, "session:session-fixture");
        assert_eq!(config.ucan_authority_grants[0].proof_reference.as_deref(), Some("settings-grant"));
        assert!(!config.steel_source.contains("credential"));
    }

    #[test]
    fn settings_activation_can_select_default_rollout_after_validation() {
        let (temp, mut settings) = fixture_files();
        settings.rollout_stage = Some(SteelTurnPlanningRolloutStage::Default);
        let config = steel_turn_planning_config_from_settings(&settings, temp.path())
            .expect("settings should activate")
            .expect("enabled config present");
        let outcome = plan(&config);
        assert_eq!(config.profile.rollout_stage, OrchestrationRolloutStage::Default);
        assert_eq!(outcome.execution_planner, AgentTurnExecutionPlanner::SteelScheme);
    }

    #[test]
    fn settings_activation_rejects_script_hash_mismatch() {
        let (temp, mut settings) = fixture_files();
        settings.script_blake3 = Some(ArtifactHash::digest(b"other").prefixed());
        let err = steel_turn_planning_config_from_settings(&settings, temp.path()).unwrap_err();
        assert!(matches!(err, SteelTurnPlanningActivationError::ScriptHashMismatch { .. }));
    }

    #[test]
    fn settings_activation_rejects_missing_session_authority() {
        let (temp, mut settings) = fixture_files();
        settings.session_capabilities = vec!["steel-orchestration".to_string()];
        let err = steel_turn_planning_config_from_settings(&settings, temp.path()).unwrap_err();
        assert_eq!(err, SteelTurnPlanningActivationError::MissingSessionCapability("turn-planning".to_string()));
    }

    #[test]
    fn settings_activation_rejects_disabled_required_action() {
        let (temp, mut settings) = fixture_files();
        settings.disabled_actions = vec![DEFAULT_TURN_PLANNING_SEAM.to_string()];
        let err = steel_turn_planning_config_from_settings(&settings, temp.path()).unwrap_err();
        assert_eq!(
            err,
            SteelTurnPlanningActivationError::DisabledRequiredAction(DEFAULT_TURN_PLANNING_SEAM.to_string())
        );
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
        assert_eq!(outcome.execution_planner, AgentTurnExecutionPlanner::Blocked);
        assert_eq!(outcome.receipt.status, OrchestrationPlanStatus::Blocked);
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
