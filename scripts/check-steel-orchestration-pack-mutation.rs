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
    let valid_staged = stage_orchestration_patch(&proposal(), &state(), NEW_PACK_BYTES, &passing_gates());
    let promoted = promote_staged_orchestration_pack(&valid_staged);
    let current_changed =
        rollback_orchestration_pack(&promoted, ArtifactHash::digest(b"operator edit"), state().pack_hash);
    let rolled_back = rollback_orchestration_pack(&promoted, ArtifactHash::digest(NEW_PACK_BYTES), state().pack_hash);

    let fixtures = vec![
        ("valid-update", valid_staged.status == SteelOrchestrationMutationStatus::Staged),
        ("path-escape", denied(path_escape_proposal()) == SteelOrchestrationMutationReason::PathEscape),
        (
            "stale-before-hash",
            denied(stale_hash_proposal()) == SteelOrchestrationMutationReason::StalePackHash,
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
        (
            "stale-rollback",
            current_changed.reason_code == SteelOrchestrationMutationReason::CurrentPackChanged,
        ),
        ("guarded-rollback", rolled_back.status == SteelOrchestrationMutationStatus::RolledBack),
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
            "authority-kernel-checkpoint-denial",
            "required-gate-preservation",
            "isolated-stage-before-promotion",
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
        pack_hash: ArtifactHash::digest(CURRENT_PACK_BYTES),
        required_gates: vec![REQUIRED_VALIDATE_GATE.to_string(), REQUIRED_SMOKE_GATE.to_string()],
    }
}

fn proposal() -> SteelOrchestrationPatchProposal {
    SteelOrchestrationPatchProposal {
        schema: STEEL_ORCHESTRATION_PATCH_SCHEMA.to_string(),
        intent: "tune repo-local orchestration".to_string(),
        target_paths: vec![".clankers/steel/scripts/plan-evolution.scm".to_string()],
        expected_pack_hash: ArtifactHash::digest(CURRENT_PACK_BYTES).prefixed(),
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

fn failed_gate_receipt() -> SteelOrchestrationMutationReceipt {
    let failed = [SteelOrchestrationGateResult {
        name: REQUIRED_VALIDATE_GATE.to_string(),
        passed: false,
        receipt_hash: ArtifactHash::digest(b"failed-gate"),
    }];
    stage_orchestration_patch(&proposal(), &state(), NEW_PACK_BYTES, &failed)
}

fn denied(proposal: SteelOrchestrationPatchProposal) -> SteelOrchestrationMutationReason {
    validate_orchestration_patch_proposal(&proposal, &state()).reason_code
}

fn hash_file(path: &str) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| format!("failed to read {path}: {error}"))?;
    Ok(ArtifactHash::digest(&bytes).prefixed())
}
