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
use clankers_runtime::steel_repo_evolution::*;
use serde_json::json;

const ERROR_EXIT: u8 = 1;
const OUT_DIR: &str = "target/steel-repo-evolution-packs";
const RECEIPT_PATH: &str = "target/steel-repo-evolution-packs/receipt.json";
const SCRIPT_SOURCE: &[u8] = b"(host \"repo.propose_patch\")";
const PACK_NAME: &str = "fixture-pack";
const SCRIPT_ID: &str = "plan-evolution";
const SCRIPT_PATH: &str = ".clankers/steel/scripts/plan-evolution.scm";
const GATE_NAME: &str = "steel-pack-validate";
const OUTPUT_BUDGET_BYTES: u64 = 4096;
const HOST_CALL_BUDGET: u64 = 4;

fn main() -> ExitCode {
    match run() {
        Ok(path) => {
            println!("steel repo evolution pack receipt written to {}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("steel repo evolution pack check failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<PathBuf, String> {
    let valid_pack = pack_json(&valid_pack());
    let repo_load_receipt = load_repo_evolution_pack(Path::new(".")).map_err(|error| error.to_string())?;
    let fixtures = vec![
        (
            "absent",
            inactive_repo_evolution_receipt().reason_code == SteelRepoEvolutionActivationReason::AbsentPack,
        ),
        (
            "valid",
            validate_repo_evolution_pack_from_export(&valid_pack, |_| Some(SCRIPT_SOURCE.to_vec())).status
                == SteelRepoEvolutionActivationStatus::Active,
        ),
        ("repo-local-runtime-load", repo_load_receipt.status == SteelRepoEvolutionActivationStatus::Active),
        (
            "malformed",
            validate_repo_evolution_pack_from_export("not-json", |_| Some(SCRIPT_SOURCE.to_vec())).reason_code
                == SteelRepoEvolutionActivationReason::InvalidProfileJson,
        ),
        (
            "invalid-nickel-contract",
            validate_repo_evolution_pack_from_sources(&valid_pack, "let profile = {} in profile", |_| {
                Some(SCRIPT_SOURCE.to_vec())
            })
            .reason_code
                == SteelRepoEvolutionActivationReason::InvalidNickelContract,
        ),
        (
            "hash-mismatch",
            validate_repo_evolution_pack_from_export(&valid_pack, |_| Some(b"other".to_vec())).reason_code
                == SteelRepoEvolutionActivationReason::ScriptHashMismatch,
        ),
        (
            "path-escape",
            validate_repo_evolution_pack_from_export(&pack_json(&path_escape_pack()), |_| Some(SCRIPT_SOURCE.to_vec()))
                .reason_code
                == SteelRepoEvolutionActivationReason::PathEscape,
        ),
        (
            "unknown-host-call",
            validate_repo_evolution_pack_from_export(&pack_json(&unknown_host_pack()), |_| {
                Some(SCRIPT_SOURCE.to_vec())
            })
            .reason_code
                == SteelRepoEvolutionActivationReason::UnknownHostCall,
        ),
        (
            "over-budget",
            validate_repo_evolution_pack_from_export(&pack_json(&over_budget_pack()), |_| Some(SCRIPT_SOURCE.to_vec()))
                .reason_code
                == SteelRepoEvolutionActivationReason::ScriptTooLarge,
        ),
        ("typed-plan", valid_plan_receipt().status == SteelRepoEvolutionPlanStatus::Accepted),
        (
            "malformed-plan",
            evaluate_repo_evolution_plan(&valid_pack_struct(), "not-json").reason_code
                == SteelRepoEvolutionPlanReason::MalformedPayload,
        ),
    ];
    for (name, passed) in &fixtures {
        if !passed {
            return Err(format!("fixture {name} failed"));
        }
    }
    let receipt = json!({
        "schema": "clankers.steel.repo_evolution_pack.check_receipt.v1",
        "fixtures": fixtures.iter().map(|(name, passed)| json!({"name": name, "passed": passed})).collect::<Vec<_>>(),
        "validated_surfaces": [
            "absent-default-deny",
            "nickel-export-profile-json",
            "script-path-and-hash-validation",
            "nickel-contract-marker-validation",
            "rust-owned-host-abi",
            "typed-evolution-plan",
            "higher-order-host-contracts",
            "repo-local-runtime-load",
            "redacted-receipts"
        ],
        "hashed_artifacts": [
            {"path": "crates/clankers-runtime/src/steel_repo_evolution.rs", "blake3": hash_file("crates/clankers-runtime/src/steel_repo_evolution.rs")?},
            {"path": "scripts/check-steel-repo-evolution-packs.rs", "blake3": hash_file("scripts/check-steel-repo-evolution-packs.rs")?},
            {"path": "docs/src/reference/steel-repo-evolution-packs.md", "blake3": hash_file("docs/src/reference/steel-repo-evolution-packs.md")?},
            {"path": ".clankers/steel/evolution-profile.ncl", "blake3": hash_file(".clankers/steel/evolution-profile.ncl")?},
            {"path": ".clankers/steel/evolution-profile.json", "blake3": hash_file(".clankers/steel/evolution-profile.json")?},
            {"path": ".clankers/steel/scripts/plan-evolution.scm", "blake3": hash_file(".clankers/steel/scripts/plan-evolution.scm")?}
        ]
    });
    let path = PathBuf::from(RECEIPT_PATH);
    fs::create_dir_all(OUT_DIR).map_err(|error| format!("failed to create {OUT_DIR}: {error}"))?;
    fs::write(&path, serde_json::to_vec_pretty(&receipt).map_err(|error| error.to_string())?)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    Ok(path)
}

fn valid_pack_struct() -> SteelRepoEvolutionPack {
    SteelRepoEvolutionPack {
        schema: STEEL_REPO_EVOLUTION_PACK_SCHEMA.to_string(),
        name: PACK_NAME.to_string(),
        abi_version: STEEL_REPO_EVOLUTION_ABI_VERSION.to_string(),
        scripts: vec![SteelRepoEvolutionScriptBinding {
            id: SCRIPT_ID.to_string(),
            path: SCRIPT_PATH.to_string(),
            blake3: ArtifactHash::digest(SCRIPT_SOURCE).prefixed(),
        }],
        allowed_host_calls: vec!["repo.propose_patch".to_string(), "repo.run_gate".to_string()],
        host_contracts: vec![
            SteelRepoEvolutionHostContract {
                name: "contract-propose-patch".to_string(),
                wraps_host_call: "repo.propose_patch".to_string(),
                mode: "higher_order".to_string(),
                preconditions: vec!["typed-patch-envelope".to_string()],
                postconditions: vec!["receipt-recorded".to_string()],
            },
            SteelRepoEvolutionHostContract {
                name: "contract-run-gate".to_string(),
                wraps_host_call: "repo.run_gate".to_string(),
                mode: "higher_order".to_string(),
                preconditions: vec!["gate-allowlisted".to_string()],
                postconditions: vec!["gate-receipt-hash".to_string()],
            },
        ],
        budgets: SteelRepoEvolutionBudgets {
            max_source_bytes: SCRIPT_SOURCE.len() as u64,
            max_output_bytes: OUTPUT_BUDGET_BYTES,
            max_host_calls: HOST_CALL_BUDGET,
        },
        gates: vec![GATE_NAME.to_string()],
        receipt_root: OUT_DIR.to_string(),
        fallback_mode: SteelRepoEvolutionFallbackMode::Block,
    }
}

fn valid_pack() -> SteelRepoEvolutionPack {
    valid_pack_struct()
}

fn path_escape_pack() -> SteelRepoEvolutionPack {
    let mut pack = valid_pack_struct();
    pack.scripts[0].path = "../plan.scm".to_string();
    pack
}

fn unknown_host_pack() -> SteelRepoEvolutionPack {
    let mut pack = valid_pack_struct();
    pack.allowed_host_calls = vec!["repo.raw_shell".to_string()];
    pack
}

fn over_budget_pack() -> SteelRepoEvolutionPack {
    let mut pack = valid_pack_struct();
    pack.budgets.max_source_bytes = 1;
    pack
}

fn valid_plan_receipt() -> SteelRepoEvolutionPlanReceipt {
    let plan = json!({
        "schema": STEEL_REPO_EVOLUTION_PLAN_SCHEMA,
        "intent": "propose orchestration improvement",
        "actions": [{"host_call": "repo.propose_patch", "payload_hash": ArtifactHash::digest(b"patch").prefixed()}],
        "gates": [GATE_NAME]
    })
    .to_string();
    evaluate_repo_evolution_plan(&valid_pack_struct(), &plan)
}

fn pack_json(pack: &SteelRepoEvolutionPack) -> String {
    serde_json::to_string(pack).expect("pack serializes")
}

fn hash_file(path: &str) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| format!("failed to read {path}: {error}"))?;
    Ok(ArtifactHash::digest(&bytes).prefixed())
}
