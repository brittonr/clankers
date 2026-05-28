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

use serde_json::Value;
use serde_json::json;

const ERROR_EXIT: u8 = 1;
const ACCEPTANCE_SCRIPT: &str = "scripts/check-embedded-agent-sdk.rs";
const POLICY_PATH: &str = "policy/embedded-lego/behavioral-rail-inventory.json";
const RECEIPT_PATH: &str = "target/embedded-sdk-release/behavioral-rail-inventory-receipt.json";
const RUST_CHECKS_MARKER: &str = "const RUST_CHECKS";

const ALLOWED_CLASSES: &[&str] = &[
    "executable_fixture",
    "receipt_verifier",
    "ast_cargo_rail",
    "temporary_string_presence_check",
];

const REQUIRED_RECEIPT_FIELDS: &[&str] = &[
    "case_id",
    "axes",
    "expected_outcome",
    "observed_outcome",
    "source_artifacts",
    "sanitized_hashes",
    "owner",
    "requirement_ids",
];

const REQUIRED_NEGATIVE_CASES: &[&str] = &[
    "provider-auth-disabled-defaults",
    "missing-session-stores",
    "denied-capabilities",
    "event-redaction",
    "forbidden-transport-leakage",
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("behavioral lego rail inventory receipt written to {RECEIPT_PATH}");
            ExitCode::SUCCESS
        }
        Err(errors) => {
            for error in errors {
                eprintln!("behavioral-lego-rails check failed: {error}");
            }
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), Vec<String>> {
    let acceptance = read(ACCEPTANCE_SCRIPT)?;
    let policy_text = read(POLICY_PATH)?;
    let policy: Value =
        serde_json::from_str(&policy_text).map_err(|error| vec![format!("failed to parse {POLICY_PATH}: {error}")])?;
    let expected_scripts = extract_acceptance_scripts(&acceptance)?;
    let rails = required_array(&policy, "rails")?;
    let negative_fixtures = required_array(&policy, "negative_fixtures")?;

    let mut errors = Vec::new();
    validate_receipt_schema(&policy, &mut errors);
    validate_rails(&expected_scripts, rails, &mut errors);
    validate_negative_fixtures(negative_fixtures, &mut errors);
    validate_fixture_drift_self_tests(&mut errors);
    if !errors.is_empty() {
        return Err(errors);
    }

    write_receipt(&expected_scripts, rails, &policy_text).map_err(|error| vec![error])?;
    Ok(())
}

fn read(path: &str) -> Result<String, Vec<String>> {
    fs::read_to_string(path).map_err(|error| vec![format!("failed to read {path}: {error}")])
}

fn extract_acceptance_scripts(source: &str) -> Result<BTreeSet<String>, Vec<String>> {
    let Some(start) = source.find(RUST_CHECKS_MARKER) else {
        return Err(vec![format!("{ACCEPTANCE_SCRIPT} missing {RUST_CHECKS_MARKER}")]);
    };
    let tail = &source[start..];
    let Some(end) = tail.find("];") else {
        return Err(vec![format!("{ACCEPTANCE_SCRIPT} has unterminated RUST_CHECKS array")]);
    };
    let block = &tail[..end];
    let mut scripts = BTreeSet::new();
    for line in block.lines() {
        let trimmed = line.trim().trim_end_matches(',').trim();
        if !trimmed.starts_with('"') || !trimmed.ends_with('"') {
            continue;
        }
        scripts.insert(trimmed.trim_matches('"').to_string());
    }
    if scripts.is_empty() {
        return Err(vec![format!("{ACCEPTANCE_SCRIPT} RUST_CHECKS array had no script entries")]);
    }
    Ok(scripts)
}

fn required_array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>, Vec<String>> {
    value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| vec![format!("{POLICY_PATH} missing array field `{key}`")])
}

fn required_str<'a>(value: &'a Value, key: &str, errors: &mut Vec<String>) -> &'a str {
    match value.get(key).and_then(Value::as_str).filter(|text| !text.trim().is_empty()) {
        Some(text) => text,
        None => {
            errors.push(format!("rail entry missing non-empty string `{key}`"));
            ""
        }
    }
}

fn required_string_array(value: &Value, key: &str, errors: &mut Vec<String>) -> Vec<String> {
    let Some(array) = value.get(key).and_then(Value::as_array) else {
        errors.push(format!("rail entry missing array `{key}`"));
        return Vec::new();
    };
    let mut values = Vec::new();
    for item in array {
        match item.as_str().filter(|text| !text.trim().is_empty()) {
            Some(text) => values.push(text.to_string()),
            None => errors.push(format!("rail entry field `{key}` contains a non-string or empty item")),
        }
    }
    if values.is_empty() {
        errors.push(format!("rail entry field `{key}` must not be empty"));
    }
    values
}

fn validate_receipt_schema(policy: &Value, errors: &mut Vec<String>) {
    let fields = policy
        .get("receipt_schema")
        .and_then(|schema| schema.get("required_fields"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    for required in REQUIRED_RECEIPT_FIELDS {
        if !fields.contains(required) {
            errors.push(format!("receipt schema missing required field `{required}`"));
        }
    }
}

fn validate_rails(expected_scripts: &BTreeSet<String>, rails: &[Value], errors: &mut Vec<String>) {
    let allowed = ALLOWED_CLASSES.iter().copied().collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    let mut class_counts: BTreeMap<String, usize> = BTreeMap::new();

    for rail in rails {
        let script = required_str(rail, "script", errors).to_string();
        let class = required_str(rail, "class", errors).to_string();
        let owner = required_str(rail, "owner", errors).to_string();
        let requirement_ids = required_string_array(rail, "requirement_ids", errors);
        let source_artifacts = required_string_array(rail, "source_artifacts", errors);
        let _ = owner;
        let _ = requirement_ids;
        *class_counts.entry(class.clone()).or_default() += 1;

        if !script.is_empty() && !seen.insert(script.clone()) {
            errors.push(format!("duplicate rail inventory entry for `{script}`"));
        }
        if !script.is_empty() && !expected_scripts.contains(&script) {
            errors.push(format!("rail inventory entry `{script}` is not wired in {ACCEPTANCE_SCRIPT}"));
        }
        if !script.is_empty() && !Path::new(&script).is_file() {
            errors.push(format!("rail inventory script does not exist: {script}"));
        }
        if !class.is_empty() && !allowed.contains(class.as_str()) {
            errors.push(format!("rail `{script}` has invalid class `{class}`"));
        }
        for artifact in source_artifacts {
            if !Path::new(&artifact).exists() {
                errors.push(format!("rail `{script}` source artifact does not exist: {artifact}"));
            }
        }
        if class == "temporary_string_presence_check" {
            let failure_mode = required_str(rail, "failure_mode", errors);
            let replacement_path = required_str(rail, "replacement_path", errors);
            if failure_mode == "" || replacement_path == "" {
                errors.push(format!("temporary string rail `{script}` must name failure_mode and replacement_path"));
            }
        }
    }

    for expected in expected_scripts {
        if !seen.contains(expected) {
            errors.push(format!("acceptance script `{expected}` missing from rail inventory"));
        }
    }
    for class in ALLOWED_CLASSES {
        if class_counts.get(*class).copied().unwrap_or_default() == 0 {
            errors.push(format!("rail inventory has no entries classified as `{class}`"));
        }
    }
}

fn validate_negative_fixtures(fixtures: &[Value], errors: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    for fixture in fixtures {
        let case_id = required_negative_str(fixture, "case_id", errors).to_string();
        let expected_outcome = required_negative_str(fixture, "expected_outcome", errors);
        let observed_outcome = required_negative_str(fixture, "observed_outcome", errors);
        let owner = required_negative_str(fixture, "owner", errors);
        let requirement_ids = required_negative_string_array(fixture, "requirement_ids", errors);
        let source_artifacts = required_negative_string_array(fixture, "source_artifacts", errors);
        let markers = required_negative_string_array(fixture, "artifact_markers", errors);
        let _ = expected_outcome;
        let _ = observed_outcome;
        let _ = owner;
        let _ = requirement_ids;

        if !case_id.is_empty() && !seen.insert(case_id.clone()) {
            errors.push(format!("duplicate negative fixture case `{case_id}`"));
        }
        if fixture.get("axes").and_then(Value::as_object).is_none_or(|axes| axes.is_empty()) {
            errors.push(format!("negative fixture `{case_id}` missing non-empty object `axes`"));
        }
        for artifact in &source_artifacts {
            let path = Path::new(artifact);
            if !path.exists() {
                errors.push(format!("negative fixture `{case_id}` source artifact does not exist: {artifact}"));
            }
        }
        for marker in markers {
            if source_artifacts.iter().any(|artifact| artifact_contains(artifact, &marker)) {
                continue;
            }
            errors.push(format!("negative fixture `{case_id}` marker `{marker}` was not found in any source artifact"));
        }
    }

    for required in REQUIRED_NEGATIVE_CASES {
        if !seen.contains(*required) {
            errors.push(format!("missing negative fixture case `{required}`"));
        }
    }
}

fn required_negative_str<'a>(value: &'a Value, key: &str, errors: &mut Vec<String>) -> &'a str {
    match value.get(key).and_then(Value::as_str).filter(|text| !text.trim().is_empty()) {
        Some(text) => text,
        None => {
            errors.push(format!("negative fixture missing non-empty string `{key}`"));
            ""
        }
    }
}

fn required_negative_string_array(value: &Value, key: &str, errors: &mut Vec<String>) -> Vec<String> {
    let Some(array) = value.get(key).and_then(Value::as_array) else {
        errors.push(format!("negative fixture missing array `{key}`"));
        return Vec::new();
    };
    let mut values = Vec::new();
    for item in array {
        match item.as_str().filter(|text| !text.trim().is_empty()) {
            Some(text) => values.push(text.to_string()),
            None => errors.push(format!("negative fixture field `{key}` contains a non-string or empty item")),
        }
    }
    if values.is_empty() {
        errors.push(format!("negative fixture field `{key}` must not be empty"));
    }
    values
}

fn artifact_contains(path: &str, marker: &str) -> bool {
    fs::read_to_string(path).is_ok_and(|text| text.contains(marker))
}

fn validate_fixture_drift_self_tests(errors: &mut Vec<String>) {
    let valid = synthetic_negative_fixtures();
    assert_validation_fails(
        errors,
        "missing negative fixture case",
        &valid[1..],
        "missing negative fixture case `provider-auth-disabled-defaults`",
    );

    let mut missing_axes = valid.clone();
    missing_axes[0].as_object_mut().expect("synthetic fixture is object").remove("axes");
    assert_validation_fails(errors, "missing fixture axes", &missing_axes, "missing non-empty object `axes`");

    let mut missing_outcome = valid.clone();
    missing_outcome[0]["observed_outcome"] = Value::String(String::new());
    assert_validation_fails(
        errors,
        "missing fixture outcome",
        &missing_outcome,
        "negative fixture missing non-empty string `observed_outcome`",
    );

    let mut missing_artifact = valid;
    missing_artifact[0]["source_artifacts"] = json!(["target/embedded-sdk-release/definitely-missing-artifact.rs"]);
    assert_validation_fails(
        errors,
        "missing fixture source artifact",
        &missing_artifact,
        "source artifact does not exist",
    );
}

fn assert_validation_fails(errors: &mut Vec<String>, label: &str, fixtures: &[Value], expected: &str) {
    let mut self_errors = Vec::new();
    validate_negative_fixtures(fixtures, &mut self_errors);
    if !self_errors.iter().any(|error| error.contains(expected)) {
        errors.push(format!(
            "fixture drift self-test `{label}` did not fail with expected diagnostic `{expected}`; got {self_errors:?}"
        ));
    }
}

fn synthetic_negative_fixtures() -> Vec<Value> {
    REQUIRED_NEGATIVE_CASES
        .iter()
        .map(|case_id| {
            json!({
                "case_id": case_id,
                "axes": {"self_test": true},
                "expected_outcome": "self-test fail closed",
                "observed_outcome": "fail_closed",
                "source_artifacts": ["scripts/check-behavioral-lego-rails.rs"],
                "artifact_markers": ["REQUIRED_NEGATIVE_CASES"],
                "owner": "embedded-sdk",
                "requirement_ids": ["behavioral-lego-parity-rails.verification.rail-failure-fixtures"]
            })
        })
        .collect()
}

fn write_receipt(expected_scripts: &BTreeSet<String>, rails: &[Value], policy_text: &str) -> Result<(), String> {
    if let Some(parent) = Path::new(RECEIPT_PATH).parent() {
        fs::create_dir_all(parent).map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let policy_hash = blake3::hash(policy_text.as_bytes()).to_hex().to_string();
    let mut class_counts = BTreeMap::new();
    for rail in rails {
        if let Some(class) = rail.get("class").and_then(Value::as_str) {
            *class_counts.entry(class).or_insert(0usize) += 1;
        }
    }
    let receipt = json!({
        "schema": "clankers.embedded_lego.behavioral_rail_inventory.receipt.v1",
        "case_id": "behavioral-lego-rail-inventory",
        "axes": {"acceptance_scripts": expected_scripts.len(), "rail_entries": rails.len()},
        "expected_outcome": "all wired acceptance scripts classified with owner and requirement ids",
        "observed_outcome": "passed",
        "source_artifacts": [ACCEPTANCE_SCRIPT, POLICY_PATH],
        "sanitized_hashes": {POLICY_PATH: policy_hash},
        "owner": "embedded-sdk",
        "requirement_ids": [
            "behavioral-lego-parity-rails.inventory.classification",
            "behavioral-lego-parity-rails.receipts.schema"
        ],
        "class_counts": class_counts,
        "negative_fixture_cases": REQUIRED_NEGATIVE_CASES,
    });
    let bytes =
        serde_json::to_vec_pretty(&receipt).map_err(|error| format!("failed to encode {RECEIPT_PATH}: {error}"))?;
    fs::write(RECEIPT_PATH, [bytes.as_slice(), b"\n"].concat())
        .map_err(|error| format!("failed to write {RECEIPT_PATH}: {error}"))
}
