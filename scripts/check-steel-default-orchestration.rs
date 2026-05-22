#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"

[dependencies]
blake3 = "1"
serde_json = "1"
---

use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;

use serde_json::Value;
use serde_json::json;

const ERROR_EXIT: u8 = 1;
const PROFILE_JSON: &str = "policy/steel-default-orchestration/orchestration-profile.json";
const PROFILE_NICKEL: &str = "policy/steel-default-orchestration/orchestration-profile.ncl";
const INVALID_PROFILE_JSON: &str = "policy/steel-default-orchestration/invalid-orchestration-profile.json";
const SCRIPT_PATH: &str = "policy/steel-default-orchestration/scripts/default-plan-turn.scm";
const RUNTIME_SOURCE: &str = "crates/clankers-runtime/src/steel_orchestration.rs";
const RUNTIME_LIB: &str = "crates/clankers-runtime/src/lib.rs";
const DOC_PATH: &str = "docs/src/reference/steel-default-orchestration.md";
const DEFAULT_OUTPUT: &str = "target/steel-default-orchestration/profile-receipt.json";
const EXPECTED_SCHEMA: &str = "clankers.steel_default_orchestration.profile.v1";
const EXPECTED_RECEIPT_SCHEMA: &str = "clankers.steel_default_orchestration.receipt.v1";
const ALLOWED_SEAMS: &[&str] = &["steel.host.plan_turn", "steel.host.route_action"];
const REQUIRED_REDACTED_FIELDS: &[&str] = &[
    "raw_prompt",
    "provider_payload",
    "compact_ucan",
    "raw_proof",
    "credential",
    "script_source",
    "tool_body",
    "absolute_path",
];
const FORBIDDEN_SAFE_FIELDS: &[&str] = &[
    "raw_prompt",
    "provider_payload",
    "compact_ucan",
    "raw_proof",
    "credential",
    "script_source",
    "tool_body",
    "absolute_path",
];
const REQUIRED_NICKEL_MARKERS: &[&str] = &[
    "OrchestrationProfile",
    "ScriptBinding",
    "RuntimeBudget",
    "HostAction",
    "ReceiptPolicy",
];
const REQUIRED_RUNTIME_MARKERS: &[&str] = &[
    "SteelOrchestrationProfile",
    "TurnPlanningInput",
    "OrchestrationPlan",
    "OrchestrationPlanReceipt",
    "plan_turn_with_steel_or_fallback",
    "evaluate_steel_request",
    "authorize_dynamic_runtime_action",
];
const FORBIDDEN_DIRECT_IMPORT_MARKERS: &[&str] = &[
    "steel_core::",
    "steel::steel_vm",
    "steel_vm::",
    "Engine::new()",
];

fn main() -> ExitCode {
    match run() {
        Ok(path) => {
            println!("steel default orchestration receipt written to {}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("steel default orchestration check failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<PathBuf, String> {
    let profile_text = fs::read_to_string(PROFILE_JSON).map_err(|error| format!("failed to read {PROFILE_JSON}: {error}"))?;
    let profile: Value = serde_json::from_str(&profile_text).map_err(|error| format!("failed to parse {PROFILE_JSON}: {error}"))?;
    let invalid_text = fs::read_to_string(INVALID_PROFILE_JSON)
        .map_err(|error| format!("failed to read {INVALID_PROFILE_JSON}: {error}"))?;
    let invalid_profile: Value = serde_json::from_str(&invalid_text)
        .map_err(|error| format!("failed to parse {INVALID_PROFILE_JSON}: {error}"))?;
    let nickel_text = fs::read_to_string(PROFILE_NICKEL).map_err(|error| format!("failed to read {PROFILE_NICKEL}: {error}"))?;
    let runtime_text = fs::read_to_string(RUNTIME_SOURCE).map_err(|error| format!("failed to read {RUNTIME_SOURCE}: {error}"))?;
    let lib_text = fs::read_to_string(RUNTIME_LIB).map_err(|error| format!("failed to read {RUNTIME_LIB}: {error}"))?;
    let doc_text = fs::read_to_string(DOC_PATH).map_err(|error| format!("failed to read {DOC_PATH}: {error}"))?;

    let mut errors = Vec::new();
    validate_nickel_markers(&nickel_text, &mut errors);
    validate_profile(&profile, &mut errors);
    validate_invalid_fixture(&invalid_profile, &mut errors);
    validate_runtime_boundaries(&runtime_text, &lib_text, &mut errors);
    validate_docs(&doc_text, &mut errors);
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }

    let hashed_artifacts = [
        PROFILE_JSON,
        PROFILE_NICKEL,
        INVALID_PROFILE_JSON,
        SCRIPT_PATH,
        RUNTIME_SOURCE,
        RUNTIME_LIB,
        DOC_PATH,
        "scripts/check-steel-default-orchestration.rs",
    ]
    .iter()
    .map(|path| hash_artifact(Path::new(path)))
    .collect::<Result<Vec<_>, _>>()?;
    let receipt = json!({
        "schema": "clankers.steel_default_orchestration.profile_check_receipt.v1",
        "profile": PROFILE_JSON,
        "nickel_contract": PROFILE_NICKEL,
        "invalid_fixture": INVALID_PROFILE_JSON,
        "validated_surfaces": [
            "policy-selected-default-profile",
            "named-planning-seam",
            "script-hash-required",
            "fallback-policy",
            "allowed-host-action-scope",
            "receipt-redaction",
            "rust-wrapper-only-adapter",
            "operator-docs"
        ],
        "hashed_artifacts": hashed_artifacts,
        "guidance": "Steel Scheme is selected by Nickel policy as a planning seam only; Rust still authorizes every dynamic-runtime envelope and owns fallback/effects."
    });
    let output_path = PathBuf::from(DEFAULT_OUTPUT);
    let parent = output_path.parent().ok_or_else(|| format!("{} has no parent", output_path.display()))?;
    fs::create_dir_all(parent).map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let bytes = serde_json::to_vec_pretty(&receipt).map_err(|error| format!("failed to encode receipt: {error}"))?;
    fs::write(&output_path, [bytes.as_slice(), b"\n"].concat())
        .map_err(|error| format!("failed to write {}: {error}", output_path.display()))?;
    Ok(output_path)
}

fn validate_nickel_markers(text: &str, errors: &mut Vec<String>) {
    for marker in REQUIRED_NICKEL_MARKERS {
        if !text.contains(marker) {
            errors.push(format!("{PROFILE_NICKEL} missing marker `{marker}`"));
        }
    }
}

fn validate_profile(profile: &Value, errors: &mut Vec<String>) {
    if required_str(profile, "schema", errors) != EXPECTED_SCHEMA {
        errors.push(format!("profile schema must be {EXPECTED_SCHEMA}"));
    }
    let seam = required_str(profile, "planning_seam", errors);
    if !ALLOWED_SEAMS.contains(&seam) {
        errors.push(format!("planning seam `{seam}` is not reviewed"));
    }
    if profile.get("enabled").and_then(Value::as_bool) == Some(true)
        && profile.get("default").and_then(Value::as_bool) == Some(true)
        && seam.is_empty()
    {
        errors.push("default Steel orchestration profile must name a seam".to_string());
    }
    let rollout = required_str(profile, "rollout_stage", errors);
    if !["comparison", "default", "disabled"].contains(&rollout) {
        errors.push(format!("unsupported rollout stage `{rollout}`"));
    }
    let fallback = required_str(profile, "fallback_mode", errors);
    if !["rust_native", "block"].contains(&fallback) {
        errors.push(format!("unsupported fallback mode `{fallback}`"));
    }
    validate_script(profile.get("script"), errors);
    validate_budget(profile.get("runtime_budget"), errors);
    validate_host_actions(profile, errors);
    validate_receipt_policy(profile.get("receipt_policy"), errors);
    validate_audit(profile.get("audit"), errors);
}

fn validate_script(script: Option<&Value>, errors: &mut Vec<String>) {
    let Some(script) = object(script, "script", errors) else { return };
    for field in ["id", "source_kind", "path"] {
        if required_str(script, field, errors).is_empty() {
            errors.push(format!("script field `{field}` must be non-empty"));
        }
    }
    let hash = required_str(script, "blake3", errors);
    if !hash.starts_with("b3:") || hash.len() < 67 || hash.contains('*') {
        errors.push("script blake3 must be pinned as b3:<64 hex chars>".to_string());
    }
}

fn validate_budget(budget: Option<&Value>, errors: &mut Vec<String>) {
    let Some(budget) = object(budget, "runtime_budget", errors) else { return };
    if required_str(budget, "steel_profile", errors).is_empty() {
        errors.push("runtime_budget steel_profile must be non-empty".to_string());
    }
    for field in ["max_source_bytes", "max_output_bytes", "max_host_calls", "max_steps", "max_plan_items", "max_input_bytes"] {
        if required_u64(budget, field, errors) == 0 {
            errors.push(format!("runtime_budget field `{field}` must be positive"));
        }
    }
}

fn validate_host_actions(profile: &Value, errors: &mut Vec<String>) {
    let seam = required_str(profile, "planning_seam", errors).to_string();
    let mut names = BTreeSet::new();
    for action in array(profile, "allowed_host_actions", errors) {
        let name = required_str(action, "name", errors).to_string();
        if !name.is_empty() && !names.insert(name.clone()) {
            errors.push(format!("duplicate host action `{name}`"));
        }
        if name != seam {
            errors.push(format!("host action `{name}` does not match selected seam `{seam}`"));
        }
        let key = required_str(action, "dynamic_runtime_action", errors);
        if key == "*" || key.ends_with(":*") || !key.starts_with("host_function:") {
            errors.push(format!("host action `{name}` has unsafe dynamic action `{key}`"));
        }
        let target_prefix = required_str(action, "target_prefix", errors);
        if target_prefix == "/" || target_prefix.contains("..") || target_prefix.is_empty() {
            errors.push(format!("host action `{name}` has unsafe target prefix `{target_prefix}`"));
        }
        if string_set(action, "required_session_capabilities", errors).is_empty() {
            errors.push(format!("host action `{name}` must require session capabilities"));
        }
        let ability = required_str(action, "ucan_ability", errors);
        if ability == "*" || !ability.starts_with("clankers/") {
            errors.push(format!("host action `{name}` has unsafe UCAN ability `{ability}`"));
        }
    }
    if names.is_empty() {
        errors.push("profile must expose at least one reviewed host action".to_string());
    }
}

fn validate_receipt_policy(receipt: Option<&Value>, errors: &mut Vec<String>) {
    let Some(receipt) = object(receipt, "receipt_policy", errors) else { return };
    if required_str(receipt, "schema", errors) != EXPECTED_RECEIPT_SCHEMA {
        errors.push(format!("receipt schema must be {EXPECTED_RECEIPT_SCHEMA}"));
    }
    let destination = required_str(receipt, "destination_prefix", errors);
    if !destination.starts_with("target/") {
        errors.push("receipt destination must stay under target/".to_string());
    }
    let redaction = required_str(receipt, "redaction", errors);
    if !["metadata_only", "public_summary"].contains(&redaction) {
        errors.push(format!("unsafe receipt redaction `{redaction}`"));
    }
    let safe = string_set(receipt, "safe_fields", errors);
    let redacted = string_set(receipt, "redacted_fields", errors);
    for forbidden in FORBIDDEN_SAFE_FIELDS {
        if safe.contains(*forbidden) {
            errors.push(format!("receipt safe_fields must not include `{forbidden}`"));
        }
    }
    for required in REQUIRED_REDACTED_FIELDS {
        if !redacted.contains(*required) {
            errors.push(format!("receipt redacted_fields missing `{required}`"));
        }
    }
}

fn validate_audit(audit: Option<&Value>, errors: &mut Vec<String>) {
    let Some(audit) = object(audit, "audit", errors) else { return };
    if required_str(audit, "owner", errors).is_empty() {
        errors.push("audit owner must be non-empty".to_string());
    }
    if audit.get("review_required").and_then(Value::as_bool) != Some(true) {
        errors.push("audit review_required must be true".to_string());
    }
    if audit.get("expansion_requires_profile_update").and_then(Value::as_bool) != Some(true) {
        errors.push("audit expansion must require profile update".to_string());
    }
}

fn validate_invalid_fixture(profile: &Value, errors: &mut Vec<String>) {
    let mut invalid_errors = Vec::new();
    validate_profile(profile, &mut invalid_errors);
    for expected in ["planning seam", "fallback", "blake3", "unsafe dynamic", "target prefix", "UCAN", "redaction", "review_required"] {
        if !invalid_errors.iter().any(|error| error.contains(expected)) {
            errors.push(format!("invalid fixture did not trigger expected error containing `{expected}`"));
        }
    }
}

fn validate_runtime_boundaries(runtime_text: &str, lib_text: &str, errors: &mut Vec<String>) {
    for marker in REQUIRED_RUNTIME_MARKERS {
        if !runtime_text.contains(marker) {
            errors.push(format!("{RUNTIME_SOURCE} missing marker `{marker}`"));
        }
    }
    for forbidden in FORBIDDEN_DIRECT_IMPORT_MARKERS {
        if runtime_text.contains(forbidden) {
            errors.push(format!("{RUNTIME_SOURCE} directly references forbidden interpreter API `{forbidden}`"));
        }
    }
    if !lib_text.contains("pub mod steel_orchestration;") {
        errors.push("runtime lib must expose steel_orchestration module".to_string());
    }
    if !lib_text.contains("pub use steel_orchestration::plan_turn_with_steel_or_fallback;") {
        errors.push("runtime lib must re-export planner seam entrypoint".to_string());
    }
}

fn validate_docs(text: &str, errors: &mut Vec<String>) {
    for marker in [
        "Steel Scheme",
        "Rust remains",
        "Nickel",
        "UCAN",
        "no ambient",
        "fallback",
        "dynamic-runtime",
        "not an OS/process sandbox",
    ] {
        if !text.contains(marker) {
            errors.push(format!("{DOC_PATH} missing required wording `{marker}`"));
        }
    }
}

fn object<'a>(value: Option<&'a Value>, label: &str, errors: &mut Vec<String>) -> Option<&'a Value> {
    match value {
        Some(Value::Object(_)) => value,
        _ => {
            errors.push(format!("{label} must be an object"));
            None
        }
    }
}

fn array<'a>(value: &'a Value, field: &str, errors: &mut Vec<String>) -> Vec<&'a Value> {
    match value.get(field) {
        Some(Value::Array(items)) => items.iter().collect(),
        Some(_) => {
            errors.push(format!("field `{field}` must be an array"));
            Vec::new()
        }
        None => {
            errors.push(format!("missing array field `{field}`"));
            Vec::new()
        }
    }
}

fn string_set(value: &Value, field: &str, errors: &mut Vec<String>) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for item in array(value, field, errors) {
        match item.as_str() {
            Some(text) if !text.is_empty() => {
                set.insert(text.to_string());
            }
            _ => errors.push(format!("field `{field}` must contain only non-empty strings")),
        }
    }
    set
}

fn required_str<'a>(value: &'a Value, field: &str, errors: &mut Vec<String>) -> &'a str {
    match value.get(field).and_then(Value::as_str) {
        Some(text) => text,
        None => {
            errors.push(format!("missing string field `{field}`"));
            ""
        }
    }
}

fn required_u64(value: &Value, field: &str, errors: &mut Vec<String>) -> u64 {
    match value.get(field).and_then(Value::as_u64) {
        Some(number) => number,
        None => {
            errors.push(format!("missing unsigned integer field `{field}`"));
            0
        }
    }
}

fn hash_artifact(path: &Path) -> Result<Value, String> {
    let mut file = fs::File::open(path).map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; 8192];
    let mut bytes = 0_u64;
    loop {
        let read = file.read(&mut buffer).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        bytes += read as u64;
        hasher.update(&buffer[..read]);
    }
    Ok(json!({
        "path": path.display().to_string(),
        "blake3": format!("b3:{}", hasher.finalize().to_hex()),
        "bytes": bytes,
    }))
}
