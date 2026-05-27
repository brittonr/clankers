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
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;

use serde_json::Value;
use serde_json::json;

const ERROR_EXIT: u8 = 1;
const POLICY_JSON: &str = "policy/steel-self-mutation/mutation-policy.json";
const POLICY_NICKEL: &str = "policy/steel-self-mutation/mutation-policy.ncl";
const INVALID_POLICY_JSON: &str = "policy/steel-self-mutation/invalid-policy.json";
const DEFAULT_OUTPUT: &str = "target/steel-self-mutation/policy-receipt.json";
const EXPECTED_SCHEMA: &str = "clankers.steel_self_mutation.policy.v1";
const EXPECTED_RECEIPT_SCHEMA: &str = "clankers.steel_self_mutation.receipt.v1";
const REQUIRED_TARGET_CLASSES: &[&str] = &["skill", "prompt", "tool_description", "repo_code", "orchestration_pack"];
const REQUIRED_VERBS: &[(&str, &str)] = &[
    ("propose_mutation", "clankers/steel/mutation.propose"),
    ("apply_mutation", "clankers/steel/mutation.apply"),
    ("commit_mutation", "clankers/steel/mutation.commit"),
    ("rollback_mutation", "clankers/steel/mutation.rollback"),
];
const REQUIRED_NICKEL_MARKERS: &[&str] = &[
    "TargetClass",
    "MutationVerb",
    "RuntimeProfile",
    "ReceiptPolicy",
    "deny_wildcard_resources",
    "max_delegation_depth",
    "safe_receipt_fields",
];
const FORBIDDEN_RECEIPT_MARKERS: &[&str] = &[
    "ucan_compact_token",
    "private_key",
    "bearer_token",
    "api_key",
    "raw_proof",
];
const REDACTION_FIELDS: &[&str] = &[
    "patch_body",
    "source",
    "raw_diagnostics",
    "absolute_paths",
    "provider_payloads",
    "credentials",
];

fn main() -> ExitCode {
    match run() {
        Ok(path) => {
            println!("steel self-mutation policy receipt written to {}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("steel self-mutation policy check failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<PathBuf, String> {
    let policy_text =
        fs::read_to_string(POLICY_JSON).map_err(|error| format!("failed to read {POLICY_JSON}: {error}"))?;
    let policy: Value =
        serde_json::from_str(&policy_text).map_err(|error| format!("failed to parse {POLICY_JSON}: {error}"))?;
    let nickel_text =
        fs::read_to_string(POLICY_NICKEL).map_err(|error| format!("failed to read {POLICY_NICKEL}: {error}"))?;
    let invalid_text = fs::read_to_string(INVALID_POLICY_JSON)
        .map_err(|error| format!("failed to read {INVALID_POLICY_JSON}: {error}"))?;
    let invalid_policy: Value = serde_json::from_str(&invalid_text)
        .map_err(|error| format!("failed to parse {INVALID_POLICY_JSON}: {error}"))?;

    let mut errors = Vec::new();
    validate_nickel_markers(&nickel_text, &mut errors);
    validate_policy(&policy, &mut errors);
    validate_invalid_fixture(&invalid_policy, &mut errors);

    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }

    let hashed_artifacts = [
        POLICY_JSON,
        POLICY_NICKEL,
        INVALID_POLICY_JSON,
        "crates/clankers-runtime/src/steel_mutation.rs",
        "scripts/check-steel-self-mutation-policy.rs",
    ]
    .iter()
    .map(|path| hash_artifact(Path::new(path)))
    .collect::<Result<Vec<_>, _>>()?;
    let receipt = json!({
        "schema": "clankers.steel_self_mutation.policy_check_receipt.v1",
        "policy": POLICY_JSON,
        "nickel_contract": POLICY_NICKEL,
        "invalid_fixture": INVALID_POLICY_JSON,
        "validated_surfaces": [
            "nickel-contract-markers",
            "target-class-policy",
            "ucan-ability-resource-vocabulary",
            "runtime-profile-budgets",
            "receipt-redaction-contract",
            "negative-invalid-policy-fixture"
        ],
        "hashed_artifacts": hashed_artifacts,
        "guidance": "Nickel owns declarative mutation policy. UCAN is runtime authority. Steel requests mutation through typed host functions; Rust enforces policy and writes receipts."
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
            errors.push(format!("{POLICY_NICKEL} missing marker `{marker}`"));
        }
    }
}

fn validate_policy(policy: &Value, errors: &mut Vec<String>) {
    if required_str(policy, "schema", errors) != EXPECTED_SCHEMA {
        errors.push(format!("policy schema must be {EXPECTED_SCHEMA}"));
    }
    let verbs = validate_mutation_verbs(policy, errors);
    validate_target_classes(policy, &verbs, errors);
    validate_runtime_profiles(policy, errors);
    validate_ucan(policy, errors);
    validate_receipt(policy, errors);
}

fn validate_mutation_verbs(policy: &Value, errors: &mut Vec<String>) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut abilities = BTreeMap::new();
    for verb in array(policy, "mutation_verbs", errors) {
        let name = required_str(verb, "name", errors).to_string();
        let ability = required_str(verb, "ucan_ability", errors).to_string();
        let host_function = required_str(verb, "host_function", errors);
        if !name.is_empty() && !seen.insert(name.clone()) {
            errors.push(format!("duplicate mutation verb `{name}`"));
        }
        if !host_function.starts_with("steel.host.") {
            errors.push(format!("verb `{name}` host function must use steel.host.* namespace"));
        }
        if ability == "*" || !ability.starts_with("clankers/steel/mutation.") {
            errors.push(format!("verb `{name}` has unsafe UCAN ability `{ability}`"));
        }
        abilities.insert(name, ability);
    }
    for (verb, ability) in REQUIRED_VERBS {
        match abilities.get(*verb) {
            Some(actual) if actual == ability => {}
            Some(actual) => errors.push(format!("verb `{verb}` ability is `{actual}`, expected `{ability}`")),
            None => errors.push(format!("missing required mutation verb `{verb}`")),
        }
    }
    seen
}

fn validate_target_classes(policy: &Value, verbs: &BTreeSet<String>, errors: &mut Vec<String>) {
    let mut classes = BTreeSet::new();
    for class in array(policy, "target_classes", errors) {
        let name = required_str(class, "name", errors).to_string();
        if !name.is_empty() && !classes.insert(name.clone()) {
            errors.push(format!("duplicate target class `{name}`"));
        }
        let resource_prefix = required_str(class, "resource_prefix", errors);
        if resource_prefix == "*" || !resource_prefix.ends_with(':') {
            errors.push(format!("target class `{name}` has unsafe resource prefix `{resource_prefix}`"));
        }
        let roots = string_set(class, "allowed_path_roots", errors);
        if roots.is_empty() {
            errors.push(format!("target class `{name}` must declare allowed path roots"));
        }
        for root in &roots {
            if root.contains("..") || root.starts_with('/') || root.contains(".git") {
                errors.push(format!("target class `{name}` has unsafe allowed root `{root}`"));
            }
        }
        let denied = string_set(class, "denied_path_patterns", errors);
        for required in ["../", "/.git/", "**/.env*", "**/*secret*"] {
            if !denied.contains(required) {
                errors.push(format!("target class `{name}` missing deny pattern `{required}`"));
            }
        }
        for verb in string_set(class, "allowed_verbs", errors) {
            if !verbs.contains(verb.as_str()) {
                errors.push(format!("target class `{name}` allows unknown verb `{verb}`"));
            }
        }
        for field in ["approval_tier", "preflight_profile", "verification_profile"] {
            let value = required_str(class, field, errors);
            if value == "none" || value.is_empty() {
                errors.push(format!("target class `{name}` field `{field}` must be explicit and non-none"));
            }
        }
        if class.get("rollback_required").and_then(Value::as_bool) != Some(true) {
            errors.push(format!("target class `{name}` must require rollback"));
        }
    }
    for required in REQUIRED_TARGET_CLASSES {
        if !classes.contains(*required) {
            errors.push(format!("missing target class `{required}`"));
        }
    }
}

fn validate_runtime_profiles(policy: &Value, errors: &mut Vec<String>) {
    let mut names = BTreeSet::new();
    for profile in array(policy, "runtime_profiles", errors) {
        let name = required_str(profile, "name", errors).to_string();
        names.insert(name.clone());
        if profile.get("ambient_authority").and_then(Value::as_bool) != Some(false) {
            errors.push(format!("runtime profile `{name}` must deny ambient authority"));
        }
        for field in ["max_source_bytes", "max_output_bytes"] {
            if required_u64(profile, field, errors) == 0 {
                errors.push(format!("runtime profile `{name}` field `{field}` must be nonzero"));
            }
        }
    }
    for required in ["steel-live-mutation-default-deny", "steel-live-mutation-requester"] {
        if !names.contains(required) {
            errors.push(format!("missing runtime profile `{required}`"));
        }
    }
}

fn validate_ucan(policy: &Value, errors: &mut Vec<String>) {
    let Some(ucan) = policy.get("ucan") else {
        errors.push("policy missing ucan section".to_string());
        return;
    };
    if ucan.get("required").and_then(Value::as_bool) != Some(true) {
        errors.push("ucan.required must be true".to_string());
    }
    if ucan.get("deny_wildcard_resources").and_then(Value::as_bool) != Some(true) {
        errors.push("ucan.deny_wildcard_resources must be true".to_string());
    }
    let max_delegation_depth = required_u64(ucan, "max_delegation_depth", errors);
    if max_delegation_depth == 0 || max_delegation_depth > 8 {
        errors.push("ucan.max_delegation_depth must be bounded and nonzero".to_string());
    }
    let safe_fields = string_set(ucan, "safe_receipt_fields", errors);
    for required in ["ability", "resource", "expiry_status", "authorization_outcome"] {
        if !safe_fields.contains(required) {
            errors.push(format!("ucan safe_receipt_fields missing `{required}`"));
        }
    }
    for forbidden in ["raw_proof", "compact_token", "private_key", "bearer_token"] {
        if safe_fields.contains(forbidden) {
            errors.push(format!("ucan safe_receipt_fields includes forbidden `{forbidden}`"));
        }
    }
}

fn validate_receipt(policy: &Value, errors: &mut Vec<String>) {
    let Some(receipt) = policy.get("receipt") else {
        errors.push("policy missing receipt section".to_string());
        return;
    };
    if required_str(receipt, "schema", errors) != EXPECTED_RECEIPT_SCHEMA {
        errors.push(format!("receipt schema must be {EXPECTED_RECEIPT_SCHEMA}"));
    }
    if receipt.get("include_policy_hash").and_then(Value::as_bool) != Some(true) {
        errors.push("receipt must include policy hash".to_string());
    }
    if receipt.get("include_safe_ucan_metadata").and_then(Value::as_bool) != Some(true) {
        errors.push("receipt must include safe UCAN metadata".to_string());
    }
    let redactions = string_set(receipt, "redact_fields", errors);
    for required in REDACTION_FIELDS {
        if !redactions.contains(*required) {
            errors.push(format!("receipt redaction missing `{required}`"));
        }
    }
    let forbidden_markers = string_set(receipt, "forbidden_receipt_markers", errors);
    for required in FORBIDDEN_RECEIPT_MARKERS {
        if !forbidden_markers.contains(*required) {
            errors.push(format!("receipt forbidden markers missing `{required}`"));
        }
    }
}

fn validate_invalid_fixture(invalid_policy: &Value, errors: &mut Vec<String>) {
    let mut fixture_errors = Vec::new();
    validate_policy(invalid_policy, &mut fixture_errors);
    for expected in [
        "unsafe resource prefix",
        "unknown verb",
        "must require rollback",
        "ucan.required must be true",
        "runtime profile `unsafe` must deny ambient authority",
        "receipt must include policy hash",
    ] {
        if !fixture_errors.iter().any(|error| error.contains(expected)) {
            errors.push(format!("invalid fixture did not trigger expected error containing `{expected}`"));
        }
    }
}

fn array<'a>(value: &'a Value, field: &str, errors: &mut Vec<String>) -> Vec<&'a Value> {
    match value.get(field).and_then(Value::as_array) {
        Some(values) => values.iter().collect(),
        None => {
            errors.push(format!("missing array field `{field}`"));
            Vec::new()
        }
    }
}

fn required_str<'a>(value: &'a Value, field: &str, errors: &mut Vec<String>) -> &'a str {
    match value.get(field).and_then(Value::as_str) {
        Some(text) if !text.is_empty() => text,
        _ => {
            errors.push(format!("missing string field `{field}`"));
            ""
        }
    }
}

fn required_u64(value: &Value, field: &str, errors: &mut Vec<String>) -> u64 {
    match value.get(field).and_then(Value::as_u64) {
        Some(number) => number,
        None => {
            errors.push(format!("missing numeric field `{field}`"));
            0
        }
    }
}

fn string_set(value: &Value, field: &str, errors: &mut Vec<String>) -> BTreeSet<String> {
    match value.get(field).and_then(Value::as_array) {
        Some(values) => values
            .iter()
            .filter_map(|item| match item.as_str() {
                Some(text) if !text.is_empty() => Some(text.to_string()),
                _ => {
                    errors.push(format!("field `{field}` contains a non-string or empty item"));
                    None
                }
            })
            .collect(),
        None => {
            errors.push(format!("missing string array field `{field}`"));
            BTreeSet::new()
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
        bytes += u64::try_from(read).map_err(|error| format!("read size overflow for {}: {error}", path.display()))?;
        hasher.update(&buffer[..read]);
    }
    Ok(json!({"path": path.to_string_lossy(), "bytes": bytes, "blake3": hasher.finalize().to_hex().to_string()}))
}
