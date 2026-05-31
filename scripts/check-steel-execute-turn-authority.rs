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

use serde_json::Value;
use serde_json::json;

const ERROR_EXIT: u8 = 1;
const RUNTIME: &str = "crates/clankers-runtime/src/steel_orchestration.rs";
const RUNTIME_LIB: &str = "crates/clankers-runtime/src/lib.rs";
const AGENT_EXECUTION: &str = "crates/clankers-agent/src/turn/steel_execution.rs";
const AGENT_TURN: &str = "crates/clankers-agent/src/turn/mod.rs";
const AGENT_PLANNING: &str = "crates/clankers-agent/src/turn/steel_planning.rs";
const SETTINGS: &str = "crates/clankers-config/src/settings.rs";
const EMBEDDED_TEST: &str = "tests/embedded_controller.rs";
const ROOT_PROFILE: &str = "policy/steel-default-orchestration/orchestration-profile.json";
const AGENT_PROFILE: &str = "crates/clankers-agent/policy/steel-default-orchestration/orchestration-profile.json";
const DOC: &str = "docs/src/reference/steel-agent-turn-wiring.md";
const SMOKE_DOC: &str = "docs/src/reference/steel-turn-planning-runtime-smoke.md";
const TASKS: &str = "cairn/archive/1970-01-01-steel-execute-turn-authority/tasks.md";
const OUTPUT: &str = "target/steel-execute-turn-authority/receipt.json";

const REQUIRED_RUNTIME_MARKERS: &[&str] = &[
    "STEEL_TURN_EXECUTION_RECEIPT_SCHEMA",
    "DEFAULT_TURN_EXECUTION_SEAM",
    "SteelTurnExecutionInput",
    "SteelTurnExecutionHostCallPayload",
    "SteelTurnExecutionHostCallReceipt",
    "SteelTurnExecutionReceipt",
    "DEFAULT_TURN_EXECUTION_SOURCE",
    "authorize_steel_turn_execution",
    "DynamicRuntimeActionKind::HostFunction",
    "clankers/steel/orchestrate.execute_turn",
    "execute_turn_authority_requires_execution_capability_and_ucan",
    "execute_turn_authority_denies_missing_ucan_or_disabled_action_before_host_runner",
];
const REQUIRED_AGENT_EXECUTION_MARKERS: &[&str] = &[
    "authorize_steel_turn_execution",
    "SteelTurnExecutionInput",
    "steel.host.execute_turn denied before provider request",
    "host_call_status=",
    "host_call_reason=",
    "host_call_receipt_hash=",
    "authority_status=",
    "authority_reason=",
    "required_ucan=",
    "authority_receipt_hash=",
    "run_engine_turn(seed, hosts).await",
];
const REQUIRED_EMBEDDED_MARKERS: &[&str] = &[
    "steel_runtime_smoke_missing_execute_authority_fails_closed_before_provider",
    "calls.load(Ordering::SeqCst), 0",
    "host_call_status=Denied",
    "host_call_reason=MissingHostCapability",
    "authority_status=PolicyDenied",
    "authority_reason=MissingSessionCapability",
    "required_ucan=clankers/steel/orchestrate.execute_turn",
];
const REQUIRED_DOC_MARKERS: &[&str] = &[
    "execution authority",
    "steel.host.execute_turn",
    "turn-execution",
    "clankers/steel/orchestrate.execute_turn",
    "before any provider request",
    "host-call",
    "JSON",
];
const FORBIDDEN_DOC_MARKERS: &[&str] = &["raw_prompt =", "provider_payload =", "compact_ucan", "credential_value"];

fn main() -> ExitCode {
    match run() {
        Ok(path) => {
            println!("steel execute-turn authority receipt written to {}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("steel execute-turn authority check failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<PathBuf, String> {
    let runtime = read(RUNTIME)?;
    let runtime_lib = read(RUNTIME_LIB)?;
    let agent_execution = read(AGENT_EXECUTION)?;
    let agent_turn = read(AGENT_TURN)?;
    let agent_planning = read(AGENT_PLANNING)?;
    let settings = read(SETTINGS)?;
    let embedded = read(EMBEDDED_TEST)?;
    let doc = read(DOC)?;
    let smoke_doc = read(SMOKE_DOC)?;
    let tasks = read(TASKS)?;
    let root_profile_text = read(ROOT_PROFILE)?;
    let agent_profile_text = read(AGENT_PROFILE)?;
    let root_profile = parse_profile(ROOT_PROFILE, &root_profile_text)?;
    let agent_profile = parse_profile(AGENT_PROFILE, &agent_profile_text)?;

    let mut errors = Vec::new();
    require_all(RUNTIME, &runtime, REQUIRED_RUNTIME_MARKERS, &mut errors);
    require_all(AGENT_EXECUTION, &agent_execution, REQUIRED_AGENT_EXECUTION_MARKERS, &mut errors);
    require_all(EMBEDDED_TEST, &embedded, REQUIRED_EMBEDDED_MARKERS, &mut errors);
    require_all(DOC, &doc, REQUIRED_DOC_MARKERS, &mut errors);
    require_all(SMOKE_DOC, &smoke_doc, REQUIRED_DOC_MARKERS, &mut errors);
    forbid_all(DOC, &doc, FORBIDDEN_DOC_MARKERS, &mut errors);
    forbid_all(SMOKE_DOC, &smoke_doc, FORBIDDEN_DOC_MARKERS, &mut errors);
    validate_profile(ROOT_PROFILE, &root_profile, &mut errors);
    validate_profile(AGENT_PROFILE, &agent_profile, &mut errors);
    for marker in [
        "DEFAULT_TURN_EXECUTION_SEAM",
        "execution_required_session_capabilities",
        "execution_required_ucan_ability",
    ] {
        if !agent_planning.contains(marker) {
            errors.push(format!("{AGENT_PLANNING} missing marker `{marker}`"));
        }
    }
    for marker in ["turn-execution", "clankers/steel/orchestrate.execute_turn"] {
        if !settings.contains(marker) {
            errors.push(format!("{SETTINGS} missing default authority marker `{marker}`"));
        }
    }
    for marker in [
        "SteelTurnExecutionInput",
        "SteelTurnExecutionHostCallPayload",
        "SteelTurnExecutionHostCallReceipt",
        "SteelTurnExecutionReceipt",
        "authorize_steel_turn_execution",
    ] {
        if !runtime_lib.contains(marker) {
            errors.push(format!("{RUNTIME_LIB} must export `{marker}`"));
        }
    }
    if !agent_turn.contains("run_steel_selected_engine_turn(seed, hosts, SteelSelectedExecutionReceiptContext")
        || !agent_turn.contains(".await?")
    {
        errors.push(format!("{AGENT_TURN} must propagate Steel execution authority failures"));
    }
    if !tasks.contains("r[steel-execute-turn-authority.pre-run.denied]") {
        errors.push(format!("{TASKS} must trace the denied pre-run boundary"));
    }
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }

    let artifacts = [
        RUNTIME,
        RUNTIME_LIB,
        AGENT_EXECUTION,
        AGENT_TURN,
        AGENT_PLANNING,
        SETTINGS,
        EMBEDDED_TEST,
        ROOT_PROFILE,
        AGENT_PROFILE,
        DOC,
        SMOKE_DOC,
        TASKS,
        "scripts/check-steel-execute-turn-authority.rs",
    ]
    .iter()
    .map(|path| hash_artifact(Path::new(path)))
    .collect::<Result<Vec<_>, _>>()?;
    let receipt = json!({
        "schema": "clankers.steel_execute_turn_authority.check_receipt.v1",
        "validated_surfaces": [
            "profile-execute-host-action",
            "runtime-execution-dto",
            "dynamic-runtime-authorization",
            "agent-pre-run-denial",
            "daemon-visible-allowed-and-denied-receipts",
            "redacted-docs-and-smoke"
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

fn parse_profile(path: &str, text: &str) -> Result<Value, String> {
    serde_json::from_str(text).map_err(|error| format!("failed to parse {path}: {error}"))
}

fn validate_profile(path: &str, profile: &Value, errors: &mut Vec<String>) {
    let Some(actions) = profile.get("allowed_host_actions").and_then(Value::as_array) else {
        errors.push(format!("{path} missing allowed_host_actions"));
        return;
    };
    let mut saw_execute = false;
    for action in actions {
        let name = action.get("name").and_then(Value::as_str).unwrap_or_default();
        if name == "steel.host.execute_turn" {
            saw_execute = true;
            if action.get("dynamic_runtime_action").and_then(Value::as_str)
                != Some("host_function:steel.host.execute_turn")
            {
                errors.push(format!("{path} execute action must use exact host_function dynamic action"));
            }
            if !string_array(action, "required_session_capabilities").iter().any(|cap| cap == "turn-execution") {
                errors.push(format!("{path} execute action must require turn-execution"));
            }
            if action.get("ucan_ability").and_then(Value::as_str) != Some("clankers/steel/orchestrate.execute_turn") {
                errors.push(format!("{path} execute action must require execute_turn UCAN ability"));
            }
        }
    }
    if !saw_execute {
        errors.push(format!("{path} must expose steel.host.execute_turn"));
    }
}

fn string_array(value: &Value, field: &str) -> Vec<String> {
    value
        .get(field)
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).map(ToString::to_string).collect::<Vec<_>>())
        .unwrap_or_default()
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

fn hash_artifact(path: &Path) -> Result<Value, String> {
    let bytes = fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let hash = blake3::hash(&bytes).to_hex().to_string();
    Ok(json!({
        "path": path.display().to_string(),
        "bytes": bytes.len(),
        "blake3": hash,
    }))
}
