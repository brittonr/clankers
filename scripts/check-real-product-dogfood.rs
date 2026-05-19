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
use std::process::ExitCode;

use serde_json::{json, Value};

const ERROR_EXIT: u8 = 1;
const POLICY_JSON: &str = "policy/embedded-lego/lego-contracts.json";
const DEFAULT_OUTPUT_DIR: &str = "target/embedded-sdk-release/product-dogfood";
const FORBIDDEN_SURFACES: &[&str] = &[
    "clankers-provider",
    "clankers-tui",
    "clankers-session",
    "clankers-db",
    "clankers-protocol",
    "clanker-router",
    "iroh",
    "ratatui",
    "crossterm",
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("real product dogfood receipt written to {DEFAULT_OUTPUT_DIR}/receipt.json");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("real product dogfood check failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), String> {
    let policy = json_file(POLICY_JSON)?;
    let dogfood = policy
        .get("product_dogfood")
        .ok_or_else(|| "policy missing product_dogfood".to_string())?;
    let manifest_path = required_str(dogfood, "manifest")?;
    let allowed_green_deps = string_set(dogfood, "allowed_green_deps")?;
    let manifest = json_file(manifest_path)?;

    validate_manifest_shape(&manifest)?;
    validate_dependency_boundary(&manifest, &allowed_green_deps)?;
    validate_source_boundary(&manifest)?;
    validate_runtime_evidence(&manifest)?;

    let report = dependency_boundary_report(&manifest, &allowed_green_deps)?;
    let transcript = sanitized_transcript(&manifest)?;
    fs::create_dir_all(DEFAULT_OUTPUT_DIR).map_err(|error| format!("failed to create {DEFAULT_OUTPUT_DIR}: {error}"))?;
    write_json(&format!("{DEFAULT_OUTPUT_DIR}/dependency-boundary-report.json"), &report)?;
    write_json(&format!("{DEFAULT_OUTPUT_DIR}/sanitized-transcript.json"), &transcript)?;

    let receipt = json!({
        "schema": "clankers.embedded_product_dogfood.receipt.v1",
        "product": required_str(&manifest, "product")?,
        "evidence": {
            "manifest": hash_artifact(Path::new(manifest_path))?,
            "dependency_boundary_report": hash_artifact(Path::new(&format!("{DEFAULT_OUTPUT_DIR}/dependency-boundary-report.json")))?,
            "sanitized_transcript": hash_artifact(Path::new(&format!("{DEFAULT_OUTPUT_DIR}/sanitized-transcript.json")))?,
            "executable_recipe": hash_artifact(Path::new(required_str(&manifest, "executable_recipe")?))?,
        },
        "no_live_credentials": manifest.pointer("/provider_seam/live_credentials") == Some(&Value::Bool(false)),
        "no_network_access": manifest.pointer("/provider_seam/network_access") == Some(&Value::Bool(false)),
        "follow_up_policy": required_str(&manifest, "follow_up_policy")?,
    });
    write_json(&format!("{DEFAULT_OUTPUT_DIR}/receipt.json"), &receipt)?;
    Ok(())
}

fn validate_manifest_shape(manifest: &Value) -> Result<(), String> {
    require_eq(manifest, "schema", "clankers.embedded_product_dogfood.manifest.v1")?;
    require_eq(manifest, "product", "embedded-product-workbench")?;
    for field in [
        "cargo_manifest",
        "executable_recipe",
        "selected_green_crates",
        "capability_packs",
        "tool_catalog_refs",
        "provider_seam",
        "session_seam",
        "shell_exclusions",
        "follow_up_policy",
    ] {
        if manifest.get(field).is_none() {
            return Err(format!("dogfood manifest missing `{field}`"));
        }
    }
    if !Path::new(required_str(manifest, "cargo_manifest")?).exists() {
        return Err("dogfood cargo_manifest path does not exist".to_string());
    }
    if !Path::new(required_str(manifest, "executable_recipe")?).exists() {
        return Err("dogfood executable_recipe path does not exist".to_string());
    }
    Ok(())
}

fn validate_dependency_boundary(manifest: &Value, allowed: &BTreeSet<String>) -> Result<(), String> {
    let selected = string_set(manifest, "selected_green_crates")?;
    for dep in &selected {
        if !allowed.contains(dep) {
            return Err(format!("dogfood selected dependency `{dep}` is not policy-allowed green"));
        }
    }
    for forbidden in FORBIDDEN_SURFACES {
        if selected.contains(*forbidden) {
            return Err(format!("dogfood selected forbidden dependency `{forbidden}`"));
        }
    }

    let cargo = fs::read_to_string(required_str(manifest, "cargo_manifest")?)
        .map_err(|error| format!("failed to read dogfood Cargo.toml: {error}"))?;
    for dep in &selected {
        if !cargo.contains(dep) {
            return Err(format!("dogfood Cargo.toml missing selected dependency `{dep}`"));
        }
    }
    for forbidden in FORBIDDEN_SURFACES {
        if cargo.contains(forbidden) {
            return Err(format!("dogfood Cargo.toml imports forbidden surface `{forbidden}`"));
        }
    }
    Ok(())
}

fn validate_source_boundary(manifest: &Value) -> Result<(), String> {
    let source_path = required_str(manifest, "executable_recipe")?;
    let source = fs::read_to_string(source_path).map_err(|error| format!("failed to read {source_path}: {error}"))?;
    for marker in ["ProductModelAdapter", "ProductSessionStore", "CatalogToolExecutor", "ProductTurnReceipt"] {
        if !source.contains(marker) {
            return Err(format!("dogfood source missing product-owned seam marker `{marker}`"));
        }
    }
    for forbidden in FORBIDDEN_SURFACES {
        if source.contains(forbidden) {
            return Err(format!("dogfood source imports forbidden surface `{forbidden}`"));
        }
    }
    Ok(())
}

fn validate_runtime_evidence(manifest: &Value) -> Result<(), String> {
    if manifest.pointer("/provider_seam/owner") != Some(&Value::String("product".to_string())) {
        return Err("dogfood provider seam must be product-owned".to_string());
    }
    if manifest.pointer("/provider_seam/live_credentials") != Some(&Value::Bool(false)) {
        return Err("dogfood provider seam must disable live credentials".to_string());
    }
    if manifest.pointer("/provider_seam/network_access") != Some(&Value::Bool(false)) {
        return Err("dogfood provider seam must disable network access".to_string());
    }
    if manifest.pointer("/session_seam/owner") != Some(&Value::String("product".to_string())) {
        return Err("dogfood session seam must be product-owned".to_string());
    }
    if manifest.pointer("/session_seam/opens_clankers_db") != Some(&Value::Bool(false)) {
        return Err("dogfood session seam must not open Clankers DB".to_string());
    }
    if !string_set(manifest, "tool_catalog_refs")?.contains("lookup_project_fact") {
        return Err("dogfood manifest must reference lookup_project_fact tool catalog entry".to_string());
    }
    if !string_set(manifest, "capability_packs")?.contains("read_only") {
        return Err("dogfood manifest must select read_only capability pack".to_string());
    }
    Ok(())
}

fn dependency_boundary_report(manifest: &Value, allowed: &BTreeSet<String>) -> Result<Value, String> {
    let selected = string_set(manifest, "selected_green_crates")?.into_iter().collect::<Vec<_>>();
    Ok(json!({
        "schema": "clankers.embedded_product_dogfood.dependency_boundary_report.v1",
        "product": required_str(manifest, "product")?,
        "selected_green_crates": selected,
        "allowed_by_policy": allowed,
        "forbidden_surfaces_absent": FORBIDDEN_SURFACES,
        "shell_exclusions": manifest.get("shell_exclusions").cloned().unwrap_or(Value::Array(Vec::new())),
    }))
}

fn sanitized_transcript(manifest: &Value) -> Result<Value, String> {
    Ok(json!({
        "schema": "clankers.embedded_product_dogfood.sanitized_transcript.v1",
        "product": required_str(manifest, "product")?,
        "session_id": "product-workbench-session",
        "turns": [
            {
                "turn_index": 1,
                "model_requests": 2,
                "tool_calls": ["lookup_project_fact"],
                "tool_result_summary": "tool-result:launch code name: Orchard",
                "assistant_summary": "Stored the product fact: Orchard.",
                "credentials_redacted": true
            },
            {
                "turn_index": 2,
                "model_requests": 1,
                "restored_context_roles": ["user", "assistant", "tool", "assistant", "user"],
                "assistant_summary": "The tool reported Orchard.",
                "credentials_redacted": true
            }
        ],
        "negative_paths": ["missing-session-fails-closed", "dangerous-tool-denied-before-product-code"]
    }))
}

fn json_file(path: &str) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|error| format!("failed to read {path}: {error}"))?;
    serde_json::from_str(&text).map_err(|error| format!("failed to parse {path}: {error}"))
}

fn write_json(path: &str, value: &Value) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| format!("failed to encode {path}: {error}"))?;
    fs::write(path, [bytes.as_slice(), b"\n"].concat()).map_err(|error| format!("failed to write {path}: {error}"))
}

fn required_str<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value.get(field).and_then(Value::as_str).ok_or_else(|| format!("missing string field `{field}`"))
}

fn require_eq(value: &Value, field: &str, expected: &str) -> Result<(), String> {
    let actual = required_str(value, field)?;
    if actual != expected {
        return Err(format!("field `{field}` expected `{expected}`, got `{actual}`"));
    }
    Ok(())
}

fn string_set(value: &Value, field: &str) -> Result<BTreeSet<String>, String> {
    let array = value.get(field).and_then(Value::as_array).ok_or_else(|| format!("missing array field `{field}`"))?;
    let mut out = BTreeSet::new();
    for item in array {
        let Some(text) = item.as_str() else {
            return Err(format!("field `{field}` contains non-string item"));
        };
        out.insert(text.to_string());
    }
    Ok(out)
}

fn hash_artifact(path: &Path) -> Result<Value, String> {
    let mut file = fs::File::open(path).map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 16 * 1024];
    let mut bytes = 0u64;
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
