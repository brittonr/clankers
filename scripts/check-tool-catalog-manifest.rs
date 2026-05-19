#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"

[dependencies]
blake3 = "1"
serde_json = "1"
---

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::{json, Map, Value};

const ERROR_EXIT: u8 = 1;
const MANIFEST: &str = "examples/embedded-tool-kit/tool-catalog-manifest.json";
const POLICY: &str = "policy/embedded-lego/lego-contracts.json";
const DOCS: &str = "docs/src/tutorials/embedded-agent-sdk.md";
const SPEC: &str = "openspec/specs/embedded-composition-kits/spec.md";
const DEFAULT_OUTPUT: &str = "target/embedded-sdk-release/tool-catalog-manifest-receipt.json";
const SAFE_CAPABILITIES: &[&str] = &["observe", "read_project"];
const DANGEROUS_CAPABILITIES: &[&str] = &["mutate", "shell", "network", "raw-log", "secret-adjacent"];
const RUNTIME_NEUTRAL_MARKERS: &[&str] = &[
    "does-not-start-stdio",
    "does-not-load-extism",
    "does-not-call-network",
    "does-not-read-secrets",
    "does-not-execute-product-tools",
];

fn main() -> ExitCode {
    match run() {
        Ok(path) => {
            println!("tool-catalog-manifest receipt written to {}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("tool-catalog-manifest check failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<PathBuf, String> {
    let text = fs::read_to_string(MANIFEST).map_err(|error| format!("failed to read {MANIFEST}: {error}"))?;
    let manifest: Value = serde_json::from_str(&text).map_err(|error| format!("failed to parse {MANIFEST}: {error}"))?;
    require_eq(&manifest, "schema", "clankers.embedded_tool_catalog.manifest.v1")?;
    validate_policy_points_at_manifest()?;
    validate_runtime_neutrality(&manifest)?;
    let tools = manifest.get("tools").and_then(Value::as_array).ok_or_else(|| "manifest missing tools".to_string())?;
    let normalized = normalize_tools(tools)?;
    validate_denial_fixtures(&manifest)?;
    validate_truncation_fixture(&manifest)?;
    validate_docs_and_spec()?;
    let normalized_bytes = serde_json::to_vec_pretty(&normalized).map_err(|error| format!("failed to encode normalized metadata: {error}"))?;
    let normalized_hash = blake3::hash(&normalized_bytes).to_hex().to_string();
    let receipt = json!({
        "schema": "clankers.embedded_tool_catalog_manifest.receipt.v1",
        "manifest": MANIFEST,
        "normalized_metadata_blake3": normalized_hash,
        "normalized_metadata": normalized,
        "hashed_artifacts": [
            hash_artifact(Path::new(MANIFEST))?,
            hash_artifact(Path::new(POLICY))?,
            hash_artifact(Path::new(DOCS))?,
            hash_artifact(Path::new(SPEC))?,
        ],
        "runtime_neutrality": RUNTIME_NEUTRAL_MARKERS,
        "boundary": "Manifest parsing/export validates data only; it does not start stdio, load Extism, call network, read secrets, or execute product tools."
    });
    let output = PathBuf::from(DEFAULT_OUTPUT);
    let parent = output.parent().ok_or_else(|| format!("{} has no parent", output.display()))?;
    fs::create_dir_all(parent).map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let bytes = serde_json::to_vec_pretty(&receipt).map_err(|error| format!("failed to encode receipt: {error}"))?;
    fs::write(&output, [bytes.as_slice(), b"\n"].concat()).map_err(|error| format!("failed to write {}: {error}", output.display()))?;
    Ok(output)
}

fn validate_policy_points_at_manifest() -> Result<(), String> {
    let policy_text = fs::read_to_string(POLICY).map_err(|error| format!("failed to read {POLICY}: {error}"))?;
    let policy: Value = serde_json::from_str(&policy_text).map_err(|error| format!("failed to parse {POLICY}: {error}"))?;
    if policy.pointer("/tool_catalog_manifest") != Some(&Value::String(MANIFEST.to_string())) {
        return Err("policy must point tool_catalog_manifest at the checked manifest".to_string());
    }
    Ok(())
}

fn validate_runtime_neutrality(manifest: &Value) -> Result<(), String> {
    let markers = string_set(manifest, "runtime_neutrality")?;
    for marker in RUNTIME_NEUTRAL_MARKERS {
        if !markers.contains(*marker) {
            return Err(format!("manifest missing runtime-neutrality marker `{marker}`"));
        }
    }
    Ok(())
}

fn normalize_tools(tools: &[Value]) -> Result<Value, String> {
    let mut names = BTreeSet::new();
    let mut normalized = Vec::new();
    for tool in tools {
        validate_tool(tool)?;
        let name = required_str(tool, "name")?;
        if !names.insert(name.to_string()) {
            return Err(format!("duplicate tool name `{name}`"));
        }
        normalized.push(json!({
            "name": name,
            "description": required_str(tool, "description")?,
            "runtime": required_str(tool, "runtime")?,
            "capabilities": sorted_strings(tool, "capabilities")?,
            "approval": required_str(tool, "approval")?,
            "redaction": required_str(tool, "redaction")?,
            "input_schema": canonicalize(tool.get("input_schema").unwrap_or(&Value::Object(Map::new()))),
        }));
    }
    normalized.sort_by(|left, right| left.get("name").and_then(Value::as_str).cmp(&right.get("name").and_then(Value::as_str)));
    Ok(Value::Array(normalized))
}

fn validate_tool(tool: &Value) -> Result<(), String> {
    let name = required_str(tool, "name")?;
    if required_str(tool, "description")?.trim().is_empty() {
        return Err(format!("tool `{name}` missing description"));
    }
    match required_str(tool, "runtime")? {
        "product-owned" | "stdio" | "extism" | "built-in" => {}
        other => return Err(format!("unknown runtime kind `{other}`")),
    }
    let schema = tool.get("input_schema").ok_or_else(|| format!("tool `{name}` missing input_schema"))?;
    if !schema.is_object() {
        return Err(format!("tool `{name}` invalid input schema"));
    }
    if let Some(properties) = schema.get("properties") {
        if !properties.is_object() {
            return Err(format!("tool `{name}` invalid input schema"));
        }
    }
    let capabilities = string_set(tool, "capabilities")?;
    let approval = required_str(tool, "approval")?;
    let redaction = required_str(tool, "redaction")?;
    let declared_dangerous = string_set_optional(tool, "declared_dangerous_capabilities")?;
    for capability in &capabilities {
        if !SAFE_CAPABILITIES.contains(&capability.as_str()) && !DANGEROUS_CAPABILITIES.contains(&capability.as_str()) {
            return Err(format!("unknown capability `{capability}`"));
        }
        if DANGEROUS_CAPABILITIES.contains(&capability.as_str()) {
            if approval != "always" {
                return Err(format!("dangerous capability requires approval for `{name}`"));
            }
            if let Some(declared) = &declared_dangerous {
                if !declared.contains(capability) {
                    return Err(format!("dangerous capability must be declared for `{name}`"));
                }
            }
        }
    }
    if capabilities.contains("secret-adjacent") && redaction == "none" {
        return Err(format!("secret-adjacent capability requires redaction for `{name}`"));
    }
    Ok(())
}

fn validate_denial_fixtures(manifest: &Value) -> Result<(), String> {
    let fixtures = manifest.get("denial_fixtures").and_then(Value::as_array).ok_or_else(|| "manifest missing denial_fixtures".to_string())?;
    let mut names = BTreeSet::new();
    for fixture in fixtures {
        let name = required_str(fixture, "name")?;
        names.insert(name.to_string());
        let expected = required_str(fixture, "expect_error")?;
        let tools = fixture.get("tools").and_then(Value::as_array).ok_or_else(|| format!("denial fixture `{name}` missing tools"))?;
        let error = normalize_tools(tools).expect_err("denial fixture should fail validation");
        if !error.contains(expected) {
            return Err(format!("denial fixture `{name}` expected `{expected}`, got `{error}`"));
        }
    }
    for required in ["duplicate-name", "invalid-schema", "unknown-runtime-kind", "unsafe-capability-default", "missing-redaction", "undeclared-dangerous-capability"] {
        if !names.contains(required) {
            return Err(format!("missing denial fixture `{required}`"));
        }
    }
    Ok(())
}

fn validate_truncation_fixture(manifest: &Value) -> Result<(), String> {
    let fixture = manifest.get("truncation_fixture").ok_or_else(|| "manifest missing truncation_fixture".to_string())?;
    if required_str(fixture, "expected_policy")? != "truncate-before-model-feedback" {
        return Err("truncation fixture must pin truncate-before-model-feedback policy".to_string());
    }
    if fixture.get("max_bytes").and_then(Value::as_u64).unwrap_or(0) == 0 {
        return Err("truncation fixture must bound max_bytes".to_string());
    }
    Ok(())
}

fn validate_docs_and_spec() -> Result<(), String> {
    let docs = fs::read_to_string(DOCS).map_err(|error| format!("failed to read {DOCS}: {error}"))?;
    for marker in ["tool-catalog-manifest", "tool-catalog-manifest.json", "scripts/check-tool-catalog-manifest.rs", "runtime-neutral"] {
        require_contains(&docs, marker, &format!("{DOCS} missing `{marker}`"))?;
    }
    let spec = fs::read_to_string(SPEC).map_err(|error| format!("failed to read {SPEC}: {error}"))?;
    for marker in ["Manifest export is normalized and runtime-neutral", "Manifest validation diagnostics are actionable", "Normalized evidence distinguishes semantic drift"] {
        require_contains(&spec, marker, &format!("{SPEC} missing `{marker}`"))?;
    }
    Ok(())
}

fn canonicalize(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut ordered = BTreeMap::new();
            for (key, value) in map {
                ordered.insert(key.clone(), canonicalize(value));
            }
            Value::Object(ordered.into_iter().collect())
        }
        Value::Array(items) => Value::Array(items.iter().map(canonicalize).collect()),
        other => other.clone(),
    }
}

fn string_set(value: &Value, field: &str) -> Result<BTreeSet<String>, String> {
    let array = value.get(field).and_then(Value::as_array).ok_or_else(|| format!("missing array field `{field}`"))?;
    array.iter().map(|item| item.as_str().map(str::to_string).ok_or_else(|| format!("field `{field}` contains non-string"))).collect()
}

fn string_set_optional(value: &Value, field: &str) -> Result<Option<BTreeSet<String>>, String> {
    match value.get(field) {
        Some(_) => Ok(Some(string_set(value, field)?)),
        None => Ok(None),
    }
}

fn sorted_strings(value: &Value, field: &str) -> Result<Vec<String>, String> {
    Ok(string_set(value, field)?.into_iter().collect())
}

fn require_eq(value: &Value, field: &str, expected: &str) -> Result<(), String> {
    match value.get(field).and_then(Value::as_str) {
        Some(actual) if actual == expected => Ok(()),
        Some(actual) => Err(format!("field `{field}` expected `{expected}`, got `{actual}`")),
        None => Err(format!("missing string field `{field}`")),
    }
}

fn required_str<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value.get(field).and_then(Value::as_str).filter(|text| !text.is_empty()).ok_or_else(|| format!("missing non-empty string field `{field}`"))
}

fn require_contains(haystack: &str, needle: &str, message: &str) -> Result<(), String> {
    if haystack.contains(needle) { Ok(()) } else { Err(message.to_string()) }
}

fn hash_artifact(path: &Path) -> Result<Value, String> {
    let mut file = fs::File::open(path).map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    let mut hasher = blake3::Hasher::new();
    let mut bytes = 0u64;
    let mut buffer = [0u8; 16 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        if read == 0 { break; }
        bytes += read as u64;
        hasher.update(&buffer[..read]);
    }
    Ok(json!({"path": path.to_string_lossy(), "bytes": bytes, "blake3": hasher.finalize().to_hex().to_string()}))
}
