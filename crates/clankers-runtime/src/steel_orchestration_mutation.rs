//! Steel orchestration-pack mutation DTOs and pure validation core.
//!
//! This module lets Steel describe changes to repo-local orchestration packs.
//! Rust validates, stages, gates, activates later, and rolls back with hashes.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use clankers_artifacts::ArtifactHash;
use serde::Deserialize;
use serde::Serialize;

pub const STEEL_ORCHESTRATION_PATCH_SCHEMA: &str = "clankers.steel.orchestration-patch.v1";
pub const STEEL_ORCHESTRATION_MUTATION_RECEIPT_SCHEMA: &str = "clankers.steel.orchestration-pack-mutation.receipt.v1";
pub const STEEL_ORCHESTRATION_PACK_ROOT: &str = ".clankers/steel";
const ACTIVATION_EXPLICIT_RELOAD: &str = "explicit_reload";
const ACTIVATION_NEXT_TURN: &str = "next_turn";
const REQUIRED_GATE_PREFIX: &str = "steel-pack-";
const REDACTED_INVALID_PATCH_HASH: &str = "redacted:invalid-patch-hash";
const REDACTED_UNSAFE_TARGET_PATH: &str = "redacted:target-path";
const RAW_WRITE_AUTHORITY_CLASS: &str = "raw_write";
const UNKNOWN_AUTHORITY_CLASS: &str = "authority_change";

const RAW_HOST_AUTHORITY_PREFIXES: &[(&str, &str)] = &[
    ("raw_write:", RAW_WRITE_AUTHORITY_CLASS),
    ("write_file:", RAW_WRITE_AUTHORITY_CLASS),
    ("fs.write:", RAW_WRITE_AUTHORITY_CLASS),
    ("raw_shell:", "raw_shell"),
    ("shell:", "raw_shell"),
    ("git:", "git"),
    ("network:", "network"),
    ("provider:", "provider"),
    ("credential:", "credential"),
    ("daemon:", "daemon"),
    ("tui:", "tui"),
    ("native_tool:", "native_tool"),
    ("session_mutation:", "session_mutation"),
    ("capability_mint:", "capability_mint"),
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelOrchestrationPatchProposal {
    pub schema: String,
    pub intent: String,
    pub target_paths: Vec<String>,
    pub expected_pack_hash: String,
    pub patch_hash: String,
    pub selected_gates: Vec<String>,
    pub activation_policy: String,
    pub authority_changes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelOrchestrationMutationReceipt {
    pub schema: String,
    pub status: SteelOrchestrationMutationStatus,
    pub reason_code: SteelOrchestrationMutationReason,
    pub safe_message: String,
    pub old_pack_hash: ArtifactHash,
    pub proposed_new_pack_hash: Option<ArtifactHash>,
    pub patch_hash: Option<String>,
    pub target_paths: Vec<String>,
    pub selected_gates: Vec<String>,
    pub gate_result_hashes: Vec<ArtifactHash>,
    pub activation_decision: SteelOrchestrationActivationDecision,
    pub rollback_reference: Option<SteelOrchestrationRollbackReference>,
    pub writes_performed: bool,
    pub authority_changes: Vec<String>,
}

impl SteelOrchestrationMutationReceipt {
    #[must_use]
    pub fn receipt_hash(&self) -> ArtifactHash {
        let bytes = serde_json::to_vec(self).expect("orchestration mutation receipt serializes");
        ArtifactHash::digest(&bytes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelOrchestrationMutationStatus {
    Ready,
    Staged,
    Promoted,
    RolledBack,
    Denied,
    FailedValidation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SteelOrchestrationMutationReason {
    Ready,
    Staged,
    Promoted,
    RolledBack,
    InvalidSchema,
    MalformedPatchHash,
    PathEscape,
    StalePackHash,
    RawHostWriteDenied,
    AuthorityKernelChange,
    RequiredGateRemoval,
    UnknownActivationPolicy,
    GateFailed,
    CurrentPackChanged,
    BackupHashMismatch,
    ApplyFailed,
    RollbackFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelOrchestrationActivationDecision {
    Denied,
    StagedOnly,
    NextTurn,
    ExplicitReload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelOrchestrationRollbackReference {
    pub pre_apply_hash: ArtifactHash,
    pub post_apply_hash: ArtifactHash,
    pub backup_hash: ArtifactHash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteelOrchestrationPackState {
    pub pack_hash: ArtifactHash,
    pub required_gates: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteelOrchestrationGateResult {
    pub name: String,
    pub passed: bool,
    pub receipt_hash: ArtifactHash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteelOrchestrationPatchPayload {
    pub target_path: String,
    pub bytes: Vec<u8>,
}

#[must_use]
pub fn validate_orchestration_patch_proposal(
    proposal: &SteelOrchestrationPatchProposal,
    state: &SteelOrchestrationPackState,
) -> SteelOrchestrationMutationReceipt {
    if proposal.schema != STEEL_ORCHESTRATION_PATCH_SCHEMA {
        return denied_receipt(
            state,
            proposal,
            SteelOrchestrationMutationReason::InvalidSchema,
            "orchestration patch schema is unsupported",
        );
    }
    if proposal.expected_pack_hash != state.pack_hash.prefixed() {
        return denied_receipt(
            state,
            proposal,
            SteelOrchestrationMutationReason::StalePackHash,
            "orchestration patch expected pack hash is stale",
        );
    }
    if !proposal.patch_hash.starts_with("b3:") {
        return denied_receipt(
            state,
            proposal,
            SteelOrchestrationMutationReason::MalformedPatchHash,
            "orchestration patch hash must be a BLAKE3 receipt hash",
        );
    }
    if proposal.target_paths.iter().any(|path| !pack_path_allowed(path)) {
        return denied_receipt(
            state,
            proposal,
            SteelOrchestrationMutationReason::PathEscape,
            "orchestration patch target escapes the repo-local Steel pack root",
        );
    }
    if proposal.authority_changes.iter().any(|change| raw_host_write_attempt(change)) {
        return denied_receipt(
            state,
            proposal,
            SteelOrchestrationMutationReason::RawHostWriteDenied,
            "orchestration patch requests raw host write authority and is denied",
        );
    }
    if !proposal.authority_changes.is_empty() {
        return denied_receipt(
            state,
            proposal,
            SteelOrchestrationMutationReason::AuthorityKernelChange,
            "orchestration patch requests authority-kernel changes and needs a checkpoint",
        );
    }
    if removes_required_gate(&proposal.selected_gates, &state.required_gates) {
        return denied_receipt(
            state,
            proposal,
            SteelOrchestrationMutationReason::RequiredGateRemoval,
            "orchestration patch removes a required Steel pack validation gate",
        );
    }
    if activation_decision(&proposal.activation_policy).is_none() {
        return denied_receipt(
            state,
            proposal,
            SteelOrchestrationMutationReason::UnknownActivationPolicy,
            "orchestration patch activation policy is unsupported",
        );
    }
    SteelOrchestrationMutationReceipt {
        schema: STEEL_ORCHESTRATION_MUTATION_RECEIPT_SCHEMA.to_string(),
        status: SteelOrchestrationMutationStatus::Ready,
        reason_code: SteelOrchestrationMutationReason::Ready,
        safe_message: "orchestration patch proposal is ready for isolated staging".to_string(),
        old_pack_hash: state.pack_hash,
        proposed_new_pack_hash: None,
        patch_hash: Some(proposal.patch_hash.clone()),
        target_paths: sorted(proposal.target_paths.clone()),
        selected_gates: sorted(proposal.selected_gates.clone()),
        gate_result_hashes: Vec::new(),
        activation_decision: SteelOrchestrationActivationDecision::StagedOnly,
        rollback_reference: None,
        writes_performed: false,
        authority_changes: Vec::new(),
    }
}

pub fn stage_orchestration_patch_to_directory(
    proposal: &SteelOrchestrationPatchProposal,
    state: &SteelOrchestrationPackState,
    stage_root: &Path,
    payloads: &[SteelOrchestrationPatchPayload],
    gate_results: &[SteelOrchestrationGateResult],
) -> SteelOrchestrationMutationReceipt {
    let preflight = validate_orchestration_patch_proposal(proposal, state);
    if preflight.status != SteelOrchestrationMutationStatus::Ready {
        return preflight;
    }
    if payloads_cover_targets(payloads, &proposal.target_paths).is_err() {
        return SteelOrchestrationMutationReceipt {
            status: SteelOrchestrationMutationStatus::Denied,
            reason_code: SteelOrchestrationMutationReason::PathEscape,
            safe_message: "orchestration patch payloads do not match validated targets".to_string(),
            writes_performed: false,
            ..preflight
        };
    }
    let mut staged_pack_bytes = Vec::new();
    let mut sorted_payloads = payloads.iter().collect::<Vec<_>>();
    sorted_payloads.sort_by(|left, right| left.target_path.cmp(&right.target_path));
    for payload in sorted_payloads {
        if !pack_path_allowed(&payload.target_path) {
            return SteelOrchestrationMutationReceipt {
                status: SteelOrchestrationMutationStatus::Denied,
                reason_code: SteelOrchestrationMutationReason::PathEscape,
                safe_message: "orchestration patch payload target escapes the repo-local Steel pack root".to_string(),
                writes_performed: false,
                ..preflight
            };
        }
        let target = stage_root.join(&payload.target_path);
        let Some(parent) = target.parent() else {
            return SteelOrchestrationMutationReceipt {
                status: SteelOrchestrationMutationStatus::Denied,
                reason_code: SteelOrchestrationMutationReason::PathEscape,
                safe_message: "orchestration patch target has no parent directory".to_string(),
                writes_performed: false,
                ..preflight
            };
        };
        if fs::create_dir_all(parent).is_err() || fs::write(&target, &payload.bytes).is_err() {
            return SteelOrchestrationMutationReceipt {
                status: SteelOrchestrationMutationStatus::FailedValidation,
                reason_code: SteelOrchestrationMutationReason::GateFailed,
                safe_message: "orchestration patch could not be written to isolated staging".to_string(),
                writes_performed: false,
                ..preflight
            };
        }
        staged_pack_bytes.extend_from_slice(payload.target_path.as_bytes());
        staged_pack_bytes.push(0);
        staged_pack_bytes.extend_from_slice(&payload.bytes);
        staged_pack_bytes.push(0);
    }
    stage_orchestration_patch_after_isolated_apply(proposal, state, &staged_pack_bytes, gate_results)
}

#[must_use]
pub fn stage_orchestration_patch_after_isolated_apply(
    proposal: &SteelOrchestrationPatchProposal,
    state: &SteelOrchestrationPackState,
    staged_pack_bytes: &[u8],
    gate_results: &[SteelOrchestrationGateResult],
) -> SteelOrchestrationMutationReceipt {
    let preflight = validate_orchestration_patch_proposal(proposal, state);
    if preflight.status != SteelOrchestrationMutationStatus::Ready {
        return preflight;
    }
    let failed_gates = gate_results.iter().filter(|gate| !gate.passed).collect::<Vec<_>>();
    let gate_result_hashes = gate_results.iter().map(|gate| gate.receipt_hash).collect::<Vec<_>>();
    if !failed_gates.is_empty() {
        return SteelOrchestrationMutationReceipt {
            status: SteelOrchestrationMutationStatus::FailedValidation,
            reason_code: SteelOrchestrationMutationReason::GateFailed,
            safe_message: "orchestration patch staged in isolation but validation gates failed".to_string(),
            proposed_new_pack_hash: Some(ArtifactHash::digest(staged_pack_bytes)),
            gate_result_hashes,
            ..preflight
        };
    }
    let new_pack_hash = ArtifactHash::digest(staged_pack_bytes);
    SteelOrchestrationMutationReceipt {
        status: SteelOrchestrationMutationStatus::Staged,
        reason_code: SteelOrchestrationMutationReason::Staged,
        safe_message: "orchestration patch staged in isolation and gates passed".to_string(),
        proposed_new_pack_hash: Some(new_pack_hash),
        gate_result_hashes,
        activation_decision: activation_decision(&proposal.activation_policy)
            .unwrap_or(SteelOrchestrationActivationDecision::StagedOnly),
        rollback_reference: Some(SteelOrchestrationRollbackReference {
            pre_apply_hash: state.pack_hash,
            post_apply_hash: new_pack_hash,
            backup_hash: state.pack_hash,
        }),
        writes_performed: false,
        ..preflight
    }
}

#[must_use]
pub fn promote_staged_orchestration_pack(
    staged: &SteelOrchestrationMutationReceipt,
) -> SteelOrchestrationMutationReceipt {
    if staged.status != SteelOrchestrationMutationStatus::Staged {
        return SteelOrchestrationMutationReceipt {
            status: SteelOrchestrationMutationStatus::Denied,
            reason_code: staged.reason_code,
            safe_message: "only successfully staged orchestration packs can be promoted".to_string(),
            writes_performed: false,
            ..staged.clone()
        };
    }
    SteelOrchestrationMutationReceipt {
        status: SteelOrchestrationMutationStatus::Promoted,
        reason_code: SteelOrchestrationMutationReason::Promoted,
        safe_message: "orchestration pack promotion is allowed after explicit reload or later turn".to_string(),
        writes_performed: false,
        ..staged.clone()
    }
}

pub fn promote_staged_orchestration_pack_to_directory(
    staged: &SteelOrchestrationMutationReceipt,
    stage_root: &Path,
    live_root: &Path,
    backup_root: &Path,
) -> SteelOrchestrationMutationReceipt {
    let mut promoted = promote_staged_orchestration_pack(staged);
    if promoted.status != SteelOrchestrationMutationStatus::Promoted {
        return promoted;
    }
    let Some(rollback) = promoted.rollback_reference.clone() else {
        return apply_failed_receipt(&promoted, "orchestration mutation receipt has no rollback reference");
    };
    if !target_paths_allowed(&promoted.target_paths) {
        return apply_failed_receipt(&promoted, "orchestration mutation receipt target path escapes pack root");
    }
    let Ok(current_hash) = hash_target_paths(live_root, &promoted.target_paths) else {
        return apply_failed_receipt(&promoted, "orchestration live pack could not be read before promotion");
    };
    if current_hash != rollback.pre_apply_hash {
        return SteelOrchestrationMutationReceipt {
            status: SteelOrchestrationMutationStatus::Denied,
            reason_code: SteelOrchestrationMutationReason::CurrentPackChanged,
            safe_message: "orchestration live pack changed before promotion; promotion refused".to_string(),
            writes_performed: false,
            ..promoted
        };
    }
    let Ok(staged_hash) = hash_target_paths(stage_root, &promoted.target_paths) else {
        return apply_failed_receipt(&promoted, "orchestration staged pack could not be read before promotion");
    };
    if staged_hash != rollback.post_apply_hash {
        return apply_failed_receipt(&promoted, "orchestration staged pack hash does not match receipt");
    }
    if copy_target_paths(live_root, backup_root, &promoted.target_paths).is_err()
        || copy_target_paths(stage_root, live_root, &promoted.target_paths).is_err()
    {
        return apply_failed_receipt(&promoted, "orchestration staged pack could not be promoted to live files");
    }
    promoted.writes_performed = true;
    promoted.safe_message = "orchestration pack promoted after isolated staging and hash guards".to_string();
    promoted
}

#[must_use]
pub fn rollback_orchestration_pack(
    receipt: &SteelOrchestrationMutationReceipt,
    current_pack_hash: ArtifactHash,
    backup_hash: ArtifactHash,
) -> SteelOrchestrationMutationReceipt {
    let Some(rollback) = receipt.rollback_reference.clone() else {
        return SteelOrchestrationMutationReceipt {
            status: SteelOrchestrationMutationStatus::Denied,
            reason_code: SteelOrchestrationMutationReason::CurrentPackChanged,
            safe_message: "orchestration mutation receipt has no rollback reference".to_string(),
            writes_performed: false,
            ..receipt.clone()
        };
    };
    if current_pack_hash != rollback.post_apply_hash {
        return SteelOrchestrationMutationReceipt {
            status: SteelOrchestrationMutationStatus::Denied,
            reason_code: SteelOrchestrationMutationReason::CurrentPackChanged,
            safe_message: "orchestration pack changed after mutation; rollback refused".to_string(),
            writes_performed: false,
            ..receipt.clone()
        };
    }
    if backup_hash != rollback.backup_hash {
        return SteelOrchestrationMutationReceipt {
            status: SteelOrchestrationMutationStatus::Denied,
            reason_code: SteelOrchestrationMutationReason::BackupHashMismatch,
            safe_message: "orchestration pack backup hash does not match rollback receipt".to_string(),
            writes_performed: false,
            ..receipt.clone()
        };
    }
    SteelOrchestrationMutationReceipt {
        status: SteelOrchestrationMutationStatus::RolledBack,
        reason_code: SteelOrchestrationMutationReason::RolledBack,
        safe_message: "orchestration pack rollback passed post-apply and backup hash guards".to_string(),
        proposed_new_pack_hash: Some(backup_hash),
        activation_decision: SteelOrchestrationActivationDecision::ExplicitReload,
        writes_performed: false,
        ..receipt.clone()
    }
}

pub fn rollback_orchestration_pack_to_directory(
    receipt: &SteelOrchestrationMutationReceipt,
    live_root: &Path,
    backup_root: &Path,
) -> SteelOrchestrationMutationReceipt {
    let Some(rollback) = receipt.rollback_reference.clone() else {
        return SteelOrchestrationMutationReceipt {
            status: SteelOrchestrationMutationStatus::Denied,
            reason_code: SteelOrchestrationMutationReason::CurrentPackChanged,
            safe_message: "orchestration mutation receipt has no rollback reference".to_string(),
            writes_performed: false,
            ..receipt.clone()
        };
    };
    if !target_paths_allowed(&receipt.target_paths) {
        return rollback_failed_receipt(receipt, "orchestration mutation receipt target path escapes pack root");
    }
    let Ok(current_hash) = hash_target_paths(live_root, &receipt.target_paths) else {
        return rollback_failed_receipt(receipt, "orchestration live pack could not be read before rollback");
    };
    let Ok(backup_hash) = hash_target_paths(backup_root, &receipt.target_paths) else {
        return rollback_failed_receipt(receipt, "orchestration backup pack could not be read before rollback");
    };
    let mut rollback_receipt = rollback_orchestration_pack(receipt, current_hash, backup_hash);
    if rollback_receipt.status != SteelOrchestrationMutationStatus::RolledBack {
        return rollback_receipt;
    }
    if backup_hash != rollback.pre_apply_hash
        || copy_target_paths(backup_root, live_root, &receipt.target_paths).is_err()
    {
        return rollback_failed_receipt(receipt, "orchestration backup pack could not be restored to live files");
    }
    rollback_receipt.writes_performed = true;
    rollback_receipt.safe_message = "orchestration pack rollback restored live files after hash guards".to_string();
    rollback_receipt
}

fn denied_receipt(
    state: &SteelOrchestrationPackState,
    proposal: &SteelOrchestrationPatchProposal,
    reason_code: SteelOrchestrationMutationReason,
    message: impl Into<String>,
) -> SteelOrchestrationMutationReceipt {
    SteelOrchestrationMutationReceipt {
        schema: STEEL_ORCHESTRATION_MUTATION_RECEIPT_SCHEMA.to_string(),
        status: SteelOrchestrationMutationStatus::Denied,
        reason_code,
        safe_message: message.into(),
        old_pack_hash: state.pack_hash,
        proposed_new_pack_hash: None,
        patch_hash: safe_patch_hash_for_receipt(&proposal.patch_hash),
        target_paths: safe_target_paths_for_receipt(&proposal.target_paths),
        selected_gates: sorted(proposal.selected_gates.clone()),
        gate_result_hashes: Vec::new(),
        activation_decision: SteelOrchestrationActivationDecision::Denied,
        rollback_reference: None,
        writes_performed: false,
        authority_changes: safe_authority_changes_for_receipt(&proposal.authority_changes),
    }
}

fn safe_patch_hash_for_receipt(patch_hash: &str) -> Option<String> {
    if patch_hash.starts_with("b3:") {
        Some(patch_hash.to_string())
    } else {
        Some(REDACTED_INVALID_PATCH_HASH.to_string())
    }
}

fn safe_target_paths_for_receipt(target_paths: &[String]) -> Vec<String> {
    if target_paths.iter().all(|path| pack_path_allowed(path)) {
        sorted(target_paths.to_vec())
    } else {
        vec![REDACTED_UNSAFE_TARGET_PATH.to_string()]
    }
}

fn safe_authority_changes_for_receipt(authority_changes: &[String]) -> Vec<String> {
    sorted(authority_changes.iter().map(|change| authority_change_class(change).to_string()))
}

fn raw_host_write_attempt(authority_change: &str) -> bool {
    authority_change_class(authority_change) == RAW_WRITE_AUTHORITY_CLASS
}

fn authority_change_class(authority_change: &str) -> &str {
    RAW_HOST_AUTHORITY_PREFIXES
        .iter()
        .find_map(|(prefix, class)| authority_change.starts_with(prefix).then_some(*class))
        .unwrap_or(UNKNOWN_AUTHORITY_CLASS)
}

fn apply_failed_receipt(
    receipt: &SteelOrchestrationMutationReceipt,
    message: &str,
) -> SteelOrchestrationMutationReceipt {
    SteelOrchestrationMutationReceipt {
        status: SteelOrchestrationMutationStatus::FailedValidation,
        reason_code: SteelOrchestrationMutationReason::ApplyFailed,
        safe_message: message.to_string(),
        writes_performed: false,
        ..receipt.clone()
    }
}

fn rollback_failed_receipt(
    receipt: &SteelOrchestrationMutationReceipt,
    message: &str,
) -> SteelOrchestrationMutationReceipt {
    SteelOrchestrationMutationReceipt {
        status: SteelOrchestrationMutationStatus::FailedValidation,
        reason_code: SteelOrchestrationMutationReason::RollbackFailed,
        safe_message: message.to_string(),
        writes_performed: false,
        ..receipt.clone()
    }
}

fn hash_target_paths(root: &Path, target_paths: &[String]) -> Result<ArtifactHash, std::io::Error> {
    let bytes = target_paths_bytes(root, target_paths)?;
    Ok(ArtifactHash::digest(&bytes))
}

fn target_paths_bytes(root: &Path, target_paths: &[String]) -> Result<Vec<u8>, std::io::Error> {
    let mut paths = sorted(target_paths.iter().cloned()).into_iter().collect::<Vec<_>>();
    let mut bytes = Vec::new();
    for path in paths.drain(..) {
        bytes.extend_from_slice(path.as_bytes());
        bytes.push(0);
        bytes.extend_from_slice(&fs::read(root.join(&path))?);
        bytes.push(0);
    }
    Ok(bytes)
}

fn copy_target_paths(from_root: &Path, to_root: &Path, target_paths: &[String]) -> Result<(), std::io::Error> {
    for path in sorted(target_paths.iter().cloned()) {
        let from_path = from_root.join(&path);
        let to_path = to_root.join(&path);
        if let Some(parent) = to_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(from_path, to_path)?;
    }
    Ok(())
}

fn payloads_cover_targets(payloads: &[SteelOrchestrationPatchPayload], target_paths: &[String]) -> Result<(), ()> {
    let payload_paths = payloads.iter().map(|payload| payload.target_path.as_str()).collect::<BTreeSet<_>>();
    let target_paths = target_paths.iter().map(String::as_str).collect::<BTreeSet<_>>();
    if payload_paths == target_paths { Ok(()) } else { Err(()) }
}

fn target_paths_allowed(target_paths: &[String]) -> bool {
    !target_paths.is_empty() && target_paths.iter().all(|path| pack_path_allowed(path))
}

fn pack_path_allowed(path: &str) -> bool {
    path.starts_with(&format!("{STEEL_ORCHESTRATION_PACK_ROOT}/"))
        && !path.contains("..")
        && !path.contains('\\')
        && !path.contains('\0')
        && !path.contains("/.git/")
}

fn removes_required_gate(selected_gates: &[String], required_gates: &[String]) -> bool {
    let selected = selected_gates.iter().map(String::as_str).collect::<BTreeSet<_>>();
    required_gates
        .iter()
        .any(|gate| gate.starts_with(REQUIRED_GATE_PREFIX) && !selected.contains(gate.as_str()))
}

fn activation_decision(policy: &str) -> Option<SteelOrchestrationActivationDecision> {
    match policy {
        ACTIVATION_NEXT_TURN => Some(SteelOrchestrationActivationDecision::NextTurn),
        ACTIVATION_EXPLICIT_RELOAD => Some(SteelOrchestrationActivationDecision::ExplicitReload),
        _ => None,
    }
}

fn sorted(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut values = values.into_iter().collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    const CURRENT_PACK_BYTES: &[u8] = b"old steel pack";
    const NEW_PACK_BYTES: &[u8] = b"new steel pack";
    const TARGET_PATH: &str = ".clankers/steel/scripts/plan-evolution.scm";

    fn state() -> SteelOrchestrationPackState {
        SteelOrchestrationPackState {
            pack_hash: current_hash(),
            required_gates: vec!["steel-pack-validate".to_string(), "steel-pack-smoke".to_string()],
        }
    }

    fn proposal() -> SteelOrchestrationPatchProposal {
        SteelOrchestrationPatchProposal {
            schema: STEEL_ORCHESTRATION_PATCH_SCHEMA.to_string(),
            intent: "improve orchestration gate selection".to_string(),
            target_paths: vec![TARGET_PATH.to_string()],
            expected_pack_hash: current_hash().prefixed(),
            patch_hash: ArtifactHash::digest(b"patch").prefixed(),
            selected_gates: vec!["steel-pack-validate".to_string(), "steel-pack-smoke".to_string()],
            activation_policy: ACTIVATION_NEXT_TURN.to_string(),
            authority_changes: Vec::new(),
        }
    }

    fn passing_gates() -> Vec<SteelOrchestrationGateResult> {
        vec![
            SteelOrchestrationGateResult {
                name: "steel-pack-validate".to_string(),
                passed: true,
                receipt_hash: ArtifactHash::digest(b"validate"),
            },
            SteelOrchestrationGateResult {
                name: "steel-pack-smoke".to_string(),
                passed: true,
                receipt_hash: ArtifactHash::digest(b"smoke"),
            },
        ]
    }

    fn patch_payload() -> SteelOrchestrationPatchPayload {
        SteelOrchestrationPatchPayload {
            target_path: TARGET_PATH.to_string(),
            bytes: NEW_PACK_BYTES.to_vec(),
        }
    }

    fn current_hash() -> ArtifactHash {
        target_bytes_hash(TARGET_PATH, CURRENT_PACK_BYTES)
    }

    fn staged_hash() -> ArtifactHash {
        target_bytes_hash(TARGET_PATH, NEW_PACK_BYTES)
    }

    fn target_bytes_hash(path: &str, bytes: &[u8]) -> ArtifactHash {
        let mut input = Vec::new();
        input.extend_from_slice(path.as_bytes());
        input.push(0);
        input.extend_from_slice(bytes);
        input.push(0);
        ArtifactHash::digest(&input)
    }

    fn temp_stage_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("clankers-steel-orch-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn valid_orchestration_patch_stages_and_promotes_after_gates() {
        let stage_dir = temp_stage_dir("valid");
        let staged = stage_orchestration_patch_to_directory(
            &proposal(),
            &state(),
            &stage_dir,
            &[patch_payload()],
            &passing_gates(),
        );
        assert_eq!(staged.status, SteelOrchestrationMutationStatus::Staged);
        assert_eq!(staged.reason_code, SteelOrchestrationMutationReason::Staged);
        assert_eq!(staged.proposed_new_pack_hash, Some(staged_hash()));
        assert_eq!(staged.activation_decision, SteelOrchestrationActivationDecision::NextTurn);
        assert!(!staged.writes_performed);
        assert_eq!(std::fs::read(stage_dir.join(TARGET_PATH)).expect("staged file"), NEW_PACK_BYTES);

        let live_dir = temp_stage_dir("live");
        let backup_dir = temp_stage_dir("backup");
        std::fs::create_dir_all(live_dir.join(".clankers/steel/scripts")).expect("live dir");
        std::fs::write(live_dir.join(TARGET_PATH), CURRENT_PACK_BYTES).expect("live file");
        let promoted = promote_staged_orchestration_pack_to_directory(&staged, &stage_dir, &live_dir, &backup_dir);
        assert_eq!(promoted.status, SteelOrchestrationMutationStatus::Promoted);
        assert!(promoted.writes_performed);
        assert_eq!(std::fs::read(live_dir.join(TARGET_PATH)).expect("promoted file"), NEW_PACK_BYTES);
        assert_eq!(std::fs::read(backup_dir.join(TARGET_PATH)).expect("backup file"), CURRENT_PACK_BYTES);
        assert!(promoted.receipt_hash().prefixed().starts_with("b3:"));
    }

    #[test]
    fn invalid_orchestration_patches_fail_before_writes() {
        let mut cases = Vec::new();
        let mut path_escape = proposal();
        path_escape.target_paths = vec!["../outside.scm".to_string()];
        cases.push((path_escape, SteelOrchestrationMutationReason::PathEscape));
        let mut stale = proposal();
        stale.expected_pack_hash = ArtifactHash::digest(b"other").prefixed();
        cases.push((stale, SteelOrchestrationMutationReason::StalePackHash));
        let mut raw_write = proposal();
        raw_write.authority_changes = vec!["raw_write:/home/operator/.ssh/id_rsa".to_string()];
        cases.push((raw_write, SteelOrchestrationMutationReason::RawHostWriteDenied));
        let mut authority = proposal();
        authority.authority_changes = vec!["new_host_call:repo.raw_shell".to_string()];
        cases.push((authority, SteelOrchestrationMutationReason::AuthorityKernelChange));
        let mut gate_removal = proposal();
        gate_removal.selected_gates = vec!["steel-pack-validate".to_string()];
        cases.push((gate_removal, SteelOrchestrationMutationReason::RequiredGateRemoval));
        let mut malformed = proposal();
        malformed.patch_hash = "sha256:not-allowed".to_string();
        cases.push((malformed, SteelOrchestrationMutationReason::MalformedPatchHash));

        for (proposal, reason) in cases {
            let receipt = validate_orchestration_patch_proposal(&proposal, &state());
            assert_eq!(receipt.status, SteelOrchestrationMutationStatus::Denied);
            assert_eq!(receipt.reason_code, reason);
            assert!(!receipt.writes_performed);
        }
    }

    #[test]
    fn denied_receipts_redact_unsafe_content() {
        let mut unsafe_proposal = proposal();
        unsafe_proposal.target_paths = vec!["/home/operator/.ssh/id_rsa".to_string()];
        unsafe_proposal.patch_hash = "sk-live-secret-token".to_string();
        unsafe_proposal.authority_changes = vec![
            "raw_write:/home/operator/.ssh/id_rsa".to_string(),
            "credential:sk-live-secret-token".to_string(),
        ];
        let receipt = validate_orchestration_patch_proposal(&unsafe_proposal, &state());
        let receipt_json = serde_json::to_string(&receipt).expect("receipt serializes");
        assert_eq!(receipt.status, SteelOrchestrationMutationStatus::Denied);
        assert_eq!(receipt.patch_hash.as_deref(), Some(REDACTED_INVALID_PATCH_HASH));
        assert_eq!(receipt.target_paths, vec![REDACTED_UNSAFE_TARGET_PATH.to_string()]);
        assert_eq!(receipt.authority_changes, vec!["credential".to_string(), RAW_WRITE_AUTHORITY_CLASS.to_string()]);
        assert!(!receipt_json.contains("sk-live-secret-token"));
        assert!(!receipt_json.contains("/home/operator"));
        assert!(!receipt_json.contains("raw_write:"));
    }

    #[test]
    fn failed_gate_blocks_activation_after_isolated_stage() {
        let failed = vec![SteelOrchestrationGateResult {
            name: "steel-pack-validate".to_string(),
            passed: false,
            receipt_hash: ArtifactHash::digest(b"fail"),
        }];
        let receipt = stage_orchestration_patch_to_directory(
            &proposal(),
            &state(),
            &temp_stage_dir("failed-gate"),
            &[patch_payload()],
            &failed,
        );
        assert_eq!(receipt.status, SteelOrchestrationMutationStatus::FailedValidation);
        assert_eq!(receipt.reason_code, SteelOrchestrationMutationReason::GateFailed);
        assert!(!receipt.writes_performed);
        assert_eq!(receipt.proposed_new_pack_hash, Some(staged_hash()));
    }

    #[test]
    fn rollback_requires_current_and_backup_hash_match() {
        let stage_dir = temp_stage_dir("rollback");
        let staged = stage_orchestration_patch_to_directory(
            &proposal(),
            &state(),
            &stage_dir,
            &[patch_payload()],
            &passing_gates(),
        );
        let live_dir = temp_stage_dir("rollback-live");
        let backup_dir = temp_stage_dir("rollback-backup");
        std::fs::create_dir_all(live_dir.join(".clankers/steel/scripts")).expect("live dir");
        std::fs::write(live_dir.join(TARGET_PATH), CURRENT_PACK_BYTES).expect("live file");
        let promoted = promote_staged_orchestration_pack_to_directory(&staged, &stage_dir, &live_dir, &backup_dir);
        let changed = rollback_orchestration_pack(&promoted, ArtifactHash::digest(b"operator edit"), state().pack_hash);
        assert_eq!(changed.reason_code, SteelOrchestrationMutationReason::CurrentPackChanged);
        assert!(!changed.writes_performed);

        let wrong_backup = rollback_orchestration_pack(&promoted, staged_hash(), ArtifactHash::digest(b"wrong"));
        assert_eq!(wrong_backup.reason_code, SteelOrchestrationMutationReason::BackupHashMismatch);
        assert!(!wrong_backup.writes_performed);

        let rolled_back = rollback_orchestration_pack(&promoted, staged_hash(), state().pack_hash);
        assert_eq!(rolled_back.status, SteelOrchestrationMutationStatus::RolledBack);
        assert!(!rolled_back.writes_performed);

        let live_rolled_back = rollback_orchestration_pack_to_directory(&promoted, &live_dir, &backup_dir);
        assert_eq!(live_rolled_back.status, SteelOrchestrationMutationStatus::RolledBack);
        assert!(live_rolled_back.writes_performed);
        assert_eq!(std::fs::read(live_dir.join(TARGET_PATH)).expect("rolled back file"), CURRENT_PACK_BYTES);
    }
}
