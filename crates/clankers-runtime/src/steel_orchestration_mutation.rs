//! Steel orchestration-pack mutation DTOs and pure validation core.
//!
//! This module lets Steel describe changes to repo-local orchestration packs.
//! Rust validates, stages, gates, activates later, and rolls back with hashes.

use std::collections::BTreeSet;

use clankers_artifacts::ArtifactHash;
use serde::Deserialize;
use serde::Serialize;

pub const STEEL_ORCHESTRATION_PATCH_SCHEMA: &str = "clankers.steel.orchestration-patch.v1";
pub const STEEL_ORCHESTRATION_MUTATION_RECEIPT_SCHEMA: &str = "clankers.steel.orchestration-pack-mutation.receipt.v1";
pub const STEEL_ORCHESTRATION_PACK_ROOT: &str = ".clankers/steel";
const ACTIVATION_EXPLICIT_RELOAD: &str = "explicit_reload";
const ACTIVATION_NEXT_TURN: &str = "next_turn";
const REQUIRED_GATE_PREFIX: &str = "steel-pack-";

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
    AuthorityKernelChange,
    RequiredGateRemoval,
    UnknownActivationPolicy,
    GateFailed,
    CurrentPackChanged,
    BackupHashMismatch,
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

#[must_use]
pub fn stage_orchestration_patch(
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
        writes_performed: true,
        ..staged.clone()
    }
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
        writes_performed: true,
        ..receipt.clone()
    }
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
        patch_hash: Some(proposal.patch_hash.clone()),
        target_paths: sorted(proposal.target_paths.clone()),
        selected_gates: sorted(proposal.selected_gates.clone()),
        gate_result_hashes: Vec::new(),
        activation_decision: SteelOrchestrationActivationDecision::Denied,
        rollback_reference: None,
        writes_performed: false,
        authority_changes: sorted(proposal.authority_changes.clone()),
    }
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

    fn state() -> SteelOrchestrationPackState {
        SteelOrchestrationPackState {
            pack_hash: ArtifactHash::digest(CURRENT_PACK_BYTES),
            required_gates: vec!["steel-pack-validate".to_string(), "steel-pack-smoke".to_string()],
        }
    }

    fn proposal() -> SteelOrchestrationPatchProposal {
        SteelOrchestrationPatchProposal {
            schema: STEEL_ORCHESTRATION_PATCH_SCHEMA.to_string(),
            intent: "improve orchestration gate selection".to_string(),
            target_paths: vec![".clankers/steel/scripts/plan-evolution.scm".to_string()],
            expected_pack_hash: ArtifactHash::digest(CURRENT_PACK_BYTES).prefixed(),
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

    #[test]
    fn valid_orchestration_patch_stages_and_promotes_after_gates() {
        let staged = stage_orchestration_patch(&proposal(), &state(), NEW_PACK_BYTES, &passing_gates());
        assert_eq!(staged.status, SteelOrchestrationMutationStatus::Staged);
        assert_eq!(staged.reason_code, SteelOrchestrationMutationReason::Staged);
        assert_eq!(staged.proposed_new_pack_hash, Some(ArtifactHash::digest(NEW_PACK_BYTES)));
        assert_eq!(staged.activation_decision, SteelOrchestrationActivationDecision::NextTurn);
        assert!(!staged.writes_performed);

        let promoted = promote_staged_orchestration_pack(&staged);
        assert_eq!(promoted.status, SteelOrchestrationMutationStatus::Promoted);
        assert!(promoted.writes_performed);
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
    fn failed_gate_blocks_activation_after_isolated_stage() {
        let failed = vec![SteelOrchestrationGateResult {
            name: "steel-pack-validate".to_string(),
            passed: false,
            receipt_hash: ArtifactHash::digest(b"fail"),
        }];
        let receipt = stage_orchestration_patch(&proposal(), &state(), NEW_PACK_BYTES, &failed);
        assert_eq!(receipt.status, SteelOrchestrationMutationStatus::FailedValidation);
        assert_eq!(receipt.reason_code, SteelOrchestrationMutationReason::GateFailed);
        assert!(!receipt.writes_performed);
        assert_eq!(receipt.proposed_new_pack_hash, Some(ArtifactHash::digest(NEW_PACK_BYTES)));
    }

    #[test]
    fn rollback_requires_current_and_backup_hash_match() {
        let staged = stage_orchestration_patch(&proposal(), &state(), NEW_PACK_BYTES, &passing_gates());
        let promoted = promote_staged_orchestration_pack(&staged);
        let changed = rollback_orchestration_pack(&promoted, ArtifactHash::digest(b"operator edit"), state().pack_hash);
        assert_eq!(changed.reason_code, SteelOrchestrationMutationReason::CurrentPackChanged);
        assert!(!changed.writes_performed);

        let wrong_backup = rollback_orchestration_pack(
            &promoted,
            ArtifactHash::digest(NEW_PACK_BYTES),
            ArtifactHash::digest(b"wrong"),
        );
        assert_eq!(wrong_backup.reason_code, SteelOrchestrationMutationReason::BackupHashMismatch);
        assert!(!wrong_backup.writes_performed);

        let rolled_back =
            rollback_orchestration_pack(&promoted, ArtifactHash::digest(NEW_PACK_BYTES), state().pack_hash);
        assert_eq!(rolled_back.status, SteelOrchestrationMutationStatus::RolledBack);
        assert!(rolled_back.writes_performed);
    }
}
