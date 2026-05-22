#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"

[dependencies]
blake3 = "1"
serde_json = "1"
---

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;

use serde_json::json;

const ERROR_EXIT: u8 = 1;
const RUNTIME: &str = "crates/clankers-runtime/src/steel_orchestration.rs";
const AGENT: &str = "crates/clankers-agent/src/turn/steel_planning.rs";
const CONFIG: &str = "crates/clankers-config/src/settings.rs";
const DOC: &str = "docs/src/reference/steel-turn-planning-ucan-authority.md";
const SUMMARY: &str = "docs/src/SUMMARY.md";
const TASKS: &str = "cairn/changes/steel-turn-planning-ucan-authority/tasks.md";
const ARCHIVED_TASKS: &str = "cairn/archive/2026-05-22-steel-turn-planning-ucan-authority/tasks.md";
const OUTPUT: &str = "target/steel-turn-planning-ucan-authority/receipt.json";

const RUNTIME_MARKERS: &[&str] = &[
    "SteelTurnPlanningAuthorityGrant",
    "SteelTurnPlanningAuthorityReceipt",
    "UcanAuthorityDenied",
    "authorize_steel_turn_planning_invocation",
    "basalt_enforce",
    "ExpiredGrant",
    "RevokedGrant",
    "WrongResource",
    "MissingGrant",
    "ucan_authority_receipt",
    "explicit_ucan_authority_grant_allows_steel_planning",
    "expired_revoked_and_wrong_scope_authority_grants_fail_closed_before_dynamic_action",
];
const AGENT_MARKERS: &[&str] = &[
    "ucan_authority_grants",
    "authority_grants_from_settings",
    "ucan_authority={:?}",
    "ucan_reason={:?}",
    "settings_activation_builds_comparison_config_from_profile_and_script",
];
const CONFIG_MARKERS: &[&str] = &[
    "SteelTurnPlanningAuthorityGrantSettings",
    "ucan_authority_grants",
    "BlankUcanAuthorityGrant",
    "ucanAuthorityGrants",
    "steel_turn_planning_validation_rejects_blank_authority_grant",
];
const DOC_MARKERS: &[&str] = &[
    "Steel Turn Planning UCAN Authority",
    "Basalt/UCAN vocabulary",
    "clankers:agent-turn-planning",
    "UcanAuthorityDenied",
    "target/steel-turn-planning-ucan-authority/receipt.json",
];
const FORBIDDEN_DOC_MARKERS: &[&str] = &[
    "credential_value",
    "raw_ucan_token",
    "provider_payload =",
    "raw_prompt =",
];

fn main() -> ExitCode {
    match run() {
        Ok(path) => {
            println!("steel turn planning UCAN authority receipt written to {}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("steel turn planning UCAN authority check failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<PathBuf, String> {
    let runtime = read(RUNTIME)?;
    let agent = read(AGENT)?;
    let config = read(CONFIG)?;
    let doc = read(DOC)?;
    let summary = read(SUMMARY)?;
    let task_path = existing_task_path();
    let tasks = read(task_path)?;
    let mut errors = Vec::new();

    require_all(RUNTIME, &runtime, RUNTIME_MARKERS, &mut errors);
    require_all(AGENT, &agent, AGENT_MARKERS, &mut errors);
    require_all(CONFIG, &config, CONFIG_MARKERS, &mut errors);
    require_all(DOC, &doc, DOC_MARKERS, &mut errors);
    forbid_all(DOC, &doc, FORBIDDEN_DOC_MARKERS, &mut errors);
    if !summary.contains("steel-turn-planning-ucan-authority.md") {
        errors.push(format!("{SUMMARY} must link the UCAN authority reference doc"));
    }
    for unchecked in unchecked_tasks(&tasks) {
        if !unchecked.contains("V4.") && !unchecked.contains("V5.") {
            errors.push(format!("{task_path} still has unchecked implementation/test task: {unchecked}"));
        }
    }
    if !runtime.contains("authorization_receipts.is_empty()") {
        errors.push(format!("{RUNTIME} must prove denied authority blocks before dynamic action authorization"));
    }
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }

    let artifacts = [
        RUNTIME,
        AGENT,
        CONFIG,
        DOC,
        SUMMARY,
        task_path,
        "scripts/check-steel-turn-planning-ucan-authority.rs",
    ]
    .iter()
    .map(|path| hash_artifact(Path::new(path)))
    .collect::<Result<Vec<_>, _>>()?;
    let receipt = json!({
        "schema": "clankers.steel_turn_planning_ucan_authority.receipt.v1",
        "validated_surfaces": [
            "runtime-authority-dto",
            "basalt-backed-authority-gate",
            "fail-closed-denials-before-steel-effect",
            "settings-to-runtime-grant-threading",
            "daemon-visible-redacted-authority-status",
            "docs-and-cairn-tasks"
        ],
        "hashed_artifacts": artifacts,
        "redaction": {
            "raw_ucan_tokens": "omitted",
            "credentials": "omitted",
            "prompt_bodies": "omitted",
            "profile_bodies": "omitted",
            "script_bodies": "omitted",
            "provider_payloads": "omitted"
        },
        "guidance": "Steel Scheme remains request/planning logic; Rust validates Basalt/UCAN authority and blocks before provider/tool effects."
    });
    let output = PathBuf::from(OUTPUT);
    let parent = output.parent().ok_or_else(|| format!("{} has no parent", output.display()))?;
    fs::create_dir_all(parent).map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let bytes = serde_json::to_vec_pretty(&receipt).map_err(|error| format!("failed to encode receipt: {error}"))?;
    fs::write(&output, [bytes.as_slice(), b"\n"].concat())
        .map_err(|error| format!("failed to write {}: {error}", output.display()))?;
    Ok(output)
}

fn existing_task_path() -> &'static str {
    if Path::new(TASKS).exists() {
        TASKS
    } else {
        ARCHIVED_TASKS
    }
}

fn unchecked_tasks(text: &str) -> impl Iterator<Item = &str> {
    text.lines().filter(|line| line.trim_start().starts_with("- [ ]"))
}

fn read(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("failed to read {path}: {error}"))
}

fn require_all(path: &str, text: &str, markers: &[&str], errors: &mut Vec<String>) {
    for marker in markers {
        if !text.contains(marker) {
            errors.push(format!("{path} missing marker `{marker}`"));
        }
    }
}

fn forbid_all(path: &str, text: &str, markers: &[&str], errors: &mut Vec<String>) {
    for marker in markers {
        if text.contains(marker) {
            errors.push(format!("{path} must not contain forbidden marker `{marker}`"));
        }
    }
}

fn hash_artifact(path: &Path) -> Result<serde_json::Value, String> {
    let bytes = fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    Ok(json!({
        "path": path.display().to_string(),
        "blake3": format!("blake3:{}", blake3::hash(&bytes).to_hex()),
        "bytes": bytes.len(),
    }))
}
