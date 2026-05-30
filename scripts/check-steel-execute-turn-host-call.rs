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
const AGENT_EXECUTION: &str = "crates/clankers-agent/src/turn/steel_execution.rs";
const AGENT_TURN: &str = "crates/clankers-agent/src/turn/mod.rs";
const EMBEDDED_TEST: &str = "tests/embedded_controller.rs";
const DOC_AGENT: &str = "docs/src/reference/steel-agent-turn-wiring.md";
const DOC_SMOKE: &str = "docs/src/reference/steel-turn-planning-runtime-smoke.md";
const SPEC: &str = "cairn/specs/steel-execute-turn-host-call/spec.md";
const TASKS: &str = "cairn/archive/1970-01-01-steel-execute-turn-host-call/tasks.md";
const OUTPUT: &str = "target/steel-execute-turn-host-call/receipt.json";

const REQUIRED_RUNTIME_MARKERS: &[&str] = &[
    "STEEL_TURN_EXECUTION_HOST_CALL_SCHEMA",
    "DEFAULT_TURN_EXECUTION_SOURCE",
    "SteelTurnExecutionHostCallReceipt",
    "evaluate_steel_execution_host_call",
    "steel_execution_host_call_payload_is_valid",
    "SteelRuntimeRequest",
    "SteelHostFunctionRegistration",
    "host_call_receipt.is_allowed()",
    "execute_turn_host_call_rejects_malformed_payload_before_authorized_status",
];
const REQUIRED_AGENT_MARKERS: &[&str] = &[
    "host_call_status=",
    "host_call_reason=",
    "host_call_outcome=",
    "host_call_payload=",
    "host_call_receipt_hash=",
    "authority_status=",
    "run_engine_turn(seed, hosts).await",
];
const REQUIRED_TEST_MARKERS: &[&str] = &[
    "host_call_status=Succeeded",
    "host_call_reason=Ok",
    "host_call_outcome=Approved",
    "host_call_payload=Valid",
    "host_call_status=Denied",
    "host_call_reason=MissingHostCapability",
    "calls.load(Ordering::SeqCst), 0",
];
const REQUIRED_AGENT_DOC_MARKERS: &[&str] = &[
    "steel.host.execute_turn",
    "host-call",
    "Steel host-call status/reason/hash",
    "before any provider request",
];
const REQUIRED_SMOKE_DOC_MARKERS: &[&str] = &[
    "steel.host.execute_turn",
    "host-call",
    "host_call_status=Succeeded",
    "host_call_reason=MissingHostCapability",
    "before any provider request",
];
const REQUIRED_SPEC_MARKERS: &[&str] = &[
    "r[steel-execute-turn-host-call.runtime.allowed]",
    "r[steel-execute-turn-host-call.runtime.denied]",
    "r[steel-execute-turn-host-call.runtime.malformed]",
    "r[steel-execute-turn-host-call.receipts.allowed]",
    "r[steel-execute-turn-host-call.verification.real-denial]",
];
const FORBIDDEN_DOC_MARKERS: &[&str] = &["raw_prompt =", "provider_payload =", "credential_value", "compact_ucan"];

fn main() -> ExitCode {
    match run() {
        Ok(path) => {
            println!("steel execute-turn host-call receipt written to {}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("steel execute-turn host-call check failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<PathBuf, String> {
    let runtime = read(RUNTIME)?;
    let runtime_lib = read(RUNTIME_LIB)?;
    let agent_execution = read(AGENT_EXECUTION)?;
    let agent_turn = read(AGENT_TURN)?;
    let embedded = read(EMBEDDED_TEST)?;
    let doc_agent = read(DOC_AGENT)?;
    let doc_smoke = read(DOC_SMOKE)?;
    let spec = read(SPEC)?;
    let tasks = read(TASKS)?;
    let mut errors = Vec::new();

    require_all(RUNTIME, &runtime, REQUIRED_RUNTIME_MARKERS, &mut errors);
    require_all(
        RUNTIME_LIB,
        &runtime_lib,
        &["SteelTurnExecutionHostCallReceipt", "DEFAULT_TURN_EXECUTION_SOURCE"],
        &mut errors,
    );
    require_all(AGENT_EXECUTION, &agent_execution, REQUIRED_AGENT_MARKERS, &mut errors);
    require_all(AGENT_TURN, &agent_turn, &["host_call_status=Succeeded", "host_call_payload=Valid"], &mut errors);
    require_all(EMBEDDED_TEST, &embedded, REQUIRED_TEST_MARKERS, &mut errors);
    require_all(DOC_AGENT, &doc_agent, REQUIRED_AGENT_DOC_MARKERS, &mut errors);
    require_all(DOC_SMOKE, &doc_smoke, REQUIRED_SMOKE_DOC_MARKERS, &mut errors);
    require_all(SPEC, &spec, REQUIRED_SPEC_MARKERS, &mut errors);
    require_all(TASKS, &tasks, &["r[steel-execute-turn-host-call.runtime.malformed]"], &mut errors);
    forbid_all(DOC_AGENT, &doc_agent, FORBIDDEN_DOC_MARKERS, &mut errors);
    forbid_all(DOC_SMOKE, &doc_smoke, FORBIDDEN_DOC_MARKERS, &mut errors);
    if agent_execution.contains("raw_prompt=") {
        errors.push("agent host-call receipt must not format raw prompt fields".to_string());
    }
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }

    let artifacts = [
        RUNTIME,
        RUNTIME_LIB,
        AGENT_EXECUTION,
        AGENT_TURN,
        EMBEDDED_TEST,
        DOC_AGENT,
        DOC_SMOKE,
        SPEC,
        TASKS,
        "scripts/check-steel-execute-turn-host-call.rs",
    ]
    .iter()
    .map(|path| hash_artifact(Path::new(path)))
    .collect::<Result<Vec<_>, _>>()?;
    let receipt = json!({
        "schema": "clankers.steel_execute_turn_host_call.check_receipt.v1",
        "validated_surfaces": [
            "runtime-steel-host-call-source",
            "typed-execution-host-call-payload",
            "host-call-approval-denial-and-malformed-tests",
            "agent-redacted-host-call-fields",
            "embedded-provider-skip-on-host-call-denial"
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
