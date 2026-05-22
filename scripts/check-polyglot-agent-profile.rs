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
const PROFILE_JSON: &str = "policy/polyglot-agent/agent-profile.json";
const PROFILE_NICKEL: &str = "policy/polyglot-agent/agent-profile.ncl";
const INVALID_PROFILE_JSON: &str = "policy/polyglot-agent/invalid-agent-profile.json";
const DEFAULT_OUTPUT: &str = "target/polyglot-agent/profile-receipt.json";
const EXPECTED_SCHEMA: &str = "clankers.polyglot_agent.profile.v1";
const EXPECTED_RECEIPT_SCHEMA: &str = "clankers.polyglot_agent.receipt.v1";
const REQUIRED_TOOLS: &[(&str, &str)] = &[
    ("steel_orchestrate", "clankers/steel/orchestrate.run"),
    ("wasm_tool_execute", "clankers/wasm/tool.execute"),
];
const ALLOWED_TOOL_MODES: &[&str] = &["host", "wasm", "disabled_placeholder"];
const REQUIRED_NICKEL_MARKERS: &[&str] = &[
    "AgentProfile",
    "PromptTemplate",
    "ToolManifest",
    "RuntimeProfile",
    "ModelProfile",
    "JsonSchema",
];
const FORBIDDEN_SAFE_RECEIPT_FIELDS: &[&str] = &[
    "compact_ucan",
    "raw_proof",
    "credential",
    "provider_payload",
    "raw_prompt",
    "tool_body",
    "absolute_path",
];
const REQUIRED_REDACTED_FIELDS: &[&str] = &[
    "raw_prompt",
    "provider_payload",
    "compact_ucan",
    "raw_proof",
    "credential",
    "tool_body",
    "absolute_path",
];

fn main() -> ExitCode {
    match run() {
        Ok(path) => {
            println!("polyglot agent profile receipt written to {}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("polyglot agent profile check failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<PathBuf, String> {
    let profile_text =
        fs::read_to_string(PROFILE_JSON).map_err(|error| format!("failed to read {PROFILE_JSON}: {error}"))?;
    let profile: Value =
        serde_json::from_str(&profile_text).map_err(|error| format!("failed to parse {PROFILE_JSON}: {error}"))?;
    let nickel_text =
        fs::read_to_string(PROFILE_NICKEL).map_err(|error| format!("failed to read {PROFILE_NICKEL}: {error}"))?;
    let invalid_text = fs::read_to_string(INVALID_PROFILE_JSON)
        .map_err(|error| format!("failed to read {INVALID_PROFILE_JSON}: {error}"))?;
    let invalid_profile: Value = serde_json::from_str(&invalid_text)
        .map_err(|error| format!("failed to parse {INVALID_PROFILE_JSON}: {error}"))?;

    let mut errors = Vec::new();
    validate_nickel_markers(&nickel_text, &mut errors);
    validate_profile(&profile, &mut errors);
    validate_invalid_fixture(&invalid_profile, &mut errors);

    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }

    let hashed_artifacts = [
        PROFILE_JSON,
        PROFILE_NICKEL,
        INVALID_PROFILE_JSON,
        "scripts/check-polyglot-agent-profile.rs",
    ]
    .iter()
    .map(|path| hash_artifact(Path::new(path)))
    .collect::<Result<Vec<_>, _>>()?;
    let receipt = json!({
        "schema": "clankers.polyglot_agent.profile_check_receipt.v1",
        "profile": PROFILE_JSON,
        "nickel_contract": PROFILE_NICKEL,
        "invalid_fixture": INVALID_PROFILE_JSON,
        "validated_surfaces": [
            "nickel-contract-markers",
            "prompt-template-variable-parity",
            "model-profile-defaults",
            "runtime-profile-defaults-and-budgets",
            "tool-schema-host-parity-shape",
            "receipt-redaction-policy",
            "negative-invalid-profile-fixture"
        ],
        "hashed_artifacts": hashed_artifacts,
        "guidance": "Nickel owns boot-time agent profile contracts. Rust consumes exported typed data and fails closed before activating mismatched prompt, model, runtime, or tool profiles."
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
    let prompt_names = validate_prompts(profile, errors);
    let model_names = validate_models(profile, errors);
    let runtime_names = validate_runtime_profiles(profile, errors);
    validate_default_ref(profile, "default_model_profile", &model_names, errors);
    validate_default_ref(profile, "default_runtime_profile", &runtime_names, errors);
    validate_tools(profile, errors);
    validate_receipt_policy(profile, errors);
    if prompt_names.is_empty() {
        errors.push("profile must declare at least one prompt".to_string());
    }
}

fn validate_prompts(profile: &Value, errors: &mut Vec<String>) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for prompt in array(profile, "prompts", errors) {
        let name = required_str(prompt, "name", errors).to_string();
        if !name.is_empty() && !names.insert(name.clone()) {
            errors.push(format!("duplicate prompt `{name}`"));
        }
        let template = required_str(prompt, "template", errors);
        if template.trim().is_empty() {
            errors.push(format!("prompt `{name}` must have non-empty template"));
        }
        for variable in string_set(prompt, "required_variables", errors) {
            let marker = format!("{{{{{variable}}}}}");
            if !template.contains(&marker) {
                errors.push(format!("prompt `{name}` requires `{variable}` but template lacks `{marker}`"));
            }
        }
    }
    names
}

fn validate_models(profile: &Value, errors: &mut Vec<String>) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for model in array(profile, "model_profiles", errors) {
        let name = required_str(model, "name", errors).to_string();
        if !name.is_empty() && !names.insert(name.clone()) {
            errors.push(format!("duplicate model profile `{name}`"));
        }
        for field in ["provider", "model"] {
            if required_str(model, field, errors).is_empty() {
                errors.push(format!("model profile `{name}` field `{field}` must be non-empty"));
            }
        }
        let temperature = required_number(model, "temperature", errors);
        if !(0.0..=2.0).contains(&temperature) {
            errors.push(format!("model profile `{name}` temperature must be within 0..=2"));
        }
        if required_u64(model, "max_output_tokens", errors) == 0 {
            errors.push(format!("model profile `{name}` max_output_tokens must be positive"));
        }
    }
    names
}

fn validate_runtime_profiles(profile: &Value, errors: &mut Vec<String>) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for runtime in array(profile, "runtime_profiles", errors) {
        let name = required_str(runtime, "name", errors).to_string();
        if !name.is_empty() && !names.insert(name.clone()) {
            errors.push(format!("duplicate runtime profile `{name}`"));
        }
        for field in ["steel_profile", "wasm_profile"] {
            if required_str(runtime, field, errors).is_empty() {
                errors.push(format!("runtime profile `{name}` field `{field}` must be non-empty"));
            }
        }
        for field in [
            "max_prompt_bytes",
            "max_tool_calls",
            "max_steel_host_calls",
            "max_wasm_memory_bytes",
            "max_wasm_fuel",
        ] {
            if required_u64(runtime, field, errors) == 0 {
                errors.push(format!("runtime profile `{name}` field `{field}` must be positive"));
            }
        }
    }
    names
}

fn validate_default_ref(profile: &Value, field: &str, allowed: &BTreeSet<String>, errors: &mut Vec<String>) {
    let value = required_str(profile, field, errors);
    if !allowed.contains(value) {
        errors.push(format!("{field} `{value}` does not reference a declared profile"));
    }
}

fn validate_tools(profile: &Value, errors: &mut Vec<String>) {
    let mut tools = BTreeMap::new();
    for tool in array(profile, "tools", errors) {
        let name = required_str(tool, "name", errors).to_string();
        if !name.is_empty() && tools.insert(name.clone(), tool).is_some() {
            errors.push(format!("duplicate tool `{name}`"));
        }
        let mode = required_str(tool, "mode", errors);
        if !ALLOWED_TOOL_MODES.contains(&mode) {
            errors.push(format!("tool `{name}` has unsupported mode `{mode}`"));
        }
        if required_str(tool, "description", errors).trim().is_empty() {
            errors.push(format!("tool `{name}` must have description"));
        }
        validate_schema(tool.get("input_schema"), &format!("tool `{name}` input_schema"), errors);
        validate_schema(tool.get("output_schema"), &format!("tool `{name}` output_schema"), errors);
        let capabilities = string_set(tool, "required_capabilities", errors);
        if mode != "disabled_placeholder" && capabilities.is_empty() {
            errors.push(format!("tool `{name}` must declare required capabilities"));
        }
        let ability = tool.get("ucan_ability").and_then(Value::as_str).unwrap_or_default();
        if mode != "disabled_placeholder" && (ability.is_empty() || ability == "*" || !ability.starts_with("clankers/"))
        {
            errors.push(format!("tool `{name}` has unsafe UCAN ability `{ability}`"));
        }
    }
    for (tool_name, expected_ability) in REQUIRED_TOOLS {
        match tools.get(*tool_name) {
            Some(tool) => {
                let actual = tool.get("ucan_ability").and_then(Value::as_str).unwrap_or_default();
                if actual != *expected_ability {
                    errors.push(format!("tool `{tool_name}` ability is `{actual}`, expected `{expected_ability}`"));
                }
            }
            None => errors.push(format!("missing required tool `{tool_name}`")),
        }
    }
}

fn validate_schema(schema: Option<&Value>, label: &str, errors: &mut Vec<String>) {
    let Some(schema) = schema else {
        errors.push(format!("{label} is missing"));
        return;
    };
    if required_str(schema, "type", errors) != "object" {
        errors.push(format!("{label} must be an object schema"));
    }
    if !schema.get("properties").is_some_and(Value::is_object) {
        errors.push(format!("{label} must declare object properties"));
    }
    if !schema.get("required").is_some_and(Value::is_array) {
        errors.push(format!("{label} must declare required array"));
    }
    if schema.get("additionalProperties").and_then(Value::as_bool) != Some(false) {
        errors.push(format!("{label} must set additionalProperties=false"));
    }
}

fn validate_receipt_policy(profile: &Value, errors: &mut Vec<String>) {
    let receipt = match profile.get("receipt_policy") {
        Some(Value::Object(_)) => &profile["receipt_policy"],
        _ => {
            errors.push("receipt_policy must be an object".to_string());
            return;
        }
    };
    if required_str(receipt, "schema", errors) != EXPECTED_RECEIPT_SCHEMA {
        errors.push(format!("receipt policy schema must be {EXPECTED_RECEIPT_SCHEMA}"));
    }
    let safe = string_set(receipt, "safe_fields", errors);
    let redacted = string_set(receipt, "redacted_fields", errors);
    for forbidden in FORBIDDEN_SAFE_RECEIPT_FIELDS {
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

fn validate_invalid_fixture(profile: &Value, errors: &mut Vec<String>) {
    let mut invalid_errors = Vec::new();
    validate_profile(profile, &mut invalid_errors);
    for expected in [
        "temperature",
        "unsupported mode",
        "agent_name",
        "default_model_profile",
        "safe_fields",
    ] {
        if !invalid_errors.iter().any(|error| error.contains(expected)) {
            errors.push(format!("invalid fixture did not trigger expected error containing `{expected}`"));
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

fn required_number(value: &Value, field: &str, errors: &mut Vec<String>) -> f64 {
    match value.get(field).and_then(Value::as_f64) {
        Some(number) => number,
        None => {
            errors.push(format!("missing number field `{field}`"));
            0.0
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
