//! Typed Steel self-mutation request DTOs and pure authorization core.
//!
//! Steel scripts never receive filesystem, process, git, network, provider,
//! credential, daemon, TUI, or native-tool authority through this module. They
//! can only describe an intended mutation. The Rust host evaluates the exported
//! Nickel policy plus safe UCAN metadata before any shell code may write bytes.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use clankers_artifacts::ArtifactHash;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

pub const STEEL_MUTATION_POLICY_SCHEMA: &str = "clankers.steel_self_mutation.policy.v1";
pub const STEEL_MUTATION_RECEIPT_SCHEMA: &str = "clankers.steel_self_mutation.receipt.v1";
pub const STEEL_MUTATION_DECISION_SCHEMA: &str = "clankers.steel_self_mutation.decision.v1";
pub const STEEL_MUTATION_PREFLIGHT_SCHEMA: &str = "clankers.steel_self_mutation.preflight.v1";
pub const STEEL_MUTATION_APPLY_SCHEMA: &str = "clankers.steel_self_mutation.apply.v1";
pub const STEEL_MUTATION_ROLLBACK_SCHEMA: &str = "clankers.steel_self_mutation.rollback.v1";
const STEEL_MUTATION_SESSION_CAPABILITY: &str = "steel-self-mutation";
const WORKSPACE_MUTATION_SESSION_CAPABILITY: &str = "workspace-mutation";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationPolicy {
    pub schema: String,
    pub target_classes: Vec<SteelMutationTargetClass>,
    pub mutation_verbs: Vec<SteelMutationVerbPolicy>,
    pub runtime_profiles: Vec<SteelMutationRuntimeProfile>,
    pub ucan: SteelMutationUcanPolicy,
    pub receipt: SteelMutationReceiptPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationTargetClass {
    pub name: String,
    pub resource_prefix: String,
    pub allowed_path_roots: Vec<String>,
    pub denied_path_patterns: Vec<String>,
    pub allowed_verbs: Vec<String>,
    pub approval_tier: String,
    pub preflight_profile: String,
    pub verification_profile: String,
    pub rollback_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationVerbPolicy {
    pub name: String,
    pub host_function: String,
    pub ucan_ability: String,
    pub requires_approval: bool,
    pub writes_bytes: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationRuntimeProfile {
    pub name: String,
    pub steel_profile: String,
    pub max_source_bytes: u64,
    pub max_output_bytes: u64,
    pub max_host_calls: u64,
    pub ambient_authority: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationUcanPolicy {
    pub required: bool,
    pub audience_binding: String,
    pub deny_wildcard_resources: bool,
    pub max_delegation_depth: u32,
    pub safe_receipt_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationReceiptPolicy {
    pub schema: String,
    pub include_policy_hash: bool,
    pub include_safe_ucan_metadata: bool,
    pub redact_fields: Vec<String>,
    pub forbidden_receipt_markers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationRequest {
    pub target_class: String,
    pub verb: String,
    pub resource: String,
    pub expected_audience: String,
    pub relative_path: String,
    pub intent: String,
    pub patch: Option<SteelMutationPatch>,
    pub approval: SteelMutationApproval,
    pub ucan: Option<SteelMutationUcanGrant>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationPatch {
    pub format: SteelMutationPatchFormat,
    pub bytes: u64,
    pub body_blake3: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelMutationPatchFormat {
    UnifiedDiff,
    FullReplace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationApproval {
    pub approved: bool,
    pub tier: String,
    pub reviewer: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationUcanGrant {
    pub ability: String,
    pub resource: String,
    pub audience: String,
    pub expiry_status: SteelMutationUcanExpiryStatus,
    pub delegation_depth: u32,
    pub revoked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelMutationUcanExpiryStatus {
    Valid,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationDecision {
    pub schema: String,
    pub outcome: SteelMutationDecisionOutcome,
    pub reason_code: SteelMutationReasonCode,
    pub safe_message: String,
    pub host_function: Option<String>,
    pub target_class: String,
    pub normalized_path: Option<String>,
    pub required_ucan_ability: Option<String>,
    pub required_ucan_resource: Option<String>,
    pub safe_ucan_metadata: Option<SteelMutationSafeUcanMetadata>,
    pub preflight_profile: Option<String>,
    pub verification_profile: Option<String>,
    pub rollback_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelMutationDecisionOutcome {
    Allowed,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SteelMutationReasonCode {
    Allowed,
    InvalidPolicy,
    UnknownTargetClass,
    UnknownVerb,
    VerbNotAllowedForTarget,
    PathEscape,
    DeniedPathPattern,
    MissingPatch,
    MissingApproval,
    ApprovalTierMismatch,
    MissingUcan,
    ExpiredUcan,
    RevokedUcan,
    WrongUcanAbility,
    WrongUcanAudience,
    WrongUcanResource,
    WildcardUcanResource,
    OverDelegatedUcan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationSafeUcanMetadata {
    pub ability: String,
    pub resource: String,
    pub audience: String,
    pub expiry_status: SteelMutationUcanExpiryStatus,
    pub delegation_depth: u32,
    pub authorization_outcome: SteelMutationDecisionOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationHostContext {
    pub policy_hash: ArtifactHash,
    pub session_capabilities: Vec<String>,
    pub disabled_tools: Vec<String>,
    pub target_hash: Option<ArtifactHash>,
    pub repository_dirty: bool,
    pub checkpoint_id: Option<String>,
}

impl SteelMutationHostContext {
    #[must_use]
    pub fn new(policy_hash: ArtifactHash) -> Self {
        Self {
            policy_hash,
            session_capabilities: Vec::new(),
            disabled_tools: Vec::new(),
            target_hash: None,
            repository_dirty: false,
            checkpoint_id: None,
        }
    }

    #[must_use]
    pub fn with_session_capabilities<I, S>(mut self, capabilities: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.session_capabilities = capabilities.into_iter().map(Into::into).collect();
        self.session_capabilities.sort();
        self.session_capabilities.dedup();
        self
    }

    #[must_use]
    pub fn with_disabled_tools<I, S>(mut self, tools: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.disabled_tools = tools.into_iter().map(Into::into).collect();
        self.disabled_tools.sort();
        self.disabled_tools.dedup();
        self
    }

    #[must_use]
    pub fn with_target_hash(mut self, hash: ArtifactHash) -> Self {
        self.target_hash = Some(hash);
        self
    }

    #[must_use]
    pub fn with_dirty_repository(mut self, checkpoint_id: Option<String>) -> Self {
        self.repository_dirty = true;
        self.checkpoint_id = checkpoint_id;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationHostPreflightReceipt {
    pub schema: String,
    pub status: SteelMutationHostPreflightStatus,
    pub reason_code: SteelMutationHostPreflightReason,
    pub safe_message: String,
    pub decision: SteelMutationDecision,
    pub host_function: Option<String>,
    pub normalized_path: Option<String>,
    pub policy_hash: ArtifactHash,
    pub target_hash: Option<ArtifactHash>,
    pub checkpoint: SteelMutationCheckpointPlan,
    pub verification_profile: Option<String>,
    pub safe_ucan_metadata: Option<SteelMutationSafeUcanMetadata>,
    pub writes_performed: bool,
}

impl SteelMutationHostPreflightReceipt {
    #[must_use]
    pub fn receipt_hash(&self) -> ArtifactHash {
        let bytes = serde_json::to_vec(self).expect("Steel mutation preflight receipt serializes");
        ArtifactHash::digest(&bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelMutationHostPreflightStatus {
    Ready,
    Denied,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SteelMutationHostPreflightReason {
    Ready,
    DecisionDenied,
    MissingSessionCapability,
    DisabledHostFunction,
    DirtyRepositoryNeedsCheckpoint,
    MissingTargetHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationCheckpointPlan {
    pub required: bool,
    pub checkpoint_id: Option<String>,
    pub target_hash: Option<ArtifactHash>,
    pub policy_hash: ArtifactHash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteelMutationPatchPayload {
    pub format: SteelMutationPatchFormat,
    pub body: Vec<u8>,
}

impl SteelMutationPatchPayload {
    #[must_use]
    pub fn full_replace(body: impl Into<Vec<u8>>) -> Self {
        Self {
            format: SteelMutationPatchFormat::FullReplace,
            body: body.into(),
        }
    }

    #[must_use]
    pub fn body_hash(&self) -> ArtifactHash {
        ArtifactHash::digest(&self.body)
    }

    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.body.len() as u64
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationApplyReceipt {
    pub schema: String,
    pub status: SteelMutationApplyStatus,
    pub reason_code: SteelMutationApplyReason,
    pub safe_message: String,
    pub preflight: SteelMutationHostPreflightReceipt,
    pub normalized_path: Option<String>,
    pub policy_hash: ArtifactHash,
    pub target_hash_before: Option<ArtifactHash>,
    pub backup_hash: Option<ArtifactHash>,
    pub patch_hash: Option<ArtifactHash>,
    pub target_hash_after: Option<ArtifactHash>,
    pub verification: SteelMutationVerificationReceipt,
    pub writes_performed: bool,
}

impl SteelMutationApplyReceipt {
    #[must_use]
    pub fn receipt_hash(&self) -> ArtifactHash {
        let bytes = serde_json::to_vec(self).expect("Steel mutation apply receipt serializes");
        ArtifactHash::digest(&bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelMutationApplyStatus {
    Applied,
    Blocked,
    FailedVerification,
    FailedWrite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SteelMutationApplyReason {
    Applied,
    PreflightNotReady,
    MissingPatchDescriptor,
    PatchFormatMismatch,
    PatchHashMismatch,
    PatchSizeMismatch,
    UnsupportedPatchFormat,
    StaleTargetHash,
    TargetReadFailed,
    TargetWriteFailed,
    VerificationFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationVerificationReceipt {
    pub profile: Option<String>,
    pub status: SteelMutationVerificationStatus,
    pub safe_summary: String,
}

impl SteelMutationVerificationReceipt {
    #[must_use]
    pub fn skipped(message: impl Into<String>) -> Self {
        Self {
            profile: None,
            status: SteelMutationVerificationStatus::Skipped,
            safe_summary: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelMutationVerificationStatus {
    Passed,
    Failed,
    Skipped,
}

pub trait SteelMutationTargetStore {
    fn read_target(&self, normalized_path: &str) -> Result<Vec<u8>, SteelMutationIoError>;
    fn write_target(&mut self, normalized_path: &str, bytes: &[u8]) -> Result<(), SteelMutationIoError>;
}

pub trait SteelMutationVerifier {
    fn verify(&self, profile: &str, normalized_path: &str) -> SteelMutationVerificationReceipt;
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SteelMutationIoError {
    #[error("target is unavailable")]
    Unavailable,
    #[error("target IO failed: {message}")]
    Failed { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationRollbackReceipt {
    pub schema: String,
    pub status: SteelMutationRollbackStatus,
    pub reason_code: SteelMutationRollbackReason,
    pub safe_message: String,
    pub normalized_path: Option<String>,
    pub policy_hash: ArtifactHash,
    pub current_target_hash: Option<ArtifactHash>,
    pub recorded_post_apply_hash: Option<ArtifactHash>,
    pub backup_hash: Option<ArtifactHash>,
    pub restored_target_hash: Option<ArtifactHash>,
    pub writes_performed: bool,
}

impl SteelMutationRollbackReceipt {
    #[must_use]
    pub fn receipt_hash(&self) -> ArtifactHash {
        let bytes = serde_json::to_vec(self).expect("Steel mutation rollback receipt serializes");
        ArtifactHash::digest(&bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelMutationRollbackStatus {
    RolledBack,
    Blocked,
    FailedWrite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SteelMutationRollbackReason {
    RolledBack,
    ApplyReceiptNotRollbackable,
    MissingRecordedPostApplyHash,
    MissingBackupHash,
    BackupHashMismatch,
    CurrentTargetChanged,
    TargetReadFailed,
    TargetWriteFailed,
}

pub trait SteelMutationBackupStore {
    fn read_backup(&self, backup_hash: ArtifactHash) -> Result<Vec<u8>, SteelMutationIoError>;
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SteelMutationPolicyParseError {
    #[error("failed to parse Steel mutation policy: {message}")]
    Json { message: String },
}

pub fn parse_steel_mutation_policy(text: &str) -> Result<SteelMutationPolicy, SteelMutationPolicyParseError> {
    serde_json::from_str(text).map_err(|error| SteelMutationPolicyParseError::Json {
        message: error.to_string(),
    })
}

#[must_use]
pub fn rollback_steel_mutation_host_function(
    apply_receipt: &SteelMutationApplyReceipt,
    target_store: &mut dyn SteelMutationTargetStore,
    backup_store: &dyn SteelMutationBackupStore,
) -> SteelMutationRollbackReceipt {
    let rollback = match rollback_inputs(apply_receipt) {
        RollbackInputStatus::Ready(rollback) => rollback,
        RollbackInputStatus::Blocked(receipt) => return receipt,
    };
    let current = match target_store.read_target(&rollback.path) {
        Ok(bytes) => bytes,
        Err(_) => {
            return rollback_receipt(
                apply_receipt,
                SteelMutationRollbackStatus::FailedWrite,
                SteelMutationRollbackReason::TargetReadFailed,
                "failed to read target before rollback",
                None,
                None,
                false,
            );
        }
    };
    let current_hash = ArtifactHash::digest(&current);
    if current_hash != rollback.recorded_post_apply_hash {
        return rollback_receipt(
            apply_receipt,
            SteelMutationRollbackStatus::Blocked,
            SteelMutationRollbackReason::CurrentTargetChanged,
            "target changed after mutation; rollback refused to avoid clobbering operator edits",
            Some(current_hash),
            None,
            false,
        );
    }
    let backup = match backup_store.read_backup(rollback.backup_hash) {
        Ok(bytes) => bytes,
        Err(_) => {
            return rollback_receipt(
                apply_receipt,
                SteelMutationRollbackStatus::Blocked,
                SteelMutationRollbackReason::MissingBackupHash,
                "backup bytes are unavailable for rollback",
                Some(current_hash),
                None,
                false,
            );
        }
    };
    let actual_backup_hash = ArtifactHash::digest(&backup);
    if actual_backup_hash != rollback.backup_hash {
        return rollback_receipt(
            apply_receipt,
            SteelMutationRollbackStatus::Blocked,
            SteelMutationRollbackReason::BackupHashMismatch,
            "backup bytes do not match recorded backup hash",
            Some(current_hash),
            None,
            false,
        );
    }
    if target_store.write_target(&rollback.path, &backup).is_err() {
        return rollback_receipt(
            apply_receipt,
            SteelMutationRollbackStatus::FailedWrite,
            SteelMutationRollbackReason::TargetWriteFailed,
            "failed to restore rollback backup bytes",
            Some(current_hash),
            None,
            false,
        );
    }
    rollback_receipt(
        apply_receipt,
        SteelMutationRollbackStatus::RolledBack,
        SteelMutationRollbackReason::RolledBack,
        "rollback restored recorded backup bytes after hash guards passed",
        Some(current_hash),
        Some(actual_backup_hash),
        true,
    )
}

struct RollbackInputs {
    path: String,
    recorded_post_apply_hash: ArtifactHash,
    backup_hash: ArtifactHash,
}

enum RollbackInputStatus {
    Ready(RollbackInputs),
    Blocked(SteelMutationRollbackReceipt),
}

fn rollback_inputs(apply_receipt: &SteelMutationApplyReceipt) -> RollbackInputStatus {
    let Some(path) = apply_receipt.normalized_path.clone() else {
        return RollbackInputStatus::Blocked(rollback_receipt(
            apply_receipt,
            SteelMutationRollbackStatus::Blocked,
            SteelMutationRollbackReason::ApplyReceiptNotRollbackable,
            "apply receipt does not identify a rollback target",
            None,
            None,
            false,
        ));
    };
    let Some(recorded_post_apply_hash) = apply_receipt.target_hash_after else {
        return RollbackInputStatus::Blocked(rollback_receipt(
            apply_receipt,
            SteelMutationRollbackStatus::Blocked,
            SteelMutationRollbackReason::MissingRecordedPostApplyHash,
            "rollback requires recorded post-apply target hash",
            None,
            None,
            false,
        ));
    };
    let Some(backup_hash) = apply_receipt.backup_hash else {
        return RollbackInputStatus::Blocked(rollback_receipt(
            apply_receipt,
            SteelMutationRollbackStatus::Blocked,
            SteelMutationRollbackReason::MissingBackupHash,
            "rollback requires recorded backup hash",
            None,
            None,
            false,
        ));
    };
    RollbackInputStatus::Ready(RollbackInputs {
        path,
        recorded_post_apply_hash,
        backup_hash,
    })
}

#[must_use]
pub fn preflight_steel_mutation_host_function(
    policy: &SteelMutationPolicy,
    request: &SteelMutationRequest,
    context: &SteelMutationHostContext,
) -> SteelMutationHostPreflightReceipt {
    let decision = authorize_steel_mutation(policy, request);
    let checkpoint = checkpoint_plan(&decision, context);
    if decision.outcome == SteelMutationDecisionOutcome::Denied {
        return host_preflight_receipt(
            decision,
            context,
            checkpoint,
            SteelMutationHostPreflightStatus::Denied,
            SteelMutationHostPreflightReason::DecisionDenied,
            "Rust host denied mutation before preflight planning",
        );
    }
    if let Some(missing) = missing_required_capability(&decision, request, context) {
        return host_preflight_receipt(
            decision,
            context,
            checkpoint,
            SteelMutationHostPreflightStatus::Blocked,
            SteelMutationHostPreflightReason::MissingSessionCapability,
            format!("session lacks required mutation capability `{missing}`"),
        );
    }
    if host_function_is_disabled(&decision, request, context) {
        return host_preflight_receipt(
            decision,
            context,
            checkpoint,
            SteelMutationHostPreflightStatus::Blocked,
            SteelMutationHostPreflightReason::DisabledHostFunction,
            "requested Steel mutation host function is disabled for this session",
        );
    }
    if context.repository_dirty && context.checkpoint_id.is_none() {
        return host_preflight_receipt(
            decision,
            context,
            checkpoint,
            SteelMutationHostPreflightStatus::Blocked,
            SteelMutationHostPreflightReason::DirtyRepositoryNeedsCheckpoint,
            "dirty repository requires an explicit checkpoint before mutation",
        );
    }
    if decision.rollback_required && context.target_hash.is_none() {
        return host_preflight_receipt(
            decision,
            context,
            checkpoint,
            SteelMutationHostPreflightStatus::Blocked,
            SteelMutationHostPreflightReason::MissingTargetHash,
            "rollback-required mutation must capture a target hash before writing",
        );
    }
    host_preflight_receipt(
        decision,
        context,
        checkpoint,
        SteelMutationHostPreflightStatus::Ready,
        SteelMutationHostPreflightReason::Ready,
        "Steel mutation host function is ready for imperative shell execution",
    )
}

#[must_use]
pub fn apply_steel_mutation_host_function(
    policy: &SteelMutationPolicy,
    request: &SteelMutationRequest,
    context: &SteelMutationHostContext,
    payload: &SteelMutationPatchPayload,
    target_store: &mut dyn SteelMutationTargetStore,
    verifier: &dyn SteelMutationVerifier,
) -> SteelMutationApplyReceipt {
    let preflight = preflight_steel_mutation_host_function(policy, request, context);
    if preflight.status != SteelMutationHostPreflightStatus::Ready {
        return apply_blocked_receipt(
            preflight,
            SteelMutationApplyReason::PreflightNotReady,
            "mutation apply blocked before writing by host preflight",
            None,
            "preflight was not ready",
        );
    }
    let patch_hash = payload.body_hash();
    let Some(descriptor) = request.patch.as_ref() else {
        return apply_blocked_receipt(
            preflight,
            SteelMutationApplyReason::MissingPatchDescriptor,
            "mutation apply requires a patch descriptor",
            None,
            "missing patch descriptor",
        );
    };
    if let Some(receipt) = patch_payload_validation_receipt(preflight.clone(), descriptor, payload, patch_hash) {
        return receipt;
    }
    let Some(path) = preflight.normalized_path.clone() else {
        return apply_blocked_receipt(
            preflight,
            SteelMutationApplyReason::PreflightNotReady,
            "preflight did not produce a normalized target path",
            Some(patch_hash),
            "missing normalized path",
        );
    };
    let target_before = match target_store.read_target(&path) {
        Ok(bytes) => bytes,
        Err(_) => {
            return apply_receipt(
                preflight,
                SteelMutationApplyStatus::FailedWrite,
                SteelMutationApplyReason::TargetReadFailed,
                "failed to read target before mutation",
                None,
                None,
                Some(patch_hash),
                None,
                SteelMutationVerificationReceipt::skipped("target read failed"),
                false,
            );
        }
    };
    let before_hash = ArtifactHash::digest(&target_before);
    if preflight.target_hash != Some(before_hash) {
        return apply_receipt(
            preflight,
            SteelMutationApplyStatus::Blocked,
            SteelMutationApplyReason::StaleTargetHash,
            "target hash changed after preflight checkpoint capture",
            Some(before_hash),
            None,
            Some(patch_hash),
            None,
            SteelMutationVerificationReceipt::skipped("stale target hash"),
            false,
        );
    }
    write_and_verify_mutation(preflight, payload, target_store, verifier, path, before_hash, patch_hash)
}

fn apply_blocked_receipt(
    preflight: SteelMutationHostPreflightReceipt,
    reason: SteelMutationApplyReason,
    message: &'static str,
    patch_hash: Option<ArtifactHash>,
    verification_message: &'static str,
) -> SteelMutationApplyReceipt {
    apply_receipt(
        preflight,
        SteelMutationApplyStatus::Blocked,
        reason,
        message,
        None,
        None,
        patch_hash,
        None,
        SteelMutationVerificationReceipt::skipped(verification_message),
        false,
    )
}

fn patch_payload_validation_receipt(
    preflight: SteelMutationHostPreflightReceipt,
    descriptor: &SteelMutationPatch,
    payload: &SteelMutationPatchPayload,
    patch_hash: ArtifactHash,
) -> Option<SteelMutationApplyReceipt> {
    if descriptor.format != payload.format {
        return Some(apply_blocked_receipt(
            preflight,
            SteelMutationApplyReason::PatchFormatMismatch,
            "patch payload format does not match authorized descriptor",
            Some(patch_hash),
            "patch format mismatch",
        ));
    }
    if descriptor.bytes != payload.bytes() {
        return Some(apply_blocked_receipt(
            preflight,
            SteelMutationApplyReason::PatchSizeMismatch,
            "patch payload size does not match authorized descriptor",
            Some(patch_hash),
            "patch size mismatch",
        ));
    }
    if descriptor.body_blake3 != patch_hash.prefixed() {
        return Some(apply_blocked_receipt(
            preflight,
            SteelMutationApplyReason::PatchHashMismatch,
            "patch payload hash does not match authorized descriptor",
            Some(patch_hash),
            "patch hash mismatch",
        ));
    }
    if payload.format != SteelMutationPatchFormat::FullReplace {
        return Some(apply_blocked_receipt(
            preflight,
            SteelMutationApplyReason::UnsupportedPatchFormat,
            "only full-replace payloads are currently executable by the host apply shell",
            Some(patch_hash),
            "unsupported patch format",
        ));
    }
    None
}

fn write_and_verify_mutation(
    preflight: SteelMutationHostPreflightReceipt,
    payload: &SteelMutationPatchPayload,
    target_store: &mut dyn SteelMutationTargetStore,
    verifier: &dyn SteelMutationVerifier,
    path: String,
    before_hash: ArtifactHash,
    patch_hash: ArtifactHash,
) -> SteelMutationApplyReceipt {
    if target_store.write_target(&path, &payload.body).is_err() {
        return apply_receipt(
            preflight,
            SteelMutationApplyStatus::FailedWrite,
            SteelMutationApplyReason::TargetWriteFailed,
            "failed to write target mutation payload",
            Some(before_hash),
            Some(before_hash),
            Some(patch_hash),
            None,
            SteelMutationVerificationReceipt::skipped("target write failed"),
            false,
        );
    }
    let after_hash = ArtifactHash::digest(&payload.body);
    let verification = match preflight.verification_profile.as_deref() {
        Some(profile) => verifier.verify(profile, &path),
        None => SteelMutationVerificationReceipt::skipped("no verification profile selected"),
    };
    if verification.status != SteelMutationVerificationStatus::Passed {
        return apply_receipt(
            preflight,
            SteelMutationApplyStatus::FailedVerification,
            SteelMutationApplyReason::VerificationFailed,
            "mutation payload was written but verification failed",
            Some(before_hash),
            Some(before_hash),
            Some(patch_hash),
            Some(after_hash),
            verification,
            true,
        );
    }
    apply_receipt(
        preflight,
        SteelMutationApplyStatus::Applied,
        SteelMutationApplyReason::Applied,
        "mutation payload was written and verification passed",
        Some(before_hash),
        Some(before_hash),
        Some(patch_hash),
        Some(after_hash),
        verification,
        true,
    )
}

#[must_use]
pub fn authorize_steel_mutation(policy: &SteelMutationPolicy, request: &SteelMutationRequest) -> SteelMutationDecision {
    let Some(target) = target_class(policy, &request.target_class) else {
        return deny(
            request,
            SteelMutationReasonCode::UnknownTargetClass,
            "mutation target class is not declared by policy",
            None,
            None,
            None,
        );
    };
    let Some(verb) = verb_policy(policy, &request.verb) else {
        return deny_with_target(
            request,
            target,
            SteelMutationReasonCode::UnknownVerb,
            "mutation verb is not declared by policy",
            None,
            None,
        );
    };
    if let Some(decision) = pre_ucan_mutation_denial(policy, request, target, verb) {
        return decision;
    }
    let Some(normalized_path) = normalize_relative_path(&request.relative_path) else {
        return deny_with_target(
            request,
            target,
            SteelMutationReasonCode::PathEscape,
            "mutation path escapes the repository-relative target boundary",
            Some(verb),
            None,
        );
    };
    if let Some(decision) = path_mutation_denial(request, target, verb, &normalized_path) {
        return decision;
    }
    let required_resource = format!("{}{}", target.resource_prefix, request.resource);
    let ucan_request = UcanAuthorizationRequest {
        policy,
        verb,
        required_resource: &required_resource,
        expected_audience: &request.expected_audience,
        grant: request.ucan.as_ref(),
    };
    let ucan = match authorize_ucan(ucan_request) {
        Ok(grant) => grant,
        Err((code, message, metadata)) => {
            return deny_with_target(request, target, code, message, Some(verb), Some(normalized_path))
                .with_safe_ucan_metadata(metadata);
        }
    };
    allowed_mutation_decision(request, target, verb, normalized_path, required_resource.clone(), ucan)
}

fn pre_ucan_mutation_denial(
    policy: &SteelMutationPolicy,
    request: &SteelMutationRequest,
    target: &SteelMutationTargetClass,
    verb: &SteelMutationVerbPolicy,
) -> Option<SteelMutationDecision> {
    if !policy_is_safe(policy) {
        return Some(deny_with_target(
            request,
            target,
            SteelMutationReasonCode::InvalidPolicy,
            "mutation policy is not fail-closed",
            Some(verb),
            None,
        ));
    }
    if !target.allowed_verbs.iter().any(|allowed| allowed == &request.verb) {
        return Some(deny_with_target(
            request,
            target,
            SteelMutationReasonCode::VerbNotAllowedForTarget,
            "mutation verb is not allowed for target class",
            Some(verb),
            None,
        ));
    }
    None
}

fn path_mutation_denial(
    request: &SteelMutationRequest,
    target: &SteelMutationTargetClass,
    verb: &SteelMutationVerbPolicy,
    normalized_path: &str,
) -> Option<SteelMutationDecision> {
    if !path_has_allowed_root(normalized_path, &target.allowed_path_roots) {
        return Some(deny_with_target(
            request,
            target,
            SteelMutationReasonCode::PathEscape,
            "mutation path is outside policy allowlisted roots",
            Some(verb),
            Some(normalized_path.to_string()),
        ));
    }
    if path_hits_denied_pattern(normalized_path, &target.denied_path_patterns) {
        return Some(deny_with_target(
            request,
            target,
            SteelMutationReasonCode::DeniedPathPattern,
            "mutation path matches a denied policy pattern",
            Some(verb),
            Some(normalized_path.to_string()),
        ));
    }
    write_and_approval_denial(request, target, verb, normalized_path)
}

fn write_and_approval_denial(
    request: &SteelMutationRequest,
    target: &SteelMutationTargetClass,
    verb: &SteelMutationVerbPolicy,
    normalized_path: &str,
) -> Option<SteelMutationDecision> {
    if verb.writes_bytes && request.patch.is_none() {
        return Some(deny_with_target(
            request,
            target,
            SteelMutationReasonCode::MissingPatch,
            "byte-writing mutation verb requires a patch descriptor",
            Some(verb),
            Some(normalized_path.to_string()),
        ));
    }
    if verb.requires_approval && !request.approval.approved {
        return Some(deny_with_target(
            request,
            target,
            SteelMutationReasonCode::MissingApproval,
            "mutation verb requires explicit approval",
            Some(verb),
            Some(normalized_path.to_string()),
        ));
    }
    if verb.requires_approval && request.approval.tier != target.approval_tier {
        return Some(deny_with_target(
            request,
            target,
            SteelMutationReasonCode::ApprovalTierMismatch,
            "approval tier does not match target policy",
            Some(verb),
            Some(normalized_path.to_string()),
        ));
    }
    None
}

fn allowed_mutation_decision(
    _request: &SteelMutationRequest,
    target: &SteelMutationTargetClass,
    verb: &SteelMutationVerbPolicy,
    normalized_path: String,
    required_resource: String,
    ucan: &SteelMutationUcanGrant,
) -> SteelMutationDecision {
    SteelMutationDecision {
        schema: STEEL_MUTATION_DECISION_SCHEMA.to_string(),
        outcome: SteelMutationDecisionOutcome::Allowed,
        reason_code: SteelMutationReasonCode::Allowed,
        safe_message: "mutation request is authorized for Rust host preflight".to_string(),
        host_function: Some(verb.host_function.clone()),
        target_class: target.name.clone(),
        normalized_path: Some(normalized_path),
        required_ucan_ability: Some(verb.ucan_ability.clone()),
        required_ucan_resource: Some(required_resource),
        safe_ucan_metadata: Some(safe_ucan_metadata(ucan, SteelMutationDecisionOutcome::Allowed)),
        preflight_profile: Some(target.preflight_profile.clone()),
        verification_profile: Some(target.verification_profile.clone()),
        rollback_required: target.rollback_required,
    }
}

fn rollback_receipt(
    apply_receipt: &SteelMutationApplyReceipt,
    status: SteelMutationRollbackStatus,
    reason_code: SteelMutationRollbackReason,
    message: impl Into<String>,
    current_target_hash: Option<ArtifactHash>,
    restored_target_hash: Option<ArtifactHash>,
    writes_performed: bool,
) -> SteelMutationRollbackReceipt {
    SteelMutationRollbackReceipt {
        schema: STEEL_MUTATION_ROLLBACK_SCHEMA.to_string(),
        status,
        reason_code,
        safe_message: message.into(),
        normalized_path: apply_receipt.normalized_path.clone(),
        policy_hash: apply_receipt.policy_hash,
        current_target_hash,
        recorded_post_apply_hash: apply_receipt.target_hash_after,
        backup_hash: apply_receipt.backup_hash,
        restored_target_hash,
        writes_performed,
    }
}

fn apply_receipt(
    preflight: SteelMutationHostPreflightReceipt,
    status: SteelMutationApplyStatus,
    reason_code: SteelMutationApplyReason,
    message: impl Into<String>,
    target_hash_before: Option<ArtifactHash>,
    backup_hash: Option<ArtifactHash>,
    patch_hash: Option<ArtifactHash>,
    target_hash_after: Option<ArtifactHash>,
    verification: SteelMutationVerificationReceipt,
    writes_performed: bool,
) -> SteelMutationApplyReceipt {
    SteelMutationApplyReceipt {
        schema: STEEL_MUTATION_APPLY_SCHEMA.to_string(),
        status,
        reason_code,
        safe_message: message.into(),
        normalized_path: preflight.normalized_path.clone(),
        policy_hash: preflight.policy_hash,
        preflight,
        target_hash_before,
        backup_hash,
        patch_hash,
        target_hash_after,
        verification,
        writes_performed,
    }
}

fn host_preflight_receipt(
    decision: SteelMutationDecision,
    context: &SteelMutationHostContext,
    checkpoint: SteelMutationCheckpointPlan,
    status: SteelMutationHostPreflightStatus,
    reason_code: SteelMutationHostPreflightReason,
    message: impl Into<String>,
) -> SteelMutationHostPreflightReceipt {
    let host_function = decision.host_function.clone();
    let normalized_path = decision.normalized_path.clone();
    let verification_profile = decision.verification_profile.clone();
    let safe_ucan_metadata = decision.safe_ucan_metadata.clone();
    SteelMutationHostPreflightReceipt {
        schema: STEEL_MUTATION_PREFLIGHT_SCHEMA.to_string(),
        status,
        reason_code,
        safe_message: message.into(),
        decision,
        host_function,
        normalized_path,
        policy_hash: context.policy_hash,
        target_hash: context.target_hash,
        checkpoint,
        verification_profile,
        safe_ucan_metadata,
        writes_performed: false,
    }
}

fn checkpoint_plan(
    decision: &SteelMutationDecision,
    context: &SteelMutationHostContext,
) -> SteelMutationCheckpointPlan {
    SteelMutationCheckpointPlan {
        required: decision.rollback_required,
        checkpoint_id: context.checkpoint_id.clone(),
        target_hash: context.target_hash,
        policy_hash: context.policy_hash,
    }
}

fn missing_required_capability(
    decision: &SteelMutationDecision,
    request: &SteelMutationRequest,
    context: &SteelMutationHostContext,
) -> Option<&'static str> {
    let capabilities = context.session_capabilities.iter().map(String::as_str).collect::<BTreeSet<_>>();
    if !capabilities.contains(STEEL_MUTATION_SESSION_CAPABILITY) {
        return Some(STEEL_MUTATION_SESSION_CAPABILITY);
    }
    if decision.host_function.as_deref() != Some("steel.host.propose_mutation")
        && request.patch.is_some()
        && !capabilities.contains(WORKSPACE_MUTATION_SESSION_CAPABILITY)
    {
        return Some(WORKSPACE_MUTATION_SESSION_CAPABILITY);
    }
    None
}

fn host_function_is_disabled(
    decision: &SteelMutationDecision,
    request: &SteelMutationRequest,
    context: &SteelMutationHostContext,
) -> bool {
    let Some(host_function) = decision.host_function.as_deref() else {
        return false;
    };
    context.disabled_tools.iter().any(|disabled| {
        disabled == host_function
            || disabled == "steel.host.*"
            || disabled == "steel-self-mutation"
            || disabled == &request.verb
    })
}

fn target_class<'a>(policy: &'a SteelMutationPolicy, name: &str) -> Option<&'a SteelMutationTargetClass> {
    policy.target_classes.iter().find(|target| target.name == name)
}

fn verb_policy<'a>(policy: &'a SteelMutationPolicy, name: &str) -> Option<&'a SteelMutationVerbPolicy> {
    policy.mutation_verbs.iter().find(|verb| verb.name == name)
}

fn policy_is_safe(policy: &SteelMutationPolicy) -> bool {
    policy.schema == STEEL_MUTATION_POLICY_SCHEMA
        && policy.ucan.required
        && policy.ucan.deny_wildcard_resources
        && policy.receipt.schema == STEEL_MUTATION_RECEIPT_SCHEMA
        && policy.receipt.include_policy_hash
        && policy.receipt.include_safe_ucan_metadata
        && policy.runtime_profiles.iter().all(|profile| !profile.ambient_authority)
        && names_are_unique(policy.target_classes.iter().map(|target| target.name.as_str()))
        && names_are_unique(policy.mutation_verbs.iter().map(|verb| verb.name.as_str()))
}

fn names_are_unique<'a>(mut names: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = BTreeMap::new();
    names.all(|name| seen.insert(name, ()).is_none())
}

fn normalize_relative_path(path: &str) -> Option<String> {
    if path.is_empty() || path.starts_with('/') || path.contains('\0') {
        return None;
    }
    let mut normalized = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => return None,
            segment if segment.contains('\\') => return None,
            segment => normalized.push(segment),
        }
    }
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.join("/"))
    }
}

fn path_has_allowed_root(path: &str, roots: &[String]) -> bool {
    roots.iter().any(|root| {
        let normalized_root = root.trim_start_matches("./").trim_end_matches('/');
        path == normalized_root || path.strip_prefix(normalized_root).is_some_and(|suffix| suffix.starts_with('/'))
    })
}

fn path_hits_denied_pattern(path: &str, denied_patterns: &[String]) -> bool {
    denied_patterns.iter().any(|pattern| match pattern.as_str() {
        "../" => path.split('/').any(|part| part == ".."),
        "/.git/" => path.contains("/.git/") || path.starts_with(".git/"),
        "**/.env*" => path.split('/').any(|part| part.starts_with(".env")),
        "**/*secret*" => path.to_ascii_lowercase().contains("secret"),
        pattern => path.contains(pattern.trim_matches('*')),
    })
}

struct UcanAuthorizationRequest<'a> {
    policy: &'a SteelMutationPolicy,
    verb: &'a SteelMutationVerbPolicy,
    required_resource: &'a str,
    expected_audience: &'a str,
    grant: Option<&'a SteelMutationUcanGrant>,
}

fn authorize_ucan(
    request: UcanAuthorizationRequest<'_>,
) -> Result<&SteelMutationUcanGrant, (SteelMutationReasonCode, &'static str, Option<SteelMutationSafeUcanMetadata>)> {
    let Some(grant) = request.grant else {
        return Err((SteelMutationReasonCode::MissingUcan, "mutation requires UCAN authority", None));
    };
    let denied_metadata = || safe_ucan_metadata(grant, SteelMutationDecisionOutcome::Denied);
    if grant.revoked {
        return Err((SteelMutationReasonCode::RevokedUcan, "UCAN grant is revoked", Some(denied_metadata())));
    }
    if grant.expiry_status != SteelMutationUcanExpiryStatus::Valid {
        return Err((SteelMutationReasonCode::ExpiredUcan, "UCAN grant is expired", Some(denied_metadata())));
    }
    if grant.ability != request.verb.ucan_ability {
        return Err((
            SteelMutationReasonCode::WrongUcanAbility,
            "UCAN ability does not authorize mutation verb",
            Some(denied_metadata()),
        ));
    }
    if grant.audience != request.expected_audience {
        return Err((
            SteelMutationReasonCode::WrongUcanAudience,
            "UCAN audience does not match mutation host context",
            Some(denied_metadata()),
        ));
    }
    if grant.delegation_depth > request.policy.ucan.max_delegation_depth {
        return Err((
            SteelMutationReasonCode::OverDelegatedUcan,
            "UCAN delegation depth exceeds mutation policy",
            Some(denied_metadata()),
        ));
    }
    if request.policy.ucan.deny_wildcard_resources && grant.resource == "*" {
        return Err((
            SteelMutationReasonCode::WildcardUcanResource,
            "wildcard UCAN resource is denied for live mutation",
            Some(denied_metadata()),
        ));
    }
    if grant.resource != request.required_resource {
        return Err((
            SteelMutationReasonCode::WrongUcanResource,
            "UCAN resource does not match mutation target",
            Some(denied_metadata()),
        ));
    }
    Ok(grant)
}

fn safe_ucan_metadata(
    grant: &SteelMutationUcanGrant,
    outcome: SteelMutationDecisionOutcome,
) -> SteelMutationSafeUcanMetadata {
    SteelMutationSafeUcanMetadata {
        ability: grant.ability.clone(),
        resource: grant.resource.clone(),
        audience: grant.audience.clone(),
        expiry_status: grant.expiry_status.clone(),
        delegation_depth: grant.delegation_depth,
        authorization_outcome: outcome,
    }
}

fn deny(
    request: &SteelMutationRequest,
    reason_code: SteelMutationReasonCode,
    message: impl Into<String>,
    host_function: Option<String>,
    required_ability: Option<String>,
    required_resource: Option<String>,
) -> SteelMutationDecision {
    SteelMutationDecision {
        schema: STEEL_MUTATION_DECISION_SCHEMA.to_string(),
        outcome: SteelMutationDecisionOutcome::Denied,
        reason_code,
        safe_message: message.into(),
        host_function,
        target_class: request.target_class.clone(),
        normalized_path: None,
        required_ucan_ability: required_ability,
        required_ucan_resource: required_resource,
        safe_ucan_metadata: None,
        preflight_profile: None,
        verification_profile: None,
        rollback_required: false,
    }
}

fn deny_with_target(
    request: &SteelMutationRequest,
    target: &SteelMutationTargetClass,
    reason_code: SteelMutationReasonCode,
    message: impl Into<String>,
    verb: Option<&SteelMutationVerbPolicy>,
    normalized_path: Option<String>,
) -> SteelMutationDecision {
    let required_resource = verb.map(|_| format!("{}{}", target.resource_prefix, request.resource));
    let required_ability = verb.map(|policy| policy.ucan_ability.clone());
    SteelMutationDecision {
        schema: STEEL_MUTATION_DECISION_SCHEMA.to_string(),
        outcome: SteelMutationDecisionOutcome::Denied,
        reason_code,
        safe_message: message.into(),
        host_function: verb.map(|policy| policy.host_function.clone()),
        target_class: target.name.clone(),
        normalized_path,
        required_ucan_ability: required_ability,
        required_ucan_resource: required_resource,
        safe_ucan_metadata: None,
        preflight_profile: Some(target.preflight_profile.clone()),
        verification_profile: Some(target.verification_profile.clone()),
        rollback_required: target.rollback_required,
    }
}

impl SteelMutationDecision {
    #[must_use]
    fn with_safe_ucan_metadata(mut self, metadata: Option<SteelMutationSafeUcanMetadata>) -> Self {
        self.safe_ucan_metadata = metadata;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPORTED_POLICY: &str = include_str!("../../../policy/steel-self-mutation/mutation-policy.json");
    const INVALID_POLICY: &str = include_str!("../../../policy/steel-self-mutation/invalid-policy.json");
    const TEST_REVIEWER: &str = "reviewer:test";
    const TEST_AUDIENCE: &str = "session:test";
    const TEST_PATCH_BYTES: u64 = 42;

    fn policy() -> SteelMutationPolicy {
        parse_steel_mutation_policy(EXPORTED_POLICY).expect("exported policy parses")
    }

    fn base_request() -> SteelMutationRequest {
        SteelMutationRequest {
            target_class: "prompt".to_string(),
            verb: "apply_mutation".to_string(),
            resource: "agent-system".to_string(),
            expected_audience: TEST_AUDIENCE.to_string(),
            relative_path: "crates/clankers-prompts/src/lib.rs".to_string(),
            intent: "tighten prompt fixture".to_string(),
            patch: Some(SteelMutationPatch {
                format: SteelMutationPatchFormat::UnifiedDiff,
                bytes: TEST_PATCH_BYTES,
                body_blake3: "b3:patch".to_string(),
            }),
            approval: SteelMutationApproval {
                approved: true,
                tier: "human-review".to_string(),
                reviewer: Some(TEST_REVIEWER.to_string()),
            },
            ucan: Some(SteelMutationUcanGrant {
                ability: "clankers/steel/mutation.apply".to_string(),
                resource: "prompt:agent-system".to_string(),
                audience: TEST_AUDIENCE.to_string(),
                expiry_status: SteelMutationUcanExpiryStatus::Valid,
                delegation_depth: 1,
                revoked: false,
            }),
        }
    }

    fn host_context() -> SteelMutationHostContext {
        SteelMutationHostContext::new(ArtifactHash::digest(EXPORTED_POLICY.as_bytes()))
            .with_session_capabilities([STEEL_MUTATION_SESSION_CAPABILITY, WORKSPACE_MUTATION_SESSION_CAPABILITY])
            .with_target_hash(ArtifactHash::digest(b"current target bytes"))
    }

    fn full_replace_request(new_body: &[u8]) -> SteelMutationRequest {
        let mut request = base_request();
        request.patch = Some(SteelMutationPatch {
            format: SteelMutationPatchFormat::FullReplace,
            bytes: new_body.len() as u64,
            body_blake3: ArtifactHash::digest(new_body).prefixed(),
        });
        request
    }

    struct MemoryTargetStore {
        path: String,
        bytes: Vec<u8>,
        fail_write: bool,
    }

    impl MemoryTargetStore {
        fn new(path: &str, bytes: &[u8]) -> Self {
            Self {
                path: path.to_string(),
                bytes: bytes.to_vec(),
                fail_write: false,
            }
        }
    }

    impl SteelMutationTargetStore for MemoryTargetStore {
        fn read_target(&self, normalized_path: &str) -> Result<Vec<u8>, SteelMutationIoError> {
            if normalized_path == self.path {
                Ok(self.bytes.clone())
            } else {
                Err(SteelMutationIoError::Unavailable)
            }
        }

        fn write_target(&mut self, normalized_path: &str, bytes: &[u8]) -> Result<(), SteelMutationIoError> {
            if self.fail_write {
                return Err(SteelMutationIoError::Failed {
                    message: "forced write failure".to_string(),
                });
            }
            if normalized_path != self.path {
                return Err(SteelMutationIoError::Unavailable);
            }
            self.bytes = bytes.to_vec();
            Ok(())
        }
    }

    struct MemoryBackupStore {
        bytes: Vec<u8>,
    }

    impl SteelMutationBackupStore for MemoryBackupStore {
        fn read_backup(&self, _backup_hash: ArtifactHash) -> Result<Vec<u8>, SteelMutationIoError> {
            Ok(self.bytes.clone())
        }
    }

    struct FixedVerifier {
        status: SteelMutationVerificationStatus,
    }

    impl SteelMutationVerifier for FixedVerifier {
        fn verify(&self, profile: &str, normalized_path: &str) -> SteelMutationVerificationReceipt {
            SteelMutationVerificationReceipt {
                profile: Some(profile.to_string()),
                status: self.status.clone(),
                safe_summary: format!("{profile} checked {normalized_path}"),
            }
        }
    }

    #[test]
    fn apply_shell_writes_full_replace_and_records_backup_and_verification_hashes() {
        let new_body = b"updated prompt bytes";
        let request = full_replace_request(new_body);
        let mut store = MemoryTargetStore::new("crates/clankers-prompts/src/lib.rs", b"current target bytes");
        let verifier = FixedVerifier {
            status: SteelMutationVerificationStatus::Passed,
        };

        let receipt = apply_steel_mutation_host_function(
            &policy(),
            &request,
            &host_context(),
            &SteelMutationPatchPayload::full_replace(new_body.to_vec()),
            &mut store,
            &verifier,
        );

        assert_eq!(receipt.schema, STEEL_MUTATION_APPLY_SCHEMA);
        assert_eq!(receipt.status, SteelMutationApplyStatus::Applied);
        assert_eq!(receipt.reason_code, SteelMutationApplyReason::Applied);
        assert!(receipt.writes_performed);
        assert_eq!(store.bytes, new_body);
        assert_eq!(receipt.target_hash_before, Some(ArtifactHash::digest(b"current target bytes")));
        assert_eq!(receipt.backup_hash, Some(ArtifactHash::digest(b"current target bytes")));
        assert_eq!(receipt.patch_hash, Some(ArtifactHash::digest(new_body)));
        assert_eq!(receipt.target_hash_after, Some(ArtifactHash::digest(new_body)));
        assert_eq!(receipt.verification.status, SteelMutationVerificationStatus::Passed);
        assert!(receipt.receipt_hash().prefixed().starts_with("b3:"));
    }

    #[test]
    fn apply_shell_blocks_payload_hash_mismatch_before_write() {
        let mut store = MemoryTargetStore::new("crates/clankers-prompts/src/lib.rs", b"current target bytes");
        let verifier = FixedVerifier {
            status: SteelMutationVerificationStatus::Passed,
        };

        let receipt = apply_steel_mutation_host_function(
            &policy(),
            &full_replace_request(b"aaaaaaaa"),
            &host_context(),
            &SteelMutationPatchPayload::full_replace(b"bbbbbbbb".to_vec()),
            &mut store,
            &verifier,
        );

        assert_eq!(receipt.status, SteelMutationApplyStatus::Blocked);
        assert_eq!(receipt.reason_code, SteelMutationApplyReason::PatchHashMismatch);
        assert!(!receipt.writes_performed);
        assert_eq!(store.bytes, b"current target bytes");
    }

    #[test]
    fn apply_shell_blocks_stale_target_hash_before_write() {
        let new_body = b"updated prompt bytes";
        let request = full_replace_request(new_body);
        let mut store = MemoryTargetStore::new("crates/clankers-prompts/src/lib.rs", b"operator edited bytes");
        let verifier = FixedVerifier {
            status: SteelMutationVerificationStatus::Passed,
        };

        let receipt = apply_steel_mutation_host_function(
            &policy(),
            &request,
            &host_context(),
            &SteelMutationPatchPayload::full_replace(new_body.to_vec()),
            &mut store,
            &verifier,
        );

        assert_eq!(receipt.status, SteelMutationApplyStatus::Blocked);
        assert_eq!(receipt.reason_code, SteelMutationApplyReason::StaleTargetHash);
        assert!(!receipt.writes_performed);
        assert_eq!(store.bytes, b"operator edited bytes");
    }

    #[test]
    fn apply_shell_records_failed_verification_after_write_without_success_claim() {
        let new_body = b"updated prompt bytes";
        let request = full_replace_request(new_body);
        let mut store = MemoryTargetStore::new("crates/clankers-prompts/src/lib.rs", b"current target bytes");
        let verifier = FixedVerifier {
            status: SteelMutationVerificationStatus::Failed,
        };

        let receipt = apply_steel_mutation_host_function(
            &policy(),
            &request,
            &host_context(),
            &SteelMutationPatchPayload::full_replace(new_body.to_vec()),
            &mut store,
            &verifier,
        );

        assert_eq!(receipt.status, SteelMutationApplyStatus::FailedVerification);
        assert_eq!(receipt.reason_code, SteelMutationApplyReason::VerificationFailed);
        assert!(receipt.writes_performed);
        assert_eq!(store.bytes, new_body);
        assert_eq!(receipt.verification.status, SteelMutationVerificationStatus::Failed);
        assert_eq!(receipt.backup_hash, Some(ArtifactHash::digest(b"current target bytes")));
    }

    #[test]
    fn rollback_restores_backup_only_after_post_apply_and_backup_hash_match() {
        let new_body = b"updated prompt bytes";
        let request = full_replace_request(new_body);
        let mut store = MemoryTargetStore::new("crates/clankers-prompts/src/lib.rs", b"current target bytes");
        let verifier = FixedVerifier {
            status: SteelMutationVerificationStatus::Passed,
        };
        let apply_receipt = apply_steel_mutation_host_function(
            &policy(),
            &request,
            &host_context(),
            &SteelMutationPatchPayload::full_replace(new_body.to_vec()),
            &mut store,
            &verifier,
        );
        let backup_store = MemoryBackupStore {
            bytes: b"current target bytes".to_vec(),
        };

        let rollback = rollback_steel_mutation_host_function(&apply_receipt, &mut store, &backup_store);

        assert_eq!(rollback.schema, STEEL_MUTATION_ROLLBACK_SCHEMA);
        assert_eq!(rollback.status, SteelMutationRollbackStatus::RolledBack);
        assert_eq!(rollback.reason_code, SteelMutationRollbackReason::RolledBack);
        assert!(rollback.writes_performed);
        assert_eq!(store.bytes, b"current target bytes");
        assert_eq!(rollback.backup_hash, Some(ArtifactHash::digest(b"current target bytes")));
        assert_eq!(rollback.restored_target_hash, Some(ArtifactHash::digest(b"current target bytes")));
        assert!(rollback.receipt_hash().prefixed().starts_with("b3:"));
    }

    #[test]
    fn rollback_blocks_when_target_changed_after_apply() {
        let new_body = b"updated prompt bytes";
        let request = full_replace_request(new_body);
        let mut store = MemoryTargetStore::new("crates/clankers-prompts/src/lib.rs", b"current target bytes");
        let verifier = FixedVerifier {
            status: SteelMutationVerificationStatus::Passed,
        };
        let apply_receipt = apply_steel_mutation_host_function(
            &policy(),
            &request,
            &host_context(),
            &SteelMutationPatchPayload::full_replace(new_body.to_vec()),
            &mut store,
            &verifier,
        );
        store.bytes = b"operator edit after apply".to_vec();
        let backup_store = MemoryBackupStore {
            bytes: b"current target bytes".to_vec(),
        };

        let rollback = rollback_steel_mutation_host_function(&apply_receipt, &mut store, &backup_store);

        assert_eq!(rollback.status, SteelMutationRollbackStatus::Blocked);
        assert_eq!(rollback.reason_code, SteelMutationRollbackReason::CurrentTargetChanged);
        assert!(!rollback.writes_performed);
        assert_eq!(store.bytes, b"operator edit after apply");
    }

    #[test]
    fn rollback_blocks_when_backup_hash_does_not_match_receipt() {
        let new_body = b"updated prompt bytes";
        let request = full_replace_request(new_body);
        let mut store = MemoryTargetStore::new("crates/clankers-prompts/src/lib.rs", b"current target bytes");
        let verifier = FixedVerifier {
            status: SteelMutationVerificationStatus::Passed,
        };
        let apply_receipt = apply_steel_mutation_host_function(
            &policy(),
            &request,
            &host_context(),
            &SteelMutationPatchPayload::full_replace(new_body.to_vec()),
            &mut store,
            &verifier,
        );
        let backup_store = MemoryBackupStore {
            bytes: b"different backup bytes".to_vec(),
        };

        let rollback = rollback_steel_mutation_host_function(&apply_receipt, &mut store, &backup_store);

        assert_eq!(rollback.status, SteelMutationRollbackStatus::Blocked);
        assert_eq!(rollback.reason_code, SteelMutationRollbackReason::BackupHashMismatch);
        assert!(!rollback.writes_performed);
        assert_eq!(store.bytes, new_body);
    }

    #[test]
    fn preflight_receipt_is_ready_without_writing_when_policy_ucan_and_session_capabilities_pass() {
        let receipt = preflight_steel_mutation_host_function(&policy(), &base_request(), &host_context());

        assert_eq!(receipt.schema, STEEL_MUTATION_PREFLIGHT_SCHEMA);
        assert_eq!(receipt.status, SteelMutationHostPreflightStatus::Ready);
        assert_eq!(receipt.reason_code, SteelMutationHostPreflightReason::Ready);
        assert_eq!(receipt.host_function.as_deref(), Some("steel.host.apply_mutation"));
        assert_eq!(receipt.normalized_path.as_deref(), Some("crates/clankers-prompts/src/lib.rs"));
        assert_eq!(receipt.verification_profile.as_deref(), Some("prompt-schema-and-smoke"));
        assert!(receipt.checkpoint.required);
        assert_eq!(receipt.checkpoint.target_hash, host_context().target_hash);
        assert!(!receipt.writes_performed);
        assert!(receipt.receipt_hash().prefixed().starts_with("b3:"));
    }

    #[test]
    fn preflight_blocks_when_session_lacks_mutation_capability() {
        let context = SteelMutationHostContext::new(ArtifactHash::digest(EXPORTED_POLICY.as_bytes()))
            .with_session_capabilities([WORKSPACE_MUTATION_SESSION_CAPABILITY])
            .with_target_hash(ArtifactHash::digest(b"current target bytes"));
        let receipt = preflight_steel_mutation_host_function(&policy(), &base_request(), &context);

        assert_eq!(receipt.status, SteelMutationHostPreflightStatus::Blocked);
        assert_eq!(receipt.reason_code, SteelMutationHostPreflightReason::MissingSessionCapability);
        assert_eq!(receipt.decision.outcome, SteelMutationDecisionOutcome::Allowed);
        assert!(!receipt.writes_performed);
    }

    #[test]
    fn preflight_blocks_disabled_host_function_and_dirty_uncheckpointed_repo() {
        let disabled_context = host_context().with_disabled_tools(["steel.host.apply_mutation"]);
        let disabled = preflight_steel_mutation_host_function(&policy(), &base_request(), &disabled_context);
        assert_eq!(disabled.status, SteelMutationHostPreflightStatus::Blocked);
        assert_eq!(disabled.reason_code, SteelMutationHostPreflightReason::DisabledHostFunction);

        let dirty_context = host_context().with_dirty_repository(None);
        let dirty = preflight_steel_mutation_host_function(&policy(), &base_request(), &dirty_context);
        assert_eq!(dirty.status, SteelMutationHostPreflightStatus::Blocked);
        assert_eq!(dirty.reason_code, SteelMutationHostPreflightReason::DirtyRepositoryNeedsCheckpoint);
        assert_eq!(dirty.checkpoint.checkpoint_id, None);
    }

    #[test]
    fn preflight_blocks_rollback_required_request_without_target_hash() {
        let context = SteelMutationHostContext::new(ArtifactHash::digest(EXPORTED_POLICY.as_bytes()))
            .with_session_capabilities([STEEL_MUTATION_SESSION_CAPABILITY, WORKSPACE_MUTATION_SESSION_CAPABILITY]);
        let receipt = preflight_steel_mutation_host_function(&policy(), &base_request(), &context);

        assert_eq!(receipt.status, SteelMutationHostPreflightStatus::Blocked);
        assert_eq!(receipt.reason_code, SteelMutationHostPreflightReason::MissingTargetHash);
        assert!(receipt.checkpoint.required);
        assert_eq!(receipt.target_hash, None);
        assert!(!receipt.writes_performed);
    }

    #[test]
    fn prompt_apply_request_is_authorized_for_rust_host_preflight() {
        let decision = authorize_steel_mutation(&policy(), &base_request());

        assert_eq!(decision.outcome, SteelMutationDecisionOutcome::Allowed);
        assert_eq!(decision.reason_code, SteelMutationReasonCode::Allowed);
        assert_eq!(decision.host_function.as_deref(), Some("steel.host.apply_mutation"));
        assert_eq!(decision.normalized_path.as_deref(), Some("crates/clankers-prompts/src/lib.rs"));
        assert_eq!(decision.required_ucan_ability.as_deref(), Some("clankers/steel/mutation.apply"));
        assert_eq!(decision.required_ucan_resource.as_deref(), Some("prompt:agent-system"));
        assert_eq!(decision.verification_profile.as_deref(), Some("prompt-schema-and-smoke"));
        assert!(decision.rollback_required);
        assert_eq!(
            decision.safe_ucan_metadata.as_ref().map(|metadata| &metadata.authorization_outcome),
            Some(&SteelMutationDecisionOutcome::Allowed)
        );
    }

    #[test]
    fn raw_path_escape_is_denied_before_ucan_success() {
        let mut request = base_request();
        request.relative_path = "../secrets.env".to_string();
        let decision = authorize_steel_mutation(&policy(), &request);

        assert_eq!(decision.outcome, SteelMutationDecisionOutcome::Denied);
        assert_eq!(decision.reason_code, SteelMutationReasonCode::PathEscape);
        assert_eq!(decision.safe_ucan_metadata, None);
    }

    #[test]
    fn wildcard_ucan_resource_is_denied() {
        let mut request = base_request();
        request.ucan.as_mut().expect("ucan").resource = "*".to_string();
        let decision = authorize_steel_mutation(&policy(), &request);

        assert_eq!(decision.outcome, SteelMutationDecisionOutcome::Denied);
        assert_eq!(decision.reason_code, SteelMutationReasonCode::WildcardUcanResource);
        assert_eq!(
            decision.safe_ucan_metadata.as_ref().map(|metadata| &metadata.authorization_outcome),
            Some(&SteelMutationDecisionOutcome::Denied)
        );
    }

    #[test]
    fn expired_revoked_wrong_audience_and_over_delegated_ucans_are_denied() {
        for (mut grant, expected) in [
            (
                SteelMutationUcanGrant {
                    ability: "clankers/steel/mutation.apply".to_string(),
                    resource: "prompt:agent-system".to_string(),
                    audience: TEST_AUDIENCE.to_string(),
                    expiry_status: SteelMutationUcanExpiryStatus::Expired,
                    delegation_depth: 1,
                    revoked: false,
                },
                SteelMutationReasonCode::ExpiredUcan,
            ),
            (
                SteelMutationUcanGrant {
                    ability: "clankers/steel/mutation.apply".to_string(),
                    resource: "prompt:agent-system".to_string(),
                    audience: TEST_AUDIENCE.to_string(),
                    expiry_status: SteelMutationUcanExpiryStatus::Valid,
                    delegation_depth: 1,
                    revoked: true,
                },
                SteelMutationReasonCode::RevokedUcan,
            ),
            (
                SteelMutationUcanGrant {
                    ability: "clankers/steel/mutation.apply".to_string(),
                    resource: "prompt:agent-system".to_string(),
                    audience: "session:other".to_string(),
                    expiry_status: SteelMutationUcanExpiryStatus::Valid,
                    delegation_depth: 1,
                    revoked: false,
                },
                SteelMutationReasonCode::WrongUcanAudience,
            ),
            (
                SteelMutationUcanGrant {
                    ability: "clankers/steel/mutation.apply".to_string(),
                    resource: "prompt:agent-system".to_string(),
                    audience: TEST_AUDIENCE.to_string(),
                    expiry_status: SteelMutationUcanExpiryStatus::Valid,
                    delegation_depth: 2,
                    revoked: false,
                },
                SteelMutationReasonCode::OverDelegatedUcan,
            ),
        ] {
            let mut request = base_request();
            request.ucan = Some(grant.clone());
            let decision = authorize_steel_mutation(&policy(), &request);

            assert_eq!(decision.outcome, SteelMutationDecisionOutcome::Denied);
            assert_eq!(decision.reason_code, expected);
            assert_eq!(
                decision.safe_ucan_metadata.as_ref().map(|metadata| &metadata.authorization_outcome),
                Some(&SteelMutationDecisionOutcome::Denied)
            );
            grant.revoked = false;
        }
    }

    #[test]
    fn wrong_ability_and_wrong_resource_ucans_are_denied() {
        for (field, expected) in [
            ("ability", SteelMutationReasonCode::WrongUcanAbility),
            ("resource", SteelMutationReasonCode::WrongUcanResource),
        ] {
            let mut request = base_request();
            let ucan = request.ucan.as_mut().expect("ucan");
            if field == "ability" {
                ucan.ability = "clankers/steel/mutation.propose".to_string();
            } else {
                ucan.resource = "prompt:other".to_string();
            }
            let decision = authorize_steel_mutation(&policy(), &request);

            assert_eq!(decision.outcome, SteelMutationDecisionOutcome::Denied);
            assert_eq!(decision.reason_code, expected);
        }
    }

    #[test]
    fn missing_approval_is_denied_for_byte_writing_apply() {
        let mut request = base_request();
        request.approval.approved = false;
        let decision = authorize_steel_mutation(&policy(), &request);

        assert_eq!(decision.outcome, SteelMutationDecisionOutcome::Denied);
        assert_eq!(decision.reason_code, SteelMutationReasonCode::MissingApproval);
    }

    #[test]
    fn missing_patch_is_denied_for_byte_writing_apply() {
        let mut request = base_request();
        request.patch = None;
        let decision = authorize_steel_mutation(&policy(), &request);

        assert_eq!(decision.outcome, SteelMutationDecisionOutcome::Denied);
        assert_eq!(decision.reason_code, SteelMutationReasonCode::MissingPatch);
    }

    #[test]
    fn unsafe_exported_policy_fixture_fails_closed() {
        let unsafe_policy = parse_steel_mutation_policy(INVALID_POLICY).expect("invalid fixture still parses as DTO");
        let mut request = base_request();
        request.target_class = "repo_code".to_string();
        request.verb = "raw_write".to_string();
        request.relative_path = "src/main.rs".to_string();
        request.resource = "src/main.rs".to_string();
        request.ucan.as_mut().expect("ucan").ability = "*".to_string();
        request.ucan.as_mut().expect("ucan").resource = "*".to_string();

        let decision = authorize_steel_mutation(&unsafe_policy, &request);

        assert_eq!(decision.outcome, SteelMutationDecisionOutcome::Denied);
        assert_eq!(decision.reason_code, SteelMutationReasonCode::InvalidPolicy);
    }
}
