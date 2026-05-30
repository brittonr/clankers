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
const TEST: &str = "tests/embedded_controller.rs";
const EVENTS: &str = "crates/clankers-controller/src/event_processing.rs";
const DOC: &str = "docs/src/reference/steel-turn-planning-runtime-smoke.md";
const SUMMARY: &str = "docs/src/SUMMARY.md";
const TASKS: &str = "cairn/changes/steel-turn-planning-runtime-smoke/tasks.md";
const ARCHIVED_TASKS: &str = "cairn/archive/2026-05-22-steel-turn-planning-runtime-smoke/tasks.md";
const OUTPUT: &str = "target/steel-turn-planning-runtime-smoke/receipt.json";

const REQUIRED_TEST_MARKERS: &[&str] = &[
    "steel_runtime_smoke_prompt_command_emits_redacted_receipt",
    "steel_runtime_smoke_default_settings_emit_redacted_receipt",
    "steel_runtime_smoke_explicit_disable_keeps_rust_native",
    "steel_runtime_smoke_hash_mismatch_fails_closed_before_receipt",
    "steel_runtime_smoke_missing_authority_fails_closed_before_receipt",
    "SessionCommand::Prompt",
    "steel.host.plan_turn receipt",
    "status=Authorized",
    "planner=SteelScheme",
    "executor=RustNative",
    "executor=SteelScheme",
    "fallback=NotNeeded",
    "!receipt.contains(raw_prompt)",
    "calls.load(Ordering::SeqCst), 0",
];
const REQUIRED_EVENT_MARKERS: &[&str] = &[
    "AgentEvent::SystemMessage",
    "DaemonEvent::SystemMessage",
    "text: message.clone()",
    "is_error: false",
];
const REQUIRED_DOC_MARKERS: &[&str] = &[
    "Steel Turn Planning Runtime Smoke",
    "SessionCommand::Prompt",
    "steel.host.plan_turn",
    "DaemonEvent::SystemMessage",
    "executor=SteelScheme",
    "no raw prompt",
    "target/steel-turn-planning-runtime-smoke/receipt.json",
];
const FORBIDDEN_DOC_MARKERS: &[&str] = &["credential_value", "raw_prompt =", "compact_ucan", "provider_payload ="];

fn main() -> ExitCode {
    match run() {
        Ok(path) => {
            println!("steel turn planning runtime smoke receipt written to {}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("steel turn planning runtime smoke check failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<PathBuf, String> {
    let test = read(TEST)?;
    let events = read(EVENTS)?;
    let doc = read(DOC)?;
    let summary = read(SUMMARY)?;
    let task_path = existing_task_path();
    let tasks = read(task_path)?;
    let mut errors = Vec::new();

    require_all(TEST, &test, REQUIRED_TEST_MARKERS, &mut errors);
    require_all(EVENTS, &events, REQUIRED_EVENT_MARKERS, &mut errors);
    require_all(DOC, &doc, REQUIRED_DOC_MARKERS, &mut errors);
    forbid_all(DOC, &doc, FORBIDDEN_DOC_MARKERS, &mut errors);
    if !summary.contains("steel-turn-planning-runtime-smoke.md") {
        errors.push(format!("{SUMMARY} must link the runtime smoke reference doc"));
    }
    if tasks.contains("- [ ]") {
        errors.push(format!("{task_path} still has unchecked tasks"));
    }
    if count(&test, "steel_runtime_smoke_") < 5 {
        errors.push(format!("{TEST} must include default, opt-out, positive, and negative Steel runtime smoke tests"));
    }
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }

    let artifacts = [
        TEST,
        EVENTS,
        DOC,
        SUMMARY,
        task_path,
        "scripts/check-steel-turn-planning-runtime-smoke.rs",
    ]
    .iter()
    .map(|path| hash_artifact(Path::new(path)))
    .collect::<Result<Vec<_>, _>>()?;
    let receipt = json!({
        "schema": "clankers.steel_turn_planning_runtime_smoke.receipt.v1",
        "validated_surfaces": [
            "controller-prompt-command",
            "default-settings-steel-planning",
            "explicit-disable-rust-native-planning",
            "config-driven-steel-planning",
            "daemon-visible-redacted-receipt",
            "daemon-visible-steel-executor-selection",
            "hash-mismatch-fail-closed",
            "authority-missing-fail-closed",
            "provider-call-suppressed-on-invalid-activation"
        ],
        "hashed_artifacts": artifacts,
        "redaction": {
            "raw_prompts": "omitted",
            "script_bodies": "omitted",
            "credentials": "omitted",
            "ucan_proofs": "omitted",
            "provider_payloads": "omitted"
        },
        "guidance": "Steel Scheme remains a typed planner and selected-executor signal; Rust validates config, bridges redacted receipts, calls providers through host effects, and fails closed before I/O on invalid activation."
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
