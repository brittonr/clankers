#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"

[dependencies]
blake3 = "1"
clankers-runtime = { path = "../crates/clankers-runtime" }
serde_json = "1"
---

use std::fs;
use std::path::Path;
use std::process::ExitCode;

use clankers_runtime::SteelRuntimeRequest;
use clankers_runtime::evaluate_steel_request;
use serde_json::Value;
use serde_json::json;

const ERROR_EXIT: u8 = 1;
const MANIFEST_PATH: &str = "policy/steel-eval/controlled-corpus.json";
const OUT_DIR: &str = "target/steel-eval/controlled-corpus";
const RECEIPT_PATH: &str = "target/steel-eval/controlled-corpus/receipt.json";

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("steel_eval controlled corpus receipt written to {RECEIPT_PATH}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("steel_eval controlled corpus check failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), String> {
    let manifest = json_file(MANIFEST_PATH)?;
    validate_manifest(&manifest)?;
    let cases = manifest.get("cases").and_then(Value::as_array).ok_or("manifest cases must be an array")?;
    let mut passed = 0usize;
    let mut regressions = Vec::new();
    let mut case_receipts = Vec::new();

    for case in cases {
        let id = required_str(case, "id")?;
        let source = required_str(case, "source")?;
        let expected_status = required_str(case, "expected_status")?;
        let expected_reason = required_str(case, "expected_reason")?;
        let runtime = evaluate_steel_request(&SteelRuntimeRequest::pure(source));
        let status =
            serde_json::to_value(&runtime.status).map_err(|error| format!("serialize runtime status: {error}"))?;
        let reason =
            serde_json::to_value(&runtime.reason_code).map_err(|error| format!("serialize runtime reason: {error}"))?;
        let status_text = status.as_str().ok_or("runtime status did not serialize as string")?;
        let reason_text = reason.as_str().ok_or("runtime reason did not serialize as string")?;
        let output_matches = match case.get("expected_output").and_then(Value::as_str) {
            Some(expected) => runtime.output.as_deref() == Some(expected),
            None => true,
        };
        let matched = status_text == expected_status && reason_text == expected_reason && output_matches;
        if matched {
            passed += 1;
        } else {
            regressions.push(id.to_string());
        }
        case_receipts.push(json!({
            "id": id,
            "source_hash": format!("blake3:{}", blake3::hash(source.as_bytes()).to_hex()),
            "status": status_text,
            "reason": reason_text,
            "matched_expected": matched,
            "output_hash": runtime.output.as_ref().map(|out| format!("blake3:{}", blake3::hash(out.as_bytes()).to_hex())),
            "output_redacted": true,
            "host_calls": runtime.host_calls.len(),
            "ambient_authority": runtime.ambient_authority
        }));
    }

    let thresholds = manifest.get("thresholds").ok_or("manifest missing thresholds")?;
    let minimum_cases = required_u64(thresholds, "minimum_cases")? as usize;
    let minimum_success_rate = required_f64(thresholds, "minimum_success_rate")?;
    let maximum_regressions = required_u64(thresholds, "maximum_regressions")? as usize;
    let success_rate = passed as f64 / cases.len() as f64;
    let outcome_class = if cases.len() < minimum_cases {
        "blocked"
    } else if !regressions.is_empty() && regressions.len() > maximum_regressions {
        "regression"
    } else if success_rate < minimum_success_rate {
        "unchanged_or_noise"
    } else {
        "pass"
    };
    let recommended = outcome_class == "pass";
    fs::create_dir_all(OUT_DIR).map_err(|error| format!("create {OUT_DIR}: {error}"))?;
    let receipt = json!({
        "schema": "clankers.steel_eval.controlled_corpus.receipt.v1",
        "requirements": [
            "r[steel-eval-controlled-corpus-dogfood.corpus-manifest]",
            "r[steel-eval-controlled-corpus-dogfood.threshold-budget]",
            "r[steel-eval-controlled-corpus-dogfood.receipt-taxonomy]",
            "r[steel-eval-controlled-corpus-dogfood.no-authority-expansion]"
        ],
        "manifest": {
            "path": MANIFEST_PATH,
            "hash": hash_path(MANIFEST_PATH)?,
            "corpus_id": required_str(&manifest, "corpus_id")?,
            "case_count": cases.len()
        },
        "thresholds": thresholds,
        "results": {
            "passed": passed,
            "regressions": regressions,
            "success_rate": success_rate,
            "outcome_class": outcome_class,
            "recommended": recommended,
            "issue_codes": issue_codes(outcome_class)
        },
        "authority_boundary": manifest.get("authority_boundary").cloned().ok_or("manifest missing authority_boundary")?,
        "case_receipts": case_receipts,
        "redaction": {
            "raw_source_omitted_from_case_receipts": true,
            "raw_output_omitted_from_case_receipts": true,
            "credential_material_present": false
        }
    });
    write_json(RECEIPT_PATH, &receipt)?;
    if recommended {
        Ok(())
    } else {
        Err(format!("controlled corpus not recommended: outcome_class={outcome_class}"))
    }
}

fn validate_manifest(manifest: &Value) -> Result<(), String> {
    require_eq(manifest, "schema", "clankers.steel_eval.controlled_corpus.v1")?;
    let boundary = manifest.get("authority_boundary").ok_or("manifest missing authority_boundary")?;
    for field in [
        "ambient_authority",
        "network",
        "filesystem",
        "process",
        "mutation",
        "credentials",
    ] {
        if boundary.get(field) != Some(&Value::Bool(false)) {
            return Err(format!("authority boundary `{field}` must be false"));
        }
    }
    if boundary.get("max_host_calls") != Some(&Value::Number(0.into())) {
        return Err("authority boundary max_host_calls must be 0".to_string());
    }
    for field in ["host_functions", "session_capabilities"] {
        if !boundary.get(field).and_then(Value::as_array).is_some_and(Vec::is_empty) {
            return Err(format!("authority boundary `{field}` must be empty"));
        }
    }
    let cases = manifest.get("cases").and_then(Value::as_array).ok_or("manifest cases must be an array")?;
    if cases.is_empty() {
        return Err("manifest cases must not be empty".to_string());
    }
    Ok(())
}

fn issue_codes(outcome_class: &str) -> Vec<&'static str> {
    match outcome_class {
        "pass" => vec!["ok"],
        "blocked" => vec!["missing-or-too-small-corpus"],
        "regression" => vec!["regression-budget-exceeded"],
        "unchanged_or_noise" => vec!["minimum-improvement-not-met"],
        "evaluation_failure" => vec!["evaluation-failure"],
        "redaction" => vec!["redaction-required"],
        _ => vec!["unknown-outcome"],
    }
}

fn json_file(path: &str) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|error| format!("read {path}: {error}"))?;
    serde_json::from_str(&text).map_err(|error| format!("parse {path}: {error}"))
}

fn required_str<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value.get(field).and_then(Value::as_str).ok_or_else(|| format!("missing string `{field}`"))
}

fn required_u64(value: &Value, field: &str) -> Result<u64, String> {
    value.get(field).and_then(Value::as_u64).ok_or_else(|| format!("missing integer `{field}`"))
}

fn required_f64(value: &Value, field: &str) -> Result<f64, String> {
    value.get(field).and_then(Value::as_f64).ok_or_else(|| format!("missing number `{field}`"))
}

fn require_eq(value: &Value, field: &str, expected: &str) -> Result<(), String> {
    let actual = required_str(value, field)?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!("{field} expected `{expected}`, got `{actual}`"))
    }
}

fn hash_path(path: &str) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| format!("hash read {path}: {error}"))?;
    Ok(format!("blake3:{}", blake3::hash(&bytes).to_hex()))
}

fn write_json(path: &str, value: &Value) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|error| format!("serialize {path}: {error}"))?;
    bytes.push(b'\n');
    fs::write(Path::new(path), bytes).map_err(|error| format!("write {path}: {error}"))
}
