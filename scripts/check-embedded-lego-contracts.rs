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
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::{json, Value};

const ERROR_EXIT: u8 = 1;
const POLICY_JSON: &str = "policy/embedded-lego/lego-contracts.json";
const POLICY_NICKEL: &str = "policy/embedded-lego/lego-contracts.ncl";
const DEFAULT_OUTPUT: &str = "target/embedded-sdk-release/lego-contracts-receipt.json";
const SAFE_PACKS: &[&str] = &["embedding_safe", "read_only", "networkless_coding"];
const DANGEROUS_CAPABILITIES: &[&str] = &["shell", "network", "raw-log", "secret-adjacent"];
const REQUIRED_GREEN_CRATES: &[&str] = &[
    "clanker-message",
    "clankers-engine",
    "clankers-engine-host",
    "clankers-tool-host",
    "clankers-adapters",
];
const REQUIRED_RED_CRATES: &[&str] = &["clankers-provider", "clankers-tui", "clankers-session", "iroh"];
const EVIDENCE_ARTIFACTS: &[&str] = &[
    POLICY_JSON,
    POLICY_NICKEL,
    "examples/embedded-product-workbench/Cargo.toml",
    "examples/embedded-product-workbench/dogfood-manifest.json",
    "examples/embedded-product-workbench/src/main.rs",
    "scripts/check-real-product-dogfood.rs",
    "examples/embedded-session-store/src/main.rs",
    "examples/embedded-session-store/session-resume-evidence.json",
    "scripts/check-session-resume-brick.rs",
    "examples/embedded-tool-kit/tool-catalog-manifest.json",
    "scripts/check-tool-catalog-manifest.rs",
    "policy/embedded-lego/capability-pack-composition.json",
    "scripts/check-capability-pack-composition.rs",
    "examples/embedded-provider-adapter/src/main.rs",
    "examples/embedded-provider-adapter/fixtures/provider-adapter-fixtures.json",
    "scripts/check-provider-adapter-kit.rs",
    "docs/src/tutorials/embedded-agent-sdk.md",
    "docs/src/generated/embedded-sdk-api.md",
];

fn main() -> ExitCode {
    match run() {
        Ok(path) => {
            println!("embedded lego contracts receipt written to {}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("embedded lego contracts check failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<PathBuf, String> {
    let policy_text = fs::read_to_string(POLICY_JSON).map_err(|error| format!("failed to read {POLICY_JSON}: {error}"))?;
    let policy: Value = serde_json::from_str(&policy_text).map_err(|error| format!("failed to parse {POLICY_JSON}: {error}"))?;
    let nickel_text = fs::read_to_string(POLICY_NICKEL).map_err(|error| format!("failed to read {POLICY_NICKEL}: {error}"))?;

    let mut errors = Vec::new();
    validate_nickel_contract_sketch(&nickel_text, &mut errors);
    validate_crate_boundaries(&policy, &mut errors);
    validate_capability_packs(&policy, &mut errors);
    validate_capability_pack_composition(&policy, &mut errors);
    validate_tool_catalog(&policy, &mut errors);
    validate_product_dogfood(&policy, &mut errors);
    validate_provider_fixtures(&policy, &mut errors);
    validate_session_resume(&policy, &mut errors);
    validate_runtime_dispatch(&policy, &mut errors);

    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }

    let artifacts = EVIDENCE_ARTIFACTS
        .iter()
        .map(|path| hash_artifact(Path::new(path)))
        .collect::<Result<Vec<_>, _>>()?;
    let receipt = json!({
        "schema": "clankers.embedded_lego.contracts_receipt.v1",
        "policy": POLICY_JSON,
        "nickel_contract": POLICY_NICKEL,
        "validated_surfaces": [
            "crate-boundaries",
            "capability-pack-composition",
            "tool-catalog-manifest",
            "product-dogfood-manifest",
            "provider-adapter-fixtures",
            "session-resume-evidence",
            "runtime-dispatch-matrix"
        ],
        "hashed_artifacts": artifacts,
        "guidance": "Nickel is an author-time/export boundary; generic SDK crates consume Rust DTOs or generated fixtures. BLAKE3 hashes provide deterministic evidence and drift detection, not authorization."
    });

    let output_path = PathBuf::from(DEFAULT_OUTPUT);
    let parent = output_path.parent().ok_or_else(|| format!("{} has no parent", output_path.display()))?;
    fs::create_dir_all(parent).map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let bytes = serde_json::to_vec_pretty(&receipt).map_err(|error| format!("failed to encode receipt: {error}"))?;
    fs::write(&output_path, [bytes.as_slice(), b"\n"].concat())
        .map_err(|error| format!("failed to write {}: {error}", output_path.display()))?;
    Ok(output_path)
}

fn validate_nickel_contract_sketch(text: &str, errors: &mut Vec<String>) {
    for marker in [
        "BoundaryPolicy",
        "CapabilityPack",
        "ToolManifest",
        "ProviderFixture",
        "SessionResumeEvidence",
        "RuntimeDispatch",
    ] {
        if !text.contains(marker) {
            errors.push(format!("{POLICY_NICKEL} missing Nickel contract marker `{marker}`"));
        }
    }
}

fn validate_crate_boundaries(policy: &Value, errors: &mut Vec<String>) {
    let Some(boundaries) = policy.get("crate_boundaries") else {
        errors.push("policy missing crate_boundaries".to_string());
        return;
    };
    let green = string_set(boundaries, "green", errors);
    let red = string_set(boundaries, "red", errors);
    for required in REQUIRED_GREEN_CRATES {
        if !green.contains(*required) {
            errors.push(format!("green crate boundary missing `{required}`"));
        }
    }
    for forbidden in REQUIRED_RED_CRATES {
        if !red.contains(*forbidden) {
            errors.push(format!("red crate boundary missing `{forbidden}`"));
        }
        if green.contains(*forbidden) {
            errors.push(format!("forbidden shell crate `{forbidden}` appears in green boundary"));
        }
    }
}

fn validate_capability_packs(policy: &Value, errors: &mut Vec<String>) {
    let packs = array(policy, "capability_packs", errors);
    let mut names = BTreeSet::new();
    for pack in packs {
        let name = required_str(pack, "name", errors);
        if !name.is_empty() && !names.insert(name.to_string()) {
            errors.push(format!("duplicate capability pack `{name}`"));
        }
        let capabilities = string_set(pack, "capabilities", errors);
        let dangerous = pack.get("dangerous").and_then(Value::as_bool).unwrap_or(false);
        let approval = pack.get("requires_human_approval").and_then(Value::as_bool).unwrap_or(false);
        if SAFE_PACKS.contains(&name) {
            for capability in DANGEROUS_CAPABILITIES {
                if capabilities.contains(*capability) {
                    errors.push(format!("safe pack `{name}` includes dangerous capability `{capability}`"));
                }
            }
        }
        if dangerous && !approval {
            errors.push(format!("dangerous pack `{name}` lacks human approval requirement"));
        }
    }
}

fn validate_capability_pack_composition(policy: &Value, errors: &mut Vec<String>) {
    let fixture = required_str(policy, "capability_pack_composition_fixture", errors);
    if fixture != "policy/embedded-lego/capability-pack-composition.json" {
        errors.push("capability_pack_composition_fixture must point at the checked composition fixture".to_string());
    }
    if !fixture.is_empty() && !Path::new(fixture).exists() {
        errors.push(format!("capability pack composition fixture `{fixture}` does not exist"));
    }
}

fn validate_tool_catalog(policy: &Value, errors: &mut Vec<String>) {
    let manifest = required_str(policy, "tool_catalog_manifest", errors);
    if manifest != "examples/embedded-tool-kit/tool-catalog-manifest.json" {
        errors.push("tool_catalog_manifest must point at the checked embedded tool catalog manifest".to_string());
    }
    if !manifest.is_empty() && !Path::new(manifest).exists() {
        errors.push(format!("tool catalog manifest `{manifest}` does not exist"));
    }
    let tools = array(policy, "tool_catalog", errors);
    let mut names = BTreeSet::new();
    for tool in tools {
        let name = required_str(tool, "name", errors);
        if !name.is_empty() && !names.insert(name.to_string()) {
            errors.push(format!("duplicate tool catalog entry `{name}`"));
        }
        let runtime = required_str(tool, "runtime", errors);
        let approval = required_str(tool, "approval", errors);
        let redaction = required_str(tool, "redaction", errors);
        let capabilities = string_set(tool, "capabilities", errors);
        if runtime.is_empty() || required_str(tool, "schema_hash_input", errors).is_empty() {
            errors.push(format!("tool `{name}` missing runtime or schema hash input"));
        }
        if capabilities.contains("shell") && approval != "always" {
            errors.push(format!("dangerous tool `{name}` must require always approval"));
        }
        if capabilities.contains("secret-adjacent") && redaction == "none" {
            errors.push(format!("secret-adjacent tool `{name}` must declare redaction"));
        }
    }
}

fn validate_product_dogfood(policy: &Value, errors: &mut Vec<String>) {
    let Some(dogfood) = policy.get("product_dogfood") else {
        errors.push("policy missing product_dogfood".to_string());
        return;
    };
    let manifest = required_str(dogfood, "manifest", errors);
    if manifest.is_empty() || !Path::new(manifest).exists() {
        errors.push(format!("product dogfood manifest `{manifest}` does not exist"));
    }
    if !manifest.ends_with("dogfood-manifest.json") {
        errors.push(format!("product dogfood manifest `{manifest}` must be a checked manifest JSON, not only Cargo metadata"));
    }
    validate_product_dogfood_manifest(manifest, errors);
    let deps = string_set(dogfood, "allowed_green_deps", errors);
    for dep in ["clankers-provider", "clankers-tui", "clankers-session", "iroh"] {
        if deps.contains(dep) {
            errors.push(format!("product dogfood allows red dependency `{dep}`"));
        }
    }
}

fn validate_product_dogfood_manifest(path: &str, errors: &mut Vec<String>) {
    if path.is_empty() {
        return;
    }
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    let Ok(manifest) = serde_json::from_str::<Value>(&text) else {
        errors.push(format!("product dogfood manifest `{path}` is not valid JSON"));
        return;
    };
    for field in [
        "selected_green_crates",
        "capability_packs",
        "tool_catalog_refs",
        "provider_seam",
        "session_seam",
        "shell_exclusions",
        "follow_up_policy",
    ] {
        if manifest.get(field).is_none() {
            errors.push(format!("product dogfood manifest missing `{field}`"));
        }
    }
    if manifest.pointer("/provider_seam/live_credentials") != Some(&Value::Bool(false)) {
        errors.push("product dogfood manifest must disable live credentials".to_string());
    }
    if manifest.pointer("/provider_seam/network_access") != Some(&Value::Bool(false)) {
        errors.push("product dogfood manifest must disable network access".to_string());
    }
    if manifest.pointer("/session_seam/opens_clankers_db") != Some(&Value::Bool(false)) {
        errors.push("product dogfood manifest must keep Clankers DB out of the product seam".to_string());
    }
}

fn validate_provider_fixtures(policy: &Value, errors: &mut Vec<String>) {
    let fixtures = array(policy, "provider_adapter_fixtures", errors);
    let names = fixtures.iter().map(|fixture| required_str(fixture, "name", errors)).collect::<BTreeSet<_>>();
    for required in ["completed", "retryable-failure", "terminal-failure"] {
        if !names.contains(required) {
            errors.push(format!("provider adapter fixtures missing `{required}`"));
        }
    }
}

fn validate_session_resume(policy: &Value, errors: &mut Vec<String>) {
    let fixture = required_str(policy, "session_resume_evidence_fixture", errors);
    if fixture != "examples/embedded-session-store/session-resume-evidence.json" {
        errors.push("session_resume_evidence_fixture must point at the checked session resume fixture".to_string());
    }
    if !fixture.is_empty() && !Path::new(fixture).exists() {
        errors.push(format!("session resume evidence fixture `{fixture}` does not exist"));
    }
    let entries = array(policy, "session_resume_evidence", errors);
    if entries.len() < 2 {
        errors.push("session resume evidence must cover at least two product-style examples".to_string());
    }
    for entry in entries {
        let product = required_str(entry, "product", errors);
        if required_str(entry, "evidence_source", errors) != fixture {
            errors.push(format!("session evidence `{product}` must reference fixture `{fixture}`"));
        }
        for flag in ["restored_context", "missing_session_fail_closed", "owns_storage_dto"] {
            if entry.get(flag).and_then(Value::as_bool) != Some(true) {
                errors.push(format!("session evidence `{product}` must set {flag}=true"));
            }
        }
    }
}

fn validate_runtime_dispatch(policy: &Value, errors: &mut Vec<String>) {
    let entries = array(policy, "runtime_dispatch_matrix", errors);
    let mut kinds = BTreeSet::new();
    for entry in entries {
        let kind = required_str(entry, "kind", errors);
        let loader = required_str(entry, "loader", errors);
        kinds.insert(kind.to_string());
        if kind != loader && !(kind == "extism" && loader == "wasm") {
            errors.push(format!("runtime kind `{kind}` maps to unexpected loader `{loader}`"));
        }
        let forbidden = string_set(entry, "forbidden_loaders", errors);
        if forbidden.contains(loader) {
            errors.push(format!("runtime kind `{kind}` forbids its selected loader `{loader}`"));
        }
    }
    for required in ["extism", "stdio", "built-in", "product-owned"] {
        if !kinds.contains(required) {
            errors.push(format!("runtime dispatch matrix missing `{required}`"));
        }
    }
}

fn array<'a>(value: &'a Value, field: &str, errors: &mut Vec<String>) -> &'a [Value] {
    match value.get(field).and_then(Value::as_array) {
        Some(array) => array.as_slice(),
        None => {
            errors.push(format!("missing array field `{field}`"));
            &[]
        }
    }
}

fn string_set(value: &Value, field: &str, errors: &mut Vec<String>) -> BTreeSet<String> {
    array(value, field, errors)
        .iter()
        .filter_map(|item| match item.as_str() {
            Some(text) => Some(text.to_string()),
            None => {
                errors.push(format!("field `{field}` contains a non-string item"));
                None
            }
        })
        .collect()
}

fn required_str<'a>(value: &'a Value, field: &str, errors: &mut Vec<String>) -> &'a str {
    match value.get(field).and_then(Value::as_str) {
        Some(text) if !text.is_empty() => text,
        _ => {
            errors.push(format!("missing non-empty string field `{field}`"));
            ""
        }
    }
}

fn hash_artifact(path: &Path) -> Result<Value, String> {
    let mut file = fs::File::open(path).map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    let mut hasher = blake3::Hasher::new();
    let mut bytes = 0u64;
    let mut buffer = [0u8; 16 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        bytes += read as u64;
        hasher.update(&buffer[..read]);
    }
    Ok(json!({
        "path": path.to_string_lossy(),
        "bytes": bytes,
        "blake3": hasher.finalize().to_hex().to_string(),
    }))
}
