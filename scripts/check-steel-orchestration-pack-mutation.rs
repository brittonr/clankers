#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"

[dependencies]
clankers-artifacts = { path = "../crates/clankers-artifacts" }
clankers-runtime = { path = "../crates/clankers-runtime" }
serde_json = "1"
---

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;

use clankers_artifacts::ArtifactHash;
use clankers_runtime::steel_orchestration_mutation::*;
use serde_json::json;

const ERROR_EXIT: u8 = 1;
const OUT_DIR: &str = "target/steel-orchestration-pack-mutation";
const RECEIPT_PATH: &str = "target/steel-orchestration-pack-mutation/receipt.json";
const CURRENT_PACK_BYTES: &[u8] = b"old repo-local Steel pack";
const NEW_PACK_BYTES: &[u8] = b"new repo-local Steel pack";
const PATCH_BYTES: &[u8] = b"patch";
const REQUIRED_VALIDATE_GATE: &str = "steel-pack-validate";
const REQUIRED_SMOKE_GATE: &str = "steel-pack-smoke";
const TARGET_PATH: &str = ".clankers/steel/scripts/plan-evolution.scm";
const UNSAFE_SECRET_TOKEN: &str = "sk-live-secret-token";
const UNSAFE_SECRET_PATH: &str = "/home/operator/.ssh/id_rsa";

fn main() -> ExitCode {
    match run() {
        Ok(path) => {
            println!("steel orchestration-pack mutation receipt written to {}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("steel orchestration-pack mutation check failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<PathBuf, String> {
    let stage_dir = Path::new(OUT_DIR).join("stage-valid");
    let _ = fs::remove_dir_all(&stage_dir);
    let valid_staged =
        stage_orchestration_patch_to_directory(&proposal(), &state(), &stage_dir, &[patch_payload()], &passing_gates());
    let live_dir = Path::new(OUT_DIR).join("live-valid");
    let backup_dir = Path::new(OUT_DIR).join("backup-valid");
    let _ = fs::remove_dir_all(&live_dir);
    let _ = fs::remove_dir_all(&backup_dir);
    fs::create_dir_all(live_dir.join(".clankers/steel/scripts")).map_err(|error| error.to_string())?;
    fs::write(live_dir.join(TARGET_PATH), CURRENT_PACK_BYTES).map_err(|error| error.to_string())?;
    let promoted = promote_staged_orchestration_pack_to_directory(&valid_staged, &stage_dir, &live_dir, &backup_dir);
    let valid_update_passed = valid_staged.status == SteelOrchestrationMutationStatus::Staged
        && promoted.status == SteelOrchestrationMutationStatus::Promoted
        && promoted.writes_performed
        && fs::read(live_dir.join(TARGET_PATH)).map_err(|error| error.to_string())? == NEW_PACK_BYTES;
    let current_changed =
        rollback_orchestration_pack(&promoted, ArtifactHash::digest(b"operator edit"), state().pack_hash);
    let rolled_back = rollback_orchestration_pack(&promoted, staged_hash(), state().pack_hash);
    let live_rolled_back = rollback_orchestration_pack_to_directory(&promoted, &live_dir, &backup_dir);

    let fixtures = vec![
        ("valid-update", valid_update_passed),
        ("path-escape", denied(path_escape_proposal()) == SteelOrchestrationMutationReason::PathEscape),
        (
            "stale-before-hash",
            denied(stale_hash_proposal()) == SteelOrchestrationMutationReason::StalePackHash,
        ),
        (
            "raw-write-attempt",
            denied(raw_write_attempt_proposal()) == SteelOrchestrationMutationReason::RawHostWriteDenied,
        ),
        (
            "authority-widening",
            denied(authority_change_proposal()) == SteelOrchestrationMutationReason::AuthorityKernelChange,
        ),
        (
            "required-gate-removal",
            denied(required_gate_removal_proposal()) == SteelOrchestrationMutationReason::RequiredGateRemoval,
        ),
        (
            "failed-validation",
            failed_gate_receipt().status == SteelOrchestrationMutationStatus::FailedValidation,
        ),
        (
            "malformed-schema",
            denied(malformed_schema_proposal()) == SteelOrchestrationMutationReason::InvalidSchema,
        ),
        (
            "malformed-patch-hash",
            denied(malformed_patch_hash_proposal()) == SteelOrchestrationMutationReason::MalformedPatchHash,
        ),
        ("unsafe-receipt-content", unsafe_receipt_content_redacted()),
        (
            "stale-rollback",
            current_changed.reason_code == SteelOrchestrationMutationReason::CurrentPackChanged,
        ),
        (
            "guarded-rollback",
            rolled_back.status == SteelOrchestrationMutationStatus::RolledBack
                && live_rolled_back.status == SteelOrchestrationMutationStatus::RolledBack
                && live_rolled_back.writes_performed
                && fs::read(live_dir.join(TARGET_PATH)).map_err(|error| error.to_string())? == CURRENT_PACK_BYTES,
        ),
    ];
    for (name, passed) in &fixtures {
        if !passed {
            return Err(format!("fixture {name} failed"));
        }
    }
    let receipt = json!({
        "schema": "clankers.steel.orchestration_pack_mutation.check_receipt.v1",
        "fixtures": fixtures.iter().map(|(name, passed)| json!({"name": name, "passed": passed})).collect::<Vec<_>>(),
        "validated_surfaces": [
            "typed-orchestration-patch-schema",
            "path-root-validation",
            "stale-before-hash-denial",
            "raw-write-attempt-denial",
            "authority-kernel-checkpoint-denial",
            "unsafe-receipt-redaction",
            "required-gate-preservation",
            "isolated-stage-before-promotion",
            "hash-guarded-live-promotion",
            "next-turn-or-explicit-reload-activation",
            "guarded-rollback"
        ],
        "hashed_artifacts": [
            {"path": "crates/clankers-runtime/src/steel_orchestration_mutation.rs", "blake3": hash_file("crates/clankers-runtime/src/steel_orchestration_mutation.rs")?},
            {"path": "scripts/check-steel-orchestration-pack-mutation.rs", "blake3": hash_file("scripts/check-steel-orchestration-pack-mutation.rs")?},
            {"path": "docs/src/reference/steel-orchestration-pack-mutation.md", "blake3": hash_file("docs/src/reference/steel-orchestration-pack-mutation.md")?}
        ]
    });
    let path = PathBuf::from(RECEIPT_PATH);
    fs::create_dir_all(OUT_DIR).map_err(|error| format!("failed to create {OUT_DIR}: {error}"))?;
    fs::write(&path, serde_json::to_vec_pretty(&receipt).map_err(|error| error.to_string())?)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    Ok(path)
}

fn state() -> SteelOrchestrationPackState {
    SteelOrchestrationPackState {
        pack_hash: current_hash(),
        required_gates: vec![REQUIRED_VALIDATE_GATE.to_string(), REQUIRED_SMOKE_GATE.to_string()],
    }
}

fn proposal() -> SteelOrchestrationPatchProposal {
    SteelOrchestrationPatchProposal {
        schema: STEEL_ORCHESTRATION_PATCH_SCHEMA.to_string(),
        intent: "tune repo-local orchestration".to_string(),
        target_paths: vec![TARGET_PATH.to_string()],
        expected_pack_hash: current_hash().prefixed(),
        patch_hash: ArtifactHash::digest(PATCH_BYTES).prefixed(),
        selected_gates: vec![REQUIRED_VALIDATE_GATE.to_string(), REQUIRED_SMOKE_GATE.to_string()],
        activation_policy: "next_turn".to_string(),
        authority_changes: Vec::new(),
    }
}

fn path_escape_proposal() -> SteelOrchestrationPatchProposal {
    let mut proposal = proposal();
    proposal.target_paths = vec!["../outside.scm".to_string()];
    proposal
}

fn stale_hash_proposal() -> SteelOrchestrationPatchProposal {
    let mut proposal = proposal();
    proposal.expected_pack_hash = ArtifactHash::digest(b"stale").prefixed();
    proposal
}

fn raw_write_attempt_proposal() -> SteelOrchestrationPatchProposal {
    let mut proposal = proposal();
    proposal.authority_changes = vec![format!("raw_write:{UNSAFE_SECRET_PATH}")];
    proposal
}

fn authority_change_proposal() -> SteelOrchestrationPatchProposal {
    let mut proposal = proposal();
    proposal.authority_changes = vec!["new_host_call:repo.raw_shell".to_string()];
    proposal
}

fn required_gate_removal_proposal() -> SteelOrchestrationPatchProposal {
    let mut proposal = proposal();
    proposal.selected_gates = vec![REQUIRED_VALIDATE_GATE.to_string()];
    proposal
}

fn malformed_schema_proposal() -> SteelOrchestrationPatchProposal {
    let mut proposal = proposal();
    proposal.schema = "wrong".to_string();
    proposal
}

fn malformed_patch_hash_proposal() -> SteelOrchestrationPatchProposal {
    let mut proposal = proposal();
    proposal.patch_hash = "sha256:not-allowed".to_string();
    proposal
}

fn passing_gates() -> Vec<SteelOrchestrationGateResult> {
    vec![
        SteelOrchestrationGateResult {
            name: REQUIRED_VALIDATE_GATE.to_string(),
            passed: true,
            receipt_hash: ArtifactHash::digest(REQUIRED_VALIDATE_GATE.as_bytes()),
        },
        SteelOrchestrationGateResult {
            name: REQUIRED_SMOKE_GATE.to_string(),
            passed: true,
            receipt_hash: ArtifactHash::digest(REQUIRED_SMOKE_GATE.as_bytes()),
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

fn failed_gate_receipt() -> SteelOrchestrationMutationReceipt {
    let failed = [SteelOrchestrationGateResult {
        name: REQUIRED_VALIDATE_GATE.to_string(),
        passed: false,
        receipt_hash: ArtifactHash::digest(b"failed-gate"),
    }];
    let stage_dir = Path::new(OUT_DIR).join("stage-failed-gate");
    let _ = fs::remove_dir_all(&stage_dir);
    stage_orchestration_patch_to_directory(&proposal(), &state(), &stage_dir, &[patch_payload()], &failed)
}

fn unsafe_receipt_content_redacted() -> bool {
    let mut proposal = proposal();
    proposal.target_paths = vec![UNSAFE_SECRET_PATH.to_string()];
    proposal.patch_hash = format!("b3:{UNSAFE_SECRET_TOKEN}");
    proposal.selected_gates = vec![REQUIRED_VALIDATE_GATE.to_string(), UNSAFE_SECRET_TOKEN.to_string()];
    proposal.authority_changes = vec![
        format!("raw_write:{UNSAFE_SECRET_PATH}"),
        format!("credential:{UNSAFE_SECRET_TOKEN}"),
    ];
    let receipt = validate_orchestration_patch_proposal(&proposal, &state());
    let Ok(receipt_json) = serde_json::to_string(&receipt) else {
        return false;
    };
    receipt.status == SteelOrchestrationMutationStatus::Denied
        && receipt.reason_code == SteelOrchestrationMutationReason::MalformedPatchHash
        && receipt.patch_hash.as_deref() == Some("redacted:invalid-patch-hash")
        && receipt.target_paths == vec!["redacted:target-path".to_string()]
        && receipt.selected_gates == vec!["redacted:selected-gate".to_string(), REQUIRED_VALIDATE_GATE.to_string()]
        && !receipt_json.contains(UNSAFE_SECRET_TOKEN)
        && !receipt_json.contains(UNSAFE_SECRET_PATH)
        && !receipt_json.contains("raw_write:")
}

fn denied(proposal: SteelOrchestrationPatchProposal) -> SteelOrchestrationMutationReason {
    validate_orchestration_patch_proposal(&proposal, &state()).reason_code
}

fn hash_file(path: &str) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| format!("failed to read {path}: {error}"))?;
    Ok(ArtifactHash::digest(&bytes).prefixed())
}
