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
const FIXTURE: &str = "policy/embedded-lego/plugin-runtime-dispatch.json";
const POLICY: &str = "policy/embedded-lego/lego-contracts.json";
const DOCS: &str = "docs/src/tutorials/embedded-agent-sdk.md";
const SPEC: &str = "cairn/specs/embedded-composition-kits/spec.md";
const OUTPUT: &str = "target/embedded-sdk-release/plugin-runtime-dispatch-receipt.json";
const REQUIRED_KINDS: &[&str] = &["extism", "stdio", "built-in", "product-owned"];
const DANGEROUS_LOADERS: &[&str] = &["wasm", "stdio", "built-in", "product-owned"];

fn main() -> ExitCode {
    match run() {
        Ok(path) => {
            println!("plugin-runtime-dispatch receipt written to {}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("plugin-runtime-dispatch check failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<PathBuf, String> {
    let fixture_text = fs::read_to_string(FIXTURE).map_err(|error| format!("failed to read {FIXTURE}: {error}"))?;
    let fixture: Value = serde_json::from_str(&fixture_text).map_err(|error| format!("failed to parse {FIXTURE}: {error}"))?;
    let policy_text = fs::read_to_string(POLICY).map_err(|error| format!("failed to read {POLICY}: {error}"))?;
    let policy: Value = serde_json::from_str(&policy_text).map_err(|error| format!("failed to parse {POLICY}: {error}"))?;
    let docs = fs::read_to_string(DOCS).map_err(|error| format!("failed to read {DOCS}: {error}"))?;
    let spec = fs::read_to_string(SPEC).map_err(|error| format!("failed to read {SPEC}: {error}"))?;

    let mut errors = Vec::new();
    validate_fixture(&fixture, &mut errors);
    validate_policy_link(&policy, &mut errors);
    validate_docs_and_spec(&docs, &spec, &mut errors);
    validate_source_guards(&fixture, &mut errors);

    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }

    let normalized = normalized_matrix(&fixture)?;
    let output_path = PathBuf::from(OUTPUT);
    let parent = output_path.parent().ok_or_else(|| format!("{OUTPUT} has no parent"))?;
    fs::create_dir_all(parent).map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let receipt = json!({
        "schema": "clankers.embedded_lego.plugin_runtime_dispatch_receipt.v1",
        "fixture": FIXTURE,
        "normalized_matrix_blake3": blake3_hex(&normalized),
        "hashed_artifacts": [
            hash_artifact(Path::new(FIXTURE))?,
            hash_artifact(Path::new(POLICY))?,
            hash_artifact(Path::new(DOCS))?,
            hash_artifact(Path::new(SPEC))?,
            hash_artifact(Path::new("crates/clankers-plugin/src/lib.rs"))?,
            hash_artifact(Path::new("crates/clankers-plugin/src/manifest.rs"))?,
            hash_artifact(Path::new("crates/clankers-plugin/src/host_facade.rs"))?,
        ],
        "runtime_kind_allowlist": REQUIRED_KINDS,
        "guidance": "Runtime-kind dispatch remains app-edge/yellow. Extism, stdio, built-in, and product-owned entries are validated before dispatch and must not be routed through another runtime loader."
    });
    let bytes = serde_json::to_vec_pretty(&receipt).map_err(|error| format!("failed to encode receipt: {error}"))?;
    fs::write(&output_path, [bytes.as_slice(), b"\n"].concat())
        .map_err(|error| format!("failed to write {}: {error}", output_path.display()))?;
    Ok(output_path)
}

fn validate_fixture(fixture: &Value, errors: &mut Vec<String>) {
    if fixture.get("schema").and_then(Value::as_str) != Some("clankers.embedded_lego.runtime_dispatch.v1") {
        errors.push("fixture schema must be clankers.embedded_lego.runtime_dispatch.v1".to_string());
    }
    let allowlist = string_set(fixture, "runtime_kind_allowlist", errors);
    for kind in REQUIRED_KINDS {
        if !allowlist.contains(*kind) {
            errors.push(format!("runtime kind allowlist missing `{kind}`"));
        }
    }
    let entries = array(fixture, "dispatch_matrix", errors);
    let mut kinds = BTreeSet::new();
    for entry in entries {
        let kind = required_str(entry, "kind", errors);
        let loader = required_str(entry, "loader", errors);
        let owner = required_str(entry, "dispatch_owner", errors);
        kinds.insert(kind.to_string());
        if owner.is_empty() {
            errors.push(format!("runtime kind `{kind}` missing dispatch owner"));
        }
        if entry.get("policy_checked_before_dispatch").and_then(Value::as_bool) != Some(true) {
            errors.push(format!("runtime kind `{kind}` must be policy checked before dispatch"));
        }
        if kind == "extism" && loader != "wasm" {
            errors.push("extism must dispatch to wasm loader".to_string());
        } else if kind != "extism" && kind != loader {
            errors.push(format!("runtime kind `{kind}` must dispatch to matching loader, got `{loader}`"));
        }
        if !DANGEROUS_LOADERS.contains(&loader) {
            errors.push(format!("runtime kind `{kind}` uses unknown loader `{loader}`"));
        }
        let forbidden = string_set(entry, "forbidden_loaders", errors);
        if forbidden.contains(loader) {
            errors.push(format!("runtime kind `{kind}` forbids its selected loader `{loader}`"));
        }
        for required_field in ["manifest_requires", "forbidden_loaders"] {
            if array(entry, required_field, errors).is_empty() {
                errors.push(format!("runtime kind `{kind}` must declare `{required_field}`"));
            }
        }
    }
    for kind in REQUIRED_KINDS {
        if !kinds.contains(*kind) {
            errors.push(format!("dispatch matrix missing `{kind}`"));
        }
    }
    let denial_names = array(fixture, "denial_fixtures", errors)
        .iter()
        .map(|entry| required_str(entry, "name", errors))
        .collect::<BTreeSet<_>>();
    for required in [
        "stdio_without_launch_policy",
        "stdio_without_sandbox",
        "non_stdio_with_stdio_policy",
        "stdio_sent_to_wasm_loader",
    ] {
        if !denial_names.contains(required) {
            errors.push(format!("denial fixtures missing `{required}`"));
        }
    }
}

fn validate_policy_link(policy: &Value, errors: &mut Vec<String>) {
    let fixture = required_str(policy, "plugin_runtime_dispatch_fixture", errors);
    if fixture != FIXTURE {
        errors.push(format!("plugin_runtime_dispatch_fixture must point at `{FIXTURE}`"));
    }
    let policy_matrix = array(policy, "runtime_dispatch_matrix", errors);
    let policy_kinds = policy_matrix.iter().map(|entry| required_str(entry, "kind", errors)).collect::<BTreeSet<_>>();
    for kind in REQUIRED_KINDS {
        if !policy_kinds.contains(kind) {
            errors.push(format!("policy runtime dispatch matrix missing `{kind}`"));
        }
    }
}

fn validate_docs_and_spec(docs: &str, spec: &str, errors: &mut Vec<String>) {
    for marker in [
        "plugin-runtime-dispatch receipt",
        "Extism, stdio, built-in, and product-owned",
        "non-Extism entries never flow through eager WASM loading",
    ] {
        if !docs.contains(marker) {
            errors.push(format!("{DOCS} missing `{marker}`"));
        }
    }
    for marker in [
        "Runtime kind dispatch is explicit",
        "Launch policy is contract checked",
        "Dispatch matrix evidence is content addressed",
    ] {
        if !spec.contains(marker) {
            errors.push(format!("{SPEC} missing `{marker}`"));
        }
    }
}

fn validate_source_guards(fixture: &Value, errors: &mut Vec<String>) {
    for guard in array(fixture, "source_guards", errors) {
        let path = required_str(guard, "path", errors);
        let needle = required_str(guard, "must_contain", errors);
        if path.is_empty() || needle.is_empty() {
            continue;
        }
        match fs::read_to_string(path) {
            Ok(text) if text.contains(needle) => {}
            Ok(_) => errors.push(format!("{path} missing source guard `{needle}`")),
            Err(error) => errors.push(format!("failed to read source guard {path}: {error}")),
        }
    }
}

fn normalized_matrix(fixture: &Value) -> Result<Vec<u8>, String> {
    let mut entries = array(fixture, "dispatch_matrix", &mut Vec::new())
        .iter()
        .map(|entry| {
            json!({
                "kind": entry.get("kind"),
                "loader": entry.get("loader"),
                "dispatch_owner": entry.get("dispatch_owner"),
                "manifest_requires": sorted_strings(entry.get("manifest_requires")),
                "forbidden_loaders": sorted_strings(entry.get("forbidden_loaders")),
            })
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.get("kind").and_then(Value::as_str).unwrap_or_default().to_string());
    serde_json::to_vec(&entries).map_err(|error| format!("failed to normalize matrix: {error}"))
}

fn sorted_strings(value: Option<&Value>) -> Vec<String> {
    let mut items = value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    items.sort();
    items
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
    Ok(json!({"path": path.to_string_lossy(), "bytes": bytes, "blake3": hasher.finalize().to_hex().to_string()}))
}

fn blake3_hex(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}
