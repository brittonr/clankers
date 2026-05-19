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

use serde_json::{json, Value};

const ERROR_EXIT: u8 = 1;
const FIXTURE: &str = "policy/embedded-lego/capability-pack-composition.json";
const POLICY: &str = "policy/embedded-lego/lego-contracts.json";
const DOCS: &str = "docs/src/tutorials/embedded-agent-sdk.md";
const SPEC: &str = "openspec/specs/embedded-composition-kits/spec.md";
const DEFAULT_OUTPUT: &str = "target/embedded-sdk-release/capability-pack-composition-receipt.json";
const DANGEROUS: &[&str] = &["mutate_project", "shell", "network", "raw-log", "secret-adjacent"];
const SAFE_PACKS: &[&str] = &["embedding_safe", "read_only", "networkless_coding"];

fn main() -> ExitCode {
    match run() {
        Ok(path) => { println!("capability-pack-composition receipt written to {}", path.display()); ExitCode::SUCCESS }
        Err(error) => { eprintln!("capability-pack-composition check failed: {error}"); ExitCode::from(ERROR_EXIT) }
    }
}

fn run() -> Result<PathBuf, String> {
    let text = fs::read_to_string(FIXTURE).map_err(|error| format!("failed to read {FIXTURE}: {error}"))?;
    let fixture: Value = serde_json::from_str(&text).map_err(|error| format!("failed to parse {FIXTURE}: {error}"))?;
    require_eq(&fixture, "schema", "clankers.embedded_lego.capability_pack_composition.v1")?;
    validate_policy_points_at_fixture()?;
    validate_docs_and_spec()?;
    let capability_order = strings(&fixture, "capability_order")?;
    let packs = fixture.get("packs").and_then(Value::as_array).ok_or_else(|| "fixture missing packs".to_string())?;
    let pack_map = validate_packs(packs, &capability_order)?;
    let snapshots = validate_compositions(&fixture, &pack_map, &capability_order)?;
    validate_denials(&fixture, &pack_map, &capability_order)?;
    let snapshot_bytes = serde_json::to_vec_pretty(&snapshots).map_err(|error| format!("failed to encode snapshots: {error}"))?;
    let receipt = json!({
        "schema": "clankers.embedded_lego.capability_pack_composition.receipt.v1",
        "fixture": FIXTURE,
        "snapshot_blake3": blake3::hash(&snapshot_bytes).to_hex().to_string(),
        "snapshots": snapshots,
        "hashed_artifacts": [
            hash_artifact(Path::new(FIXTURE))?,
            hash_artifact(Path::new(POLICY))?,
            hash_artifact(Path::new(DOCS))?,
            hash_artifact(Path::new(SPEC))?,
        ],
        "boundary": "Capability pack composition is checked from exported fixture data; generic SDK crates do not evaluate Nickel or load product policy files at runtime."
    });
    let output = PathBuf::from(DEFAULT_OUTPUT);
    let parent = output.parent().ok_or_else(|| format!("{} has no parent", output.display()))?;
    fs::create_dir_all(parent).map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let bytes = serde_json::to_vec_pretty(&receipt).map_err(|error| format!("failed to encode receipt: {error}"))?;
    fs::write(&output, [bytes.as_slice(), b"\n"].concat()).map_err(|error| format!("failed to write {}: {error}", output.display()))?;
    Ok(output)
}

fn validate_policy_points_at_fixture() -> Result<(), String> {
    let policy_text = fs::read_to_string(POLICY).map_err(|error| format!("failed to read {POLICY}: {error}"))?;
    let policy: Value = serde_json::from_str(&policy_text).map_err(|error| format!("failed to parse {POLICY}: {error}"))?;
    if policy.pointer("/capability_pack_composition_fixture") != Some(&Value::String(FIXTURE.to_string())) {
        return Err("policy must point capability_pack_composition_fixture at the checked fixture".to_string());
    }
    Ok(())
}

fn validate_packs<'a>(packs: &'a [Value], order: &[String]) -> Result<BTreeMap<String, &'a Value>, String> {
    let mut map = BTreeMap::new();
    for pack in packs {
        let name = required_str(pack, "name")?;
        if map.insert(name.to_string(), pack).is_some() { return Err(format!("duplicate capability pack `{name}`")); }
        let capabilities = strings(pack, "capabilities")?;
        for capability in &capabilities {
            if !order.contains(capability) { return Err(format!("unknown capability atom `{capability}`")); }
        }
        if SAFE_PACKS.contains(&name) && capabilities.iter().any(|cap| DANGEROUS.contains(&cap.as_str())) {
            return Err(format!("safe pack `{name}` contains dangerous capability"));
        }
        let dangerous = pack.get("dangerous").and_then(Value::as_bool).unwrap_or(false);
        let approval = pack.get("requires_human_approval").and_then(Value::as_bool).unwrap_or(false);
        if capabilities.iter().any(|cap| DANGEROUS.contains(&cap.as_str())) && (!dangerous || !approval) {
            return Err(format!("dangerous pack `{name}` must be marked dangerous and require approval"));
        }
        pack.get("merge_priority").and_then(Value::as_i64).ok_or_else(|| format!("pack `{name}` missing merge_priority"))?;
    }
    Ok(map)
}

fn validate_compositions(fixture: &Value, packs: &BTreeMap<String, &Value>, order: &[String]) -> Result<Value, String> {
    let compositions = fixture.get("compositions").and_then(Value::as_array).ok_or_else(|| "fixture missing compositions".to_string())?;
    let mut snapshots = Vec::new();
    for composition in compositions {
        let name = required_str(composition, "name")?;
        let actual = merge(composition, packs, order)?;
        let expected = strings(composition, "expected_capabilities")?;
        if actual != expected { return Err(format!("composition `{name}` expected {expected:?}, got {actual:?}")); }
        snapshots.push(json!({"name": name, "capabilities": actual, "approval_policy": required_str(composition, "approval_policy")?}));
    }
    Ok(Value::Array(snapshots))
}

fn validate_denials(fixture: &Value, base_packs: &BTreeMap<String, &Value>, order: &[String]) -> Result<(), String> {
    let denials = fixture.get("denial_fixtures").and_then(Value::as_array).ok_or_else(|| "fixture missing denial_fixtures".to_string())?;
    for denial in denials {
        let name = required_str(denial, "name")?;
        let expected = required_str(denial, "expect_error")?;
        let mut packs = base_packs.clone();
        let inline = denial.get("inline_pack");
        if let Some(pack) = inline {
            let pack_name = required_str(pack, "name")?;
            if packs.insert(pack_name.to_string(), pack).is_some() {
                let error = format!("duplicate capability pack `{pack_name}`");
                if !error.contains(expected) { return Err(format!("denial `{name}` expected `{expected}`, got `{error}`")); }
                continue;
            }
        }
        let error = merge(denial, &packs, order).expect_err("denial fixture should fail");
        if !error.contains(expected) { return Err(format!("denial `{name}` expected `{expected}`, got `{error}`")); }
    }
    Ok(())
}

fn merge(composition: &Value, packs: &BTreeMap<String, &Value>, order: &[String]) -> Result<Vec<String>, String> {
    let approval_policy = required_str(composition, "approval_policy")?;
    let mut selected = BTreeSet::new();
    for pack_name in strings(composition, "packs")? {
        let pack = packs.get(&pack_name).ok_or_else(|| format!("unknown capability pack `{pack_name}`"))?;
        let pack_caps = strings(pack, "capabilities")?;
        let dangerous = pack_caps.iter().any(|cap| DANGEROUS.contains(&cap.as_str())) || pack.get("dangerous").and_then(Value::as_bool).unwrap_or(false);
        let approved = pack.get("requires_human_approval").and_then(Value::as_bool).unwrap_or(false);
        if dangerous && (approval_policy != "allow-dangerous-with-human-approval" || !approved) {
            return Err(format!("dangerous pack requires product approval: `{pack_name}`"));
        }
        for cap in pack_caps {
            if !order.contains(&cap) { return Err(format!("unknown capability atom `{cap}`")); }
            selected.insert(cap);
        }
    }
    Ok(order.iter().filter(|cap| selected.contains(*cap)).cloned().collect())
}

fn validate_docs_and_spec() -> Result<(), String> {
    let docs = fs::read_to_string(DOCS).map_err(|error| format!("failed to read {DOCS}: {error}"))?;
    for marker in ["capability-pack-composition", "capability-pack-composition.json", "scripts/check-capability-pack-composition.rs", "safe-only"] {
        require_contains(&docs, marker, &format!("{DOCS} missing `{marker}`"))?;
    }
    let spec = fs::read_to_string(SPEC).map_err(|error| format!("failed to read {SPEC}: {error}"))?;
    for marker in ["Pack merge order is deterministic", "Dangerous conflicts fail closed", "Pack policy is checked before Rust use"] {
        require_contains(&spec, marker, &format!("{SPEC} missing `{marker}`"))?;
    }
    Ok(())
}

fn strings(value: &Value, field: &str) -> Result<Vec<String>, String> {
    let array = value.get(field).and_then(Value::as_array).ok_or_else(|| format!("missing array field `{field}`"))?;
    let mut result = Vec::new();
    for item in array {
        result.push(item.as_str().ok_or_else(|| format!("field `{field}` contains non-string"))?.to_string());
    }
    Ok(result)
}

fn require_eq(value: &Value, field: &str, expected: &str) -> Result<(), String> {
    let actual = required_str(value, field)?;
    if actual == expected { Ok(()) } else { Err(format!("field `{field}` expected `{expected}`, got `{actual}`")) }
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
