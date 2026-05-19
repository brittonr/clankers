#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"

[dependencies]
blake3 = "1"
serde_json = "1"
---

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use serde_json::json;
use serde_json::Value;

const POLICY_PATH: &str = "policy/embedded-lego/brick-inventory-stability.json";
const RECEIPT_PATH: &str = "target/embedded-sdk-release/brick-inventory-stability-receipt.json";
const RELEASE_RECEIPT_SCRIPT: &str = "scripts/emit-embedded-sdk-release-receipt.rs";
const ERROR_EXIT: u8 = 1;

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct InventoryRow {
    entry: String,
    crate_name: String,
    kind: String,
    stability: String,
    source: String,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("brick-inventory-stability receipt written to {RECEIPT_PATH}");
            ExitCode::SUCCESS
        }
        Err(errors) => {
            for error in errors {
                eprintln!("brick-inventory-stability check failed: {error}");
            }
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), Vec<String>> {
    let policy_text = read(POLICY_PATH)?;
    let policy: Value = serde_json::from_str(&policy_text)
        .map_err(|error| vec![format!("failed to parse {POLICY_PATH}: {error}")])?;
    let inventory_path = required_str(&policy, "inventory_path")?;
    let guide_path = required_str(&policy, "guide_path")?;
    let inventory_text = read(inventory_path)?;
    let guide_text = read(guide_path)?;
    let release_script = read(RELEASE_RECEIPT_SCRIPT)?;
    let rows = parse_inventory(&inventory_text)?;

    let mut errors = Vec::new();
    validate_counts(&policy, &rows, &mut errors);
    validate_stabilities(&policy, &rows, &mut errors);
    validate_sources(&rows, &mut errors);
    validate_stable_hash(&policy, &rows, &mut errors);
    validate_migration_notes(&policy, &guide_text, &mut errors);
    validate_release_receipt_inputs(&policy, &release_script, &mut errors);

    if errors.is_empty() {
        write_receipt(&policy, &rows).map_err(|error| vec![error])?;
        Ok(())
    } else {
        Err(errors)
    }
}

fn read(path: &str) -> Result<String, Vec<String>> {
    fs::read_to_string(path).map_err(|error| vec![format!("failed to read {path}: {error}")])
}

fn required_str<'a>(value: &'a Value, key: &str) -> Result<&'a str, Vec<String>> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| vec![format!("{POLICY_PATH} missing non-empty string field `{key}`")])
}

fn required_array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>, Vec<String>> {
    value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| vec![format!("{POLICY_PATH} missing array field `{key}`")])
}

fn parse_inventory(text: &str) -> Result<Vec<InventoryRow>, Vec<String>> {
    let mut rows = Vec::new();
    let mut errors = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if !trimmed.starts_with("| `") {
            continue;
        }
        let cells = trimmed
            .trim_matches('|')
            .split('|')
            .map(|cell| cell.trim().to_string())
            .collect::<Vec<_>>();
        if cells.len() != 5 {
            errors.push(format!("inventory row {} has {} cells, expected 5", index + 1, cells.len()));
            continue;
        }
        rows.push(InventoryRow {
            entry: strip_code(&cells[0]),
            crate_name: strip_code(&cells[1]),
            kind: cells[2].to_string(),
            stability: cells[3].to_string(),
            source: strip_code(&cells[4]),
        });
    }
    if rows.is_empty() {
        errors.push("inventory has no checked rows".to_string());
    }
    if errors.is_empty() { Ok(rows) } else { Err(errors) }
}

fn strip_code(text: &str) -> String {
    text.trim().trim_matches('`').to_string()
}

fn validate_counts(policy: &Value, rows: &[InventoryRow], errors: &mut Vec<String>) {
    let Some(expected) = policy.get("expected_counts").and_then(Value::as_object) else {
        errors.push(format!("{POLICY_PATH} missing expected_counts object"));
        return;
    };
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for row in rows {
        *counts.entry(row.stability.as_str()).or_default() += 1;
    }
    check_count(expected, "total", rows.len(), errors);
    for name in ["supported", "optional-support", "compatibility-alias", "experimental", "unsupported-internal"] {
        check_count(expected, name, counts.get(name).copied().unwrap_or_default(), errors);
    }
    let stable = stable_rows(policy, rows).len();
    check_count(expected, "stable-contract", stable, errors);
}

fn check_count(expected: &serde_json::Map<String, Value>, name: &str, actual: usize, errors: &mut Vec<String>) {
    let Some(want) = expected.get(name).and_then(Value::as_u64) else {
        errors.push(format!("{POLICY_PATH} expected_counts missing `{name}`"));
        return;
    };
    if want != actual as u64 {
        errors.push(format!("inventory count drift for `{name}`: expected {want}, got {actual}"));
    }
}

fn validate_stabilities(policy: &Value, rows: &[InventoryRow], errors: &mut Vec<String>) {
    let allowed = string_set(policy, "allowed_stabilities", errors);
    let stable = string_set(policy, "stable_contract_stabilities", errors);
    if !stable.contains("supported") || !stable.contains("compatibility-alias") {
        errors.push("stable_contract_stabilities must include supported and compatibility-alias".to_string());
    }
    for row in rows {
        if !allowed.contains(row.stability.as_str()) {
            errors.push(format!("unsupported stability label `{}` for `{}`", row.stability, row.entry));
        }
    }
}

fn string_set<'a>(policy: &'a Value, key: &str, errors: &mut Vec<String>) -> BTreeSet<&'a str> {
    match required_array(policy, key) {
        Ok(values) => values.iter().filter_map(Value::as_str).collect(),
        Err(mut new_errors) => {
            errors.append(&mut new_errors);
            BTreeSet::new()
        }
    }
}

fn validate_sources(rows: &[InventoryRow], errors: &mut Vec<String>) {
    for row in rows {
        if !Path::new(&row.source).exists() {
            errors.push(format!("inventory source for `{}` does not exist: {}", row.entry, row.source));
        }
    }
}

fn stable_rows<'a>(policy: &Value, rows: &'a [InventoryRow]) -> Vec<&'a InventoryRow> {
    let stabilities = policy
        .get("stable_contract_stabilities")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    let mut stable = rows
        .iter()
        .filter(|row| stabilities.contains(row.stability.as_str()))
        .collect::<Vec<_>>();
    stable.sort();
    stable
}

fn normalized_stable_inventory(policy: &Value, rows: &[InventoryRow]) -> String {
    stable_rows(policy, rows)
        .into_iter()
        .map(|row| format!("{}|{}|{}|{}|{}", row.entry, row.crate_name, row.kind, row.stability, row.source))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn validate_stable_hash(policy: &Value, rows: &[InventoryRow], errors: &mut Vec<String>) {
    let expected = match required_str(policy, "stable_contract_blake3") {
        Ok(value) => value,
        Err(mut new_errors) => {
            errors.append(&mut new_errors);
            return;
        }
    };
    let normalized = normalized_stable_inventory(policy, rows);
    let actual = blake3::hash(normalized.as_bytes()).to_hex().to_string();
    if actual != expected {
        errors.push(format!(
            "stable brick inventory drift: expected blake3 {expected}, got {actual}; update migration notes, examples, receipt inputs, and {POLICY_PATH} together"
        ));
    }
}

fn validate_migration_notes(policy: &Value, guide: &str, errors: &mut Vec<String>) {
    let anchor = match required_str(policy, "migration_note_anchor") {
        Ok(value) => value,
        Err(mut new_errors) => {
            errors.append(&mut new_errors);
            return;
        }
    };
    if !guide.contains(anchor) {
        errors.push(format!("guide missing migration note anchor `{anchor}`"));
    }
    for phrase in ["affected entrypoint", "replacement or adapter change", "validation command"] {
        if !guide.contains(phrase) {
            errors.push(format!("guide migration policy must mention `{phrase}`"));
        }
    }
}

fn validate_release_receipt_inputs(policy: &Value, release_script: &str, errors: &mut Vec<String>) {
    let required = match required_array(policy, "required_release_receipt_artifacts") {
        Ok(value) => value,
        Err(mut new_errors) => {
            errors.append(&mut new_errors);
            return;
        }
    };
    for path in required.iter().filter_map(Value::as_str) {
        if !release_script.contains(path) {
            errors.push(format!("{RELEASE_RECEIPT_SCRIPT} does not hash required brick artifact `{path}`"));
        }
    }
}

fn write_receipt(policy: &Value, rows: &[InventoryRow]) -> Result<(), String> {
    let normalized = normalized_stable_inventory(policy, rows);
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for row in rows {
        *counts.entry(row.stability.as_str()).or_default() += 1;
    }
    let receipt = json!({
        "schema": "clankers.embedded_lego.brick_inventory_stability_receipt.v1",
        "inventory_path": required_str(policy, "inventory_path").map_err(|errors| errors.join("; "))?,
        "guide_path": required_str(policy, "guide_path").map_err(|errors| errors.join("; "))?,
        "total_rows": rows.len(),
        "stability_counts": counts,
        "stable_contract_rows": stable_rows(policy, rows).len(),
        "stable_contract_blake3": blake3::hash(normalized.as_bytes()).to_hex().to_string(),
    });
    let output = Path::new(RECEIPT_PATH);
    let parent = output.parent().ok_or_else(|| format!("{RECEIPT_PATH} has no parent"))?;
    fs::create_dir_all(parent).map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let bytes = serde_json::to_vec_pretty(&receipt).map_err(|error| format!("failed to encode receipt: {error}"))?;
    fs::write(output, [bytes.as_slice(), b"\n"].concat()).map_err(|error| format!("failed to write {RECEIPT_PATH}: {error}"))
}
