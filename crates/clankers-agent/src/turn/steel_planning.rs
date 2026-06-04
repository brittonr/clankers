use std::collections::BTreeSet;
use std::path::Path;
use std::path::PathBuf;

use clanker_message::transcript::AgentMessage;
use clankers_artifacts::ArtifactHash;
use clankers_runtime::DEFAULT_TURN_EXECUTION_SEAM;
use clankers_runtime::DEFAULT_TURN_PLANNING_SEAM;
use clankers_runtime::OrchestrationCandidate;
use clankers_runtime::OrchestrationFallbackMode;
use clankers_runtime::OrchestrationPlanReceipt;
use clankers_runtime::OrchestrationPlanStatus;
use clankers_runtime::OrchestrationRolloutStage;
use clankers_runtime::SteelOrchestrationProfile;
use clankers_runtime::SteelRuntimeProfile;
use clankers_runtime::SteelTurnPlanHostCallPayload;
use clankers_runtime::SteelTurnPlanningAuthorityGrant;
use clankers_runtime::TurnPlanningInput;
use clankers_runtime::plan_turn_with_steel_or_fallback;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::broadcast;

use crate::events::AgentEvent;
const AGENT_TURN_DECISION_ID: &str = "agent-turn-model-request";
const AGENT_TURN_DECISION_CLASS: &str = "agent-turn-plan";
const DEFAULT_STEEL_SOURCE: &str = "(host \"steel.host.plan_turn\")";
const BUNDLED_DEFAULT_PROFILE_BYTES: &[u8] =
    include_bytes!("../../policy/steel-default-orchestration/orchestration-profile.json");
const BUNDLED_DEFAULT_SCRIPT: &str =
    include_str!("../../policy/steel-default-orchestration/scripts/default-plan-turn.scm");

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
        let mut session_capabilities = profile.required_session_capabilities.clone();
        session_capabilities.extend(profile.execution_required_session_capabilities.clone());
        session_capabilities.sort();
        session_capabilities.dedup();
        let mut granted_ucan_abilities = vec![
            profile.required_ucan_ability.clone(),
            profile.execution_required_ucan_ability.clone(),
        ];
        granted_ucan_abilities.sort();
        granted_ucan_abilities.dedup();
        Self {
            steel_plan_payload: steel_plan_payload(AGENT_TURN_DECISION_ID, target_resource, AGENT_TURN_DECISION_CLASS),
            steel_source: DEFAULT_STEEL_SOURCE.to_string(),
            session_capabilities,
            granted_ucan_abilities,
            ucan_authority_grants: Vec::new(),
            disabled_actions: Vec::new(),
            profile,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSteelTurnPlanningSettings {
    pub enabled: bool,
    pub profile_path: Option<String>,
    pub script_path: Option<String>,
    pub script_blake3: Option<String>,
    pub profile_blake3: Option<String>,
    pub rollout_stage: Option<AgentSteelTurnPlanningRolloutStage>,
    pub fallback_mode: Option<AgentSteelTurnPlanningFallbackMode>,
    pub planning_seam: Option<String>,
    pub session_capabilities: Vec<String>,
    pub granted_ucan_abilities: Vec<String>,
    pub ucan_authority_grants: Vec<AgentSteelTurnPlanningAuthorityGrantSettings>,
    pub disabled_actions: Vec<String>,
    pub receipt_prefix: Option<String>,
    pub max_input_bytes: Option<u64>,
    pub max_source_bytes: u64,
}

impl Default for AgentSteelTurnPlanningSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            profile_path: None,
            script_path: None,
            script_blake3: None,
            profile_blake3: None,
            rollout_stage: None,
            fallback_mode: None,
            planning_seam: None,
            session_capabilities: vec![
                "steel-orchestration".to_string(),
                "turn-planning".to_string(),
                "turn-execution".to_string(),
            ],
            granted_ucan_abilities: vec![
                "clankers/steel/orchestrate.plan_turn".to_string(),
                "clankers/steel/orchestrate.execute_turn".to_string(),
            ],
            ucan_authority_grants: Vec::new(),
            disabled_actions: Vec::new(),
            receipt_prefix: None,
            max_input_bytes: None,
            max_source_bytes: 4096,
        }
    }
}

impl AgentSteelTurnPlanningSettings {
    #[must_use]
    pub fn uses_bundled_profile(&self) -> bool {
        self.profile_path.is_none() && self.script_path.is_none()
    }

    fn validate(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        match self.profile_path.as_deref() {
            Some(path) if path.trim().is_empty() => {
                return Err("Steel turn planning `profilePath` cannot be blank".to_string());
            }
            Some(_) => {}
            None if self.script_path.is_some() => {
                return Err("enabled Steel turn planning requires `profilePath`".to_string());
            }
            None => {}
        }
        match self.script_path.as_deref() {
            Some(path) if path.trim().is_empty() => {
                return Err("Steel turn planning `scriptPath` cannot be blank".to_string());
            }
            Some(_) => {}
            None if self.profile_path.is_some() => {
                return Err("enabled Steel turn planning requires `scriptPath`".to_string());
            }
            None => {}
        }
        if self.script_blake3.as_deref().is_some_and(|hash| hash.trim().is_empty())
            || self.profile_blake3.as_deref().is_some_and(|hash| hash.trim().is_empty())
        {
            return Err("Steel turn planning hashes cannot be blank".to_string());
        }
        if self.session_capabilities.iter().any(|capability| capability.trim().is_empty()) {
            return Err("Steel turn planning session capabilities cannot be blank".to_string());
        }
        if self.granted_ucan_abilities.iter().any(|ability| ability.trim().is_empty()) {
            return Err("Steel turn planning UCAN abilities cannot be blank".to_string());
        }
        if self.ucan_authority_grants.iter().any(|grant| {
            grant.resource.trim().is_empty()
                || grant.ability.trim().is_empty()
                || grant.audience.trim().is_empty()
                || grant.proof_reference.as_deref().is_some_and(|proof| proof.trim().is_empty())
                || grant.caveats.iter().any(|caveat| caveat.trim().is_empty())
        }) {
            return Err("Steel turn planning UCAN authority grants cannot contain blank resource, ability, audience, proof reference, or caveat entries".to_string());
        }
        if self.disabled_actions.iter().any(|action| action.trim().is_empty()) {
            return Err("Steel turn planning disabled actions cannot be blank".to_string());
        }
        if matches!(self.max_input_bytes, Some(0)) {
            return Err("Steel turn planning `maxInputBytes` must be greater than zero".to_string());
        }
        if self.max_source_bytes == 0 {
            return Err("Steel turn planning `maxSourceBytes` must be greater than zero".to_string());
        }
        if let Some(prefix) = &self.receipt_prefix
            && !prefix.starts_with("target/")
        {
            return Err("Steel turn planning `receiptPrefix` must stay under target/".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSteelTurnPlanningAuthorityGrantSettings {
    pub resource: String,
    pub ability: String,
    pub audience: String,
    pub proof_reference: Option<String>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    pub revoked: bool,
    pub caveats: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentSteelTurnPlanningRolloutStage {
    Disabled,
    Comparison,
    Default,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentSteelTurnPlanningFallbackMode {
    RustNative,
    Block,
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
    UnsupportedScriptSource,
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
            Self::UnsupportedScriptSource => {
                f.write_str("Steel turn-planning script must be the reviewed plan_turn host call")
            }
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
    settings: &AgentSteelTurnPlanningSettings,
    base_dir: &Path,
) -> Result<Option<AgentTurnSteelPlanningConfig>, SteelTurnPlanningActivationError> {
    if !settings.enabled {
        return Ok(None);
    }
    settings
        .validate()
        .map_err(|error| SteelTurnPlanningActivationError::InvalidSettings(error.to_string()))?;
    let artifacts = load_planning_artifacts(settings, base_dir)?;
    build_config_from_artifacts(settings, artifacts).map(Some)
}

struct SteelPlanningArtifacts {
    profile_bytes: Vec<u8>,
    script_source: String,
}

fn build_config_from_artifacts(
    settings: &AgentSteelTurnPlanningSettings,
    artifacts: SteelPlanningArtifacts,
) -> Result<AgentTurnSteelPlanningConfig, SteelTurnPlanningActivationError> {
    let profile_bytes = artifacts.profile_bytes;
    let script_source = artifacts.script_source;
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
    validate_turn_planning_script_source(&script_source)?;
    let script_hash = verify_optional_hash(settings.script_blake3.as_deref(), script_bytes, HashKind::Script)?;
    let policy_hash = verify_optional_hash(settings.profile_blake3.as_deref(), &profile_bytes, HashKind::Profile)?;
    let profile_export: NickelSteelOrchestrationProfile = serde_json::from_slice(&profile_bytes)
        .map_err(|error| SteelTurnPlanningActivationError::InvalidProfileJson(error.to_string()))?;
    let profile = runtime_profile_from_export(settings, &profile_export, script_hash, policy_hash)?;
    ensure_session_authority(settings, &profile)?;
    Ok(AgentTurnSteelPlanningConfig {
        steel_plan_payload: steel_plan_payload(AGENT_TURN_DECISION_ID, "session:fixture", AGENT_TURN_DECISION_CLASS),
        steel_source: script_source,
        session_capabilities: settings.session_capabilities.clone(),
        granted_ucan_abilities: settings.granted_ucan_abilities.clone(),
        ucan_authority_grants: authority_grants_from_settings(settings),
        disabled_actions: settings.disabled_actions.clone(),
        profile,
    })
}

fn validate_turn_planning_script_source(source: &str) -> Result<(), SteelTurnPlanningActivationError> {
    if source.trim() == DEFAULT_STEEL_SOURCE {
        return Ok(());
    }
    Err(SteelTurnPlanningActivationError::UnsupportedScriptSource)
}

fn load_planning_artifacts(
    settings: &AgentSteelTurnPlanningSettings,
    base_dir: &Path,
) -> Result<SteelPlanningArtifacts, SteelTurnPlanningActivationError> {
    if settings.uses_bundled_profile() {
        return Ok(SteelPlanningArtifacts {
            profile_bytes: BUNDLED_DEFAULT_PROFILE_BYTES.to_vec(),
            script_source: BUNDLED_DEFAULT_SCRIPT.to_string(),
        });
    }

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
    Ok(SteelPlanningArtifacts {
        profile_bytes,
        script_source,
    })
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
    settings: &AgentSteelTurnPlanningSettings,
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
    let required_action = required_host_action(&export.allowed_host_actions, DEFAULT_TURN_PLANNING_SEAM)?;
    let execution_action = required_host_action(&export.allowed_host_actions, DEFAULT_TURN_EXECUTION_SEAM)?;
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
        execution_required_session_capabilities: execution_action.required_session_capabilities.clone(),
        execution_required_ucan_ability: execution_action.ucan_ability.clone(),
        receipt_prefix: receipt_prefix.to_string(),
        max_input_bytes: settings.max_input_bytes.unwrap_or(export.runtime_budget.max_input_bytes),
    })
}

fn allowed_host_actions(
    actions: &[NickelSteelHostAction],
) -> Result<BTreeSet<String>, SteelTurnPlanningActivationError> {
    let mut allowed = BTreeSet::new();
    for action in actions {
        if action.name != DEFAULT_TURN_PLANNING_SEAM && action.name != DEFAULT_TURN_EXECUTION_SEAM {
            return Err(SteelTurnPlanningActivationError::UnsupportedHostAction(action.name.clone()));
        }
        allowed.insert(action.name.clone());
    }
    for required in [DEFAULT_TURN_PLANNING_SEAM, DEFAULT_TURN_EXECUTION_SEAM] {
        if !allowed.contains(required) {
            return Err(SteelTurnPlanningActivationError::UnsupportedHostAction(format!("missing {required}")));
        }
    }
    Ok(allowed)
}

fn required_host_action<'a>(
    actions: &'a [NickelSteelHostAction],
    name: &str,
) -> Result<&'a NickelSteelHostAction, SteelTurnPlanningActivationError> {
    actions
        .iter()
        .find(|action| action.name == name)
        .ok_or_else(|| SteelTurnPlanningActivationError::UnsupportedHostAction(format!("missing {name}")))
}

fn ensure_session_authority(
    settings: &AgentSteelTurnPlanningSettings,
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

fn authority_grants_from_settings(settings: &AgentSteelTurnPlanningSettings) -> Vec<SteelTurnPlanningAuthorityGrant> {
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

fn rollout_stage_to_runtime(stage: AgentSteelTurnPlanningRolloutStage) -> OrchestrationRolloutStage {
    match stage {
        AgentSteelTurnPlanningRolloutStage::Disabled => OrchestrationRolloutStage::Disabled,
        AgentSteelTurnPlanningRolloutStage::Comparison => OrchestrationRolloutStage::Comparison,
        AgentSteelTurnPlanningRolloutStage::Default => OrchestrationRolloutStage::Default,
    }
}

fn fallback_mode_to_runtime(mode: AgentSteelTurnPlanningFallbackMode) -> OrchestrationFallbackMode {
    match mode {
        AgentSteelTurnPlanningFallbackMode::RustNative => OrchestrationFallbackMode::RustNative,
        AgentSteelTurnPlanningFallbackMode::Block => OrchestrationFallbackMode::Block,
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
    pub tool_names: Vec<String>,
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
        "steel.host.plan_turn receipt status={:?} issue={:?} mode={:?} planner={:?} executor={:?} fallback={:?} receipt_hash={} plan_hash={}{}",
        receipt.status,
        receipt.issue_code,
        receipt.rollout_stage,
        receipt.planner,
        outcome.execution_planner,
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
        tool_names: request.tool_names.clone(),
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

fn steel_plan_payload(
    decision_id: impl Into<String>,
    target_resource: impl Into<String>,
    decision_class: impl Into<String>,
) -> String {
    SteelTurnPlanHostCallPayload::new(decision_id, DEFAULT_TURN_PLANNING_SEAM, target_resource, decision_class)
        .to_json()
}

fn steel_plan_payload_for_session(request: &AgentTurnPlanningRequest<'_>) -> String {
    let session_target = format!("session:{}", safe_route_token(request.session_id));
    let Ok(mut payload) = serde_json::from_str::<SteelTurnPlanHostCallPayload>(&request.config.steel_plan_payload)
    else {
        return request.config.steel_plan_payload.clone();
    };
    payload.target_resource = session_target;
    payload.to_json()
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
    tool_names: Vec<String>,
}

#[cfg(test)]
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(no_panic, no_unwrap, reason = "test code — panics are assertions")
)]
mod tests {
    use clankers_artifacts::ArtifactHash;
    use clankers_runtime::OrchestrationFallbackMode;
    use clankers_runtime::OrchestrationPlanStatus;
    use clankers_runtime::OrchestrationRolloutStage;
    use clankers_runtime::SteelOrchestrationProfile;

    use super::*;

    fn profile() -> SteelOrchestrationProfile {
        SteelOrchestrationProfile::comparison_default(ArtifactHash::digest(b"script"), ArtifactHash::digest(b"policy"))
    }

    fn fixture_files() -> (tempfile::TempDir, AgentSteelTurnPlanningSettings) {
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
            "allowed_host_actions": [
                {
                    "name": "steel.host.plan_turn",
                    "required_session_capabilities": ["steel-orchestration", "turn-planning"],
                    "ucan_ability": "clankers/steel/orchestrate.plan_turn"
                },
                {
                    "name": "steel.host.execute_turn",
                    "required_session_capabilities": ["steel-orchestration", "turn-execution"],
                    "ucan_ability": "clankers/steel/orchestrate.execute_turn"
                }
            ],
            "receipt_policy": {"destination_prefix": "target/steel-turn-planning-config-activation"}
        });
        let profile_text = serde_json::to_string_pretty(&profile).unwrap();
        std::fs::write(temp.path().join("profile.json"), profile_text.as_bytes()).unwrap();
        std::fs::write(temp.path().join("plan.scm"), script).unwrap();
        let settings = AgentSteelTurnPlanningSettings {
            enabled: true,
            profile_path: Some("profile.json".to_string()),
            script_path: Some("plan.scm".to_string()),
            script_blake3: Some(script_hash),
            profile_blake3: Some(ArtifactHash::digest(profile_text.as_bytes()).prefixed()),
            rollout_stage: Some(AgentSteelTurnPlanningRolloutStage::Comparison),
            fallback_mode: Some(AgentSteelTurnPlanningFallbackMode::RustNative),
            planning_seam: None,
            session_capabilities: vec![
                "steel-orchestration".to_string(),
                "turn-planning".to_string(),
                "turn-execution".to_string(),
            ],
            granted_ucan_abilities: vec![
                "clankers/steel/orchestrate.plan_turn".to_string(),
                "clankers/steel/orchestrate.execute_turn".to_string(),
            ],
            ucan_authority_grants: vec![AgentSteelTurnPlanningAuthorityGrantSettings {
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
    fn settings_activation_uses_bundled_default_without_paths() {
        let config = steel_turn_planning_config_from_settings(
            &AgentSteelTurnPlanningSettings::default(),
            Path::new("/path/that/does/not/contain/policy"),
        )
        .expect("bundled default settings are valid")
        .expect("bundled default config present");
        assert_eq!(config.profile.rollout_stage, OrchestrationRolloutStage::Default);
        assert_eq!(config.profile.planning_seam, DEFAULT_TURN_PLANNING_SEAM);
        assert_eq!(config.session_capabilities, vec!["steel-orchestration", "turn-planning", "turn-execution"]);
        assert_eq!(config.granted_ucan_abilities, vec![
            "clankers/steel/orchestrate.plan_turn",
            "clankers/steel/orchestrate.execute_turn"
        ]);
        assert!(config.steel_source.contains(DEFAULT_TURN_PLANNING_SEAM));
        let outcome = plan(&config);
        assert_eq!(outcome.execution_planner, AgentTurnExecutionPlanner::SteelScheme);
        assert_eq!(outcome.receipt.status, OrchestrationPlanStatus::Authorized);
    }

    #[test]
    fn settings_activation_explicit_disabled_uses_rust_native() {
        let settings = AgentSteelTurnPlanningSettings {
            enabled: false,
            ..AgentSteelTurnPlanningSettings::default()
        };
        let config = steel_turn_planning_config_from_settings(&settings, Path::new("."))
            .expect("explicitly disabled settings are valid");
        assert!(config.is_none());
    }

    #[test]
    fn settings_activation_rejects_bundled_script_over_budget() {
        let settings = AgentSteelTurnPlanningSettings {
            max_source_bytes: 1,
            ..AgentSteelTurnPlanningSettings::default()
        };
        let err = steel_turn_planning_config_from_settings(&settings, Path::new(".")).unwrap_err();
        assert!(matches!(err, SteelTurnPlanningActivationError::ScriptTooLarge { .. }));
    }

    #[test]
    fn settings_activation_rejects_bundled_hash_mismatch() {
        let settings = AgentSteelTurnPlanningSettings {
            script_blake3: Some(ArtifactHash::digest(b"wrong-script").prefixed()),
            ..AgentSteelTurnPlanningSettings::default()
        };
        let err = steel_turn_planning_config_from_settings(&settings, Path::new(".")).unwrap_err();
        assert!(matches!(err, SteelTurnPlanningActivationError::ScriptHashMismatch { .. }));
    }

    #[test]
    fn bundled_artifacts_are_compile_time_included() {
        assert!(!BUNDLED_DEFAULT_PROFILE_BYTES.is_empty());
        assert_eq!(BUNDLED_DEFAULT_SCRIPT.trim(), DEFAULT_STEEL_SOURCE);
    }

    #[test]
    fn artifact_core_rejects_malformed_profile_json() {
        let settings = AgentSteelTurnPlanningSettings::default();
        let err = build_config_from_artifacts(&settings, SteelPlanningArtifacts {
            profile_bytes: b"not-json".to_vec(),
            script_source: DEFAULT_STEEL_SOURCE.to_string(),
        })
        .unwrap_err();
        assert!(matches!(err, SteelTurnPlanningActivationError::InvalidProfileJson(_)));
    }

    #[test]
    fn artifact_core_rejects_malformed_script_before_steel_execution() {
        let settings = AgentSteelTurnPlanningSettings::default();
        let err = build_config_from_artifacts(&settings, SteelPlanningArtifacts {
            profile_bytes: BUNDLED_DEFAULT_PROFILE_BYTES.to_vec(),
            script_source: "(write-file \"/tmp/not-allowed\" \"blocked\")".to_string(),
        })
        .unwrap_err();
        assert_eq!(err, SteelTurnPlanningActivationError::UnsupportedScriptSource);
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
        assert_eq!(config.session_capabilities, vec!["steel-orchestration", "turn-planning", "turn-execution"]);
        assert_eq!(config.ucan_authority_grants.len(), 1);
        assert_eq!(config.ucan_authority_grants[0].resource, "session:session-fixture");
        assert_eq!(config.ucan_authority_grants[0].proof_reference.as_deref(), Some("settings-grant"));
        assert!(!config.steel_source.contains("credential"));
    }

    #[test]
    fn settings_activation_can_select_default_rollout_after_validation() {
        let (temp, mut settings) = fixture_files();
        settings.rollout_stage = Some(AgentSteelTurnPlanningRolloutStage::Default);
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

    fn request(config: &AgentTurnSteelPlanningConfig, tool_names: Vec<String>) -> AgentTurnPlanningRequest<'_> {
        AgentTurnPlanningRequest {
            config,
            session_id: "session-fixture",
            model: "model-fixture",
            system_prompt: "secret prompt body is not emitted",
            messages: &[],
            tool_names,
        }
    }

    fn plan(config: &AgentTurnSteelPlanningConfig) -> AgentTurnPlanningOutcome {
        plan_agent_turn(request(config, Vec::new()))
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
