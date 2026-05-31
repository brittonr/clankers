#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"

[dependencies]
blake3 = "1"
serde_json = "1"
---

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;

use serde_json::json;

const ERROR_EXIT: u8 = 1;
const RUNTIME: &str = "crates/clankers-runtime/src/steel_orchestration.rs";
const RUNTIME_LIB: &str = "crates/clankers-runtime/src/lib.rs";
const AGENT_PLANNING: &str = "crates/clankers-agent/src/turn/steel_planning.rs";
const DOC_AGENT: &str = "docs/src/reference/steel-agent-turn-wiring.md";
const DOC_DEFAULT: &str = "docs/src/reference/steel-default-orchestration.md";
const DOC_SMOKE: &str = "docs/src/reference/steel-turn-planning-runtime-smoke.md";
const SPEC: &str = "cairn/specs/steel-host-call-json-payloads/spec.md";
const TASKS: &str = "cairn/archive/1970-01-01-steel-host-call-json-payloads/tasks.md";
const OUTPUT: &str = "target/steel-host-call-json-payloads/receipt.json";

const REQUIRED_RUNTIME_MARKERS: &[&str] = &[
    "SteelTurnPlanHostCallPayload",
    "SteelTurnExecutionHostCallPayload",
    "serde_json::from_str::<SteelTurnPlanHostCallPayload>",
    "serde_json::from_str::<SteelTurnExecutionHostCallPayload>",
    "legacy_delimited_payload",
    "STEEL_TURN_EXECUTION_HOST_CALL_SCHEMA",
    "payload_hash",
    "payload_valid",
];
const REQUIRED_AGENT_MARKERS: &[&str] = &[
    "SteelTurnPlanHostCallPayload",
    "steel_plan_payload(",
    "serde_json::from_str::<SteelTurnPlanHostCallPayload>",
    ".to_json()",
];
const REQUIRED_DOC_MARKERS: &[&str] = &["typed JSON", "JSON host-call", "steel.host.execute_turn"];
const REQUIRED_SPEC_MARKERS: &[&str] = &[
    "r[steel-host-call-json-payloads.plan.valid]",
    "r[steel-host-call-json-payloads.plan.legacy-denied]",
    "r[steel-host-call-json-payloads.execute.valid]",
    "r[steel-host-call-json-payloads.execute.malformed-denied]",
    "r[steel-host-call-json-payloads.receipts.hashes]",
];
const FORBIDDEN_RUNTIME_MARKERS: &[&str] = &["payload.split('|')"];
const FORBIDDEN_DOC_MARKERS: &[&str] = &["raw_prompt =", "provider_payload =", "credential_value", "compact_ucan"];

fn main() -> ExitCode {
    match run() {
        Ok(path) => {
            println!("steel host-call JSON payload receipt written to {}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("steel host-call JSON payload check failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<PathBuf, String> {
    let runtime = read(RUNTIME)?;
    let runtime_lib = read(RUNTIME_LIB)?;
    let agent_planning = read(AGENT_PLANNING)?;
    let doc_agent = read(DOC_AGENT)?;
    let doc_default = read(DOC_DEFAULT)?;
    let doc_smoke = read(DOC_SMOKE)?;
    let spec = read(SPEC)?;
    let tasks = read(TASKS)?;
    let mut errors = Vec::new();

    require_all(RUNTIME, &runtime, REQUIRED_RUNTIME_MARKERS, &mut errors);
    require_all(
        RUNTIME_LIB,
        &runtime_lib,
        &["SteelTurnPlanHostCallPayload", "SteelTurnExecutionHostCallPayload"],
        &mut errors,
    );
    require_all(AGENT_PLANNING, &agent_planning, REQUIRED_AGENT_MARKERS, &mut errors);
    require_all(DOC_AGENT, &doc_agent, REQUIRED_DOC_MARKERS, &mut errors);
    require_all(DOC_DEFAULT, &doc_default, REQUIRED_DOC_MARKERS, &mut errors);
    require_all(DOC_SMOKE, &doc_smoke, REQUIRED_DOC_MARKERS, &mut errors);
    require_all(SPEC, &spec, REQUIRED_SPEC_MARKERS, &mut errors);
    require_all(TASKS, &tasks, &["pipe-delimited", "JSON serialization/deserialization"], &mut errors);
    forbid_all(RUNTIME, &runtime, FORBIDDEN_RUNTIME_MARKERS, &mut errors);
    forbid_all(DOC_AGENT, &doc_agent, FORBIDDEN_DOC_MARKERS, &mut errors);
    forbid_all(DOC_DEFAULT, &doc_default, FORBIDDEN_DOC_MARKERS, &mut errors);
    forbid_all(DOC_SMOKE, &doc_smoke, FORBIDDEN_DOC_MARKERS, &mut errors);
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }

    let artifacts = [
        RUNTIME,
        RUNTIME_LIB,
        AGENT_PLANNING,
        DOC_AGENT,
        DOC_DEFAULT,
        DOC_SMOKE,
        SPEC,
        TASKS,
        "scripts/check-steel-host-call-json-payloads.rs",
    ]
    .iter()
    .map(|path| hash_artifact(Path::new(path)))
    .collect::<Result<Vec<_>, _>>()?;
    let receipt = json!({
        "schema": "clankers.steel_host_call_json_payloads.check_receipt.v1",
        "validated_surfaces": [
            "planning-json-dto",
            "execute-turn-json-dto",
            "legacy-delimited-plan-rejection",
            "malformed-execute-payload-denial",
            "redacted-json-payload-receipts"
        ],
        "hashed_artifacts": artifacts,
    });
    let output_path = PathBuf::from(OUTPUT);
    let parent = output_path.parent().ok_or_else(|| format!("{} has no parent", output_path.display()))?;
    fs::create_dir_all(parent).map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let bytes = serde_json::to_vec_pretty(&receipt).map_err(|error| format!("failed to encode receipt: {error}"))?;
    fs::write(&output_path, [bytes.as_slice(), b"\n"].concat())
        .map_err(|error| format!("failed to write {}: {error}", output_path.display()))?;
    Ok(output_path)
}

fn read(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("failed to read {path}: {error}"))
}

fn require_all(path: &str, text: &str, markers: &[&str], errors: &mut Vec<String>) {
    for marker in markers {
        if !text.contains(marker) {
            errors.push(format!("{path} missing marker `{marker}`"));
        }
    }
}

fn forbid_all(path: &str, text: &str, markers: &[&str], errors: &mut Vec<String>) {
    for marker in markers {
        if text.contains(marker) {
            errors.push(format!("{path} contains forbidden marker `{marker}`"));
        }
    }
}

fn hash_artifact(path: &Path) -> Result<serde_json::Value, String> {
    let bytes = fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let hash = blake3::hash(&bytes).to_hex().to_string();
    Ok(json!({
        "path": path.display().to_string(),
        "bytes": bytes.len(),
        "blake3": hash,
    }))
}
