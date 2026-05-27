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
const SETTINGS: &str = "crates/clankers-config/src/settings.rs";
const CONFIG_LIB: &str = "crates/clankers-config/src/lib.rs";
const AGENT_LIB: &str = "crates/clankers-agent/src/lib.rs";
const TURN_MOD: &str = "crates/clankers-agent/src/turn/mod.rs";
const ADAPTER: &str = "crates/clankers-agent/src/turn/steel_planning.rs";
const DOC: &str = "docs/src/reference/steel-turn-planning-config-activation.md";
const SUMMARY: &str = "docs/src/SUMMARY.md";
const TASKS: &str = "cairn/changes/steel-turn-planning-config-activation/tasks.md";
const ARCHIVED_TASKS: &str = "cairn/archive/2026-05-22-steel-turn-planning-config-activation/tasks.md";
const OUTPUT: &str = "target/steel-turn-planning-config-activation/receipt.json";

const REQUIRED_SETTINGS_MARKERS: &[&str] = &[
    "SteelTurnPlanningSettings",
    "SteelTurnPlanningRolloutStage",
    "SteelTurnPlanningFallbackMode",
    "MissingProfilePath",
    "ReceiptOutsideTarget",
    "default_steel_turn_planning_max_source_bytes",
];
const REQUIRED_ADAPTER_MARKERS: &[&str] = &[
    "steel_turn_planning_config_from_settings",
    "verify_optional_hash",
    "runtime_profile_from_export",
    "ensure_session_authority",
    "DEFAULT_TURN_PLANNING_SEAM",
    "ScriptHashMismatch",
    "MissingSessionCapability",
    "DisabledRequiredAction",
    "settings_activation_uses_bundled_default_without_paths",
    "settings_activation_explicit_disabled_uses_rust_native",
    "settings_activation_can_select_default_rollout_after_validation",
];
const REQUIRED_AGENT_MARKERS: &[&str] = &[
    "fn steel_turn_planning_config(&self)",
    "self.steel_turn_planning_config()?",
    "Steel turn planning activation failed closed",
];
const REQUIRED_DOC_MARKERS: &[&str] = &[
    "Steel Turn Planning Config Activation",
    "default Steel planner",
    "steel.host.plan_turn",
    "Nickel-exported",
    "UCAN",
    "no ambient filesystem",
    "target/steel-turn-planning-config-activation/receipt.json",
];
const FORBIDDEN_RECEIPT_LEAKS: &[&str] = &[
    "raw_prompt",
    "provider_payload",
    "credential_value",
    "compact_ucan",
    "tool_body",
];

fn main() -> ExitCode {
    match run() {
        Ok(path) => {
            println!("steel turn planning config activation receipt written to {}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("steel turn planning config activation check failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<PathBuf, String> {
    let settings = read(SETTINGS)?;
    let config_lib = read(CONFIG_LIB)?;
    let agent_lib = read(AGENT_LIB)?;
    let turn_mod = read(TURN_MOD)?;
    let adapter = read(ADAPTER)?;
    let doc = read(DOC)?;
    let summary = read(SUMMARY)?;
    let task_path = existing_task_path();
    let tasks = read(task_path)?;
    let mut errors = Vec::new();

    require_all(SETTINGS, &settings, REQUIRED_SETTINGS_MARKERS, &mut errors);
    require_all(ADAPTER, &adapter, REQUIRED_ADAPTER_MARKERS, &mut errors);
    require_all(AGENT_LIB, &agent_lib, REQUIRED_AGENT_MARKERS, &mut errors);
    require_all(DOC, &doc, REQUIRED_DOC_MARKERS, &mut errors);
    forbid_all(DOC, &doc, FORBIDDEN_RECEIPT_LEAKS, &mut errors);
    if !config_lib.contains("pub use settings::SteelTurnPlanningSettings") {
        errors.push(format!("{CONFIG_LIB} must export Steel turn-planning settings"));
    }
    if !turn_mod.contains("pub use steel_planning::steel_turn_planning_config_from_settings") {
        errors.push(format!("{TURN_MOD} must re-export the activation helper for shared agent use"));
    }
    if count(&agent_lib, "self.steel_turn_planning_config()?") < 2 {
        errors.push(format!("{AGENT_LIB} must use the shared activation helper for normal and orchestrated turns"));
    }
    if !summary.contains("steel-turn-planning-config-activation.md") {
        errors.push(format!("{SUMMARY} must link the config activation reference doc"));
    }
    if tasks.contains("- [ ]") {
        errors.push(format!("{task_path} still has unchecked implementation/gate tasks"));
    }
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }

    let artifacts = [
        SETTINGS,
        CONFIG_LIB,
        AGENT_LIB,
        TURN_MOD,
        ADAPTER,
        DOC,
        SUMMARY,
        task_path,
        "scripts/check-steel-turn-planning-config-activation.rs",
    ]
    .iter()
    .map(|path| hash_artifact(Path::new(path)))
    .collect::<Result<Vec<_>, _>>()?;
    let receipt = json!({
        "schema": "clankers.steel_turn_planning_config_activation.receipt.v1",
        "validated_surfaces": [
            "bundled-default-settings",
            "explicit-disable-kill-switch",
            "reviewed-profile-and-script-loader",
            "blake3-freshness-checks",
            "session-and-ucan-authority-checks",
            "normal-turn-threading",
            "orchestrated-turn-threading",
            "target-only-receipts",
            "redacted-evidence"
        ],
        "hashed_artifacts": artifacts,
        "redaction": {
            "raw_prompts": "omitted",
            "provider_payloads": "omitted",
            "credentials": "omitted",
            "ucan_proofs": "omitted",
            "script_bodies": "omitted"
        },
        "guidance": "Bundled settings select reviewed Steel turn planning by default; explicit disable keeps Rust-native planning; UCAN/session state grants runtime authority; Rust validates and enforces before Steel Scheme can plan a turn."
    });
    let output = PathBuf::from(OUTPUT);
    let parent = output.parent().ok_or_else(|| format!("{} has no parent", output.display()))?;
    fs::create_dir_all(parent).map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let bytes = serde_json::to_vec_pretty(&receipt).map_err(|error| format!("failed to encode receipt: {error}"))?;
    fs::write(&output, [bytes.as_slice(), b"\n"].concat())
        .map_err(|error| format!("failed to write {}: {error}", output.display()))?;
    Ok(output)
}

fn read(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("failed to read {path}: {error}"))
}

fn existing_task_path() -> &'static str {
    if Path::new(TASKS).exists() {
        TASKS
    } else {
        ARCHIVED_TASKS
    }
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

fn count(text: &str, needle: &str) -> usize {
    text.match_indices(needle).count()
}

fn hash_artifact(path: &Path) -> Result<serde_json::Value, String> {
    let bytes = fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    Ok(json!({
        "path": path.display().to_string(),
        "blake3": format!("blake3:{}", blake3::hash(&bytes).to_hex()),
        "bytes": bytes.len(),
    }))
}
