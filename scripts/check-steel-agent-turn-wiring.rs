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
const AGENT_TURN: &str = "crates/clankers-agent/src/turn/mod.rs";
const ADAPTER: &str = "crates/clankers-agent/src/turn/steel_planning.rs";
const STEEL_EXECUTION: &str = "crates/clankers-agent/src/turn/steel_execution.rs";
const AGENT_CARGO: &str = "crates/clankers-agent/Cargo.toml";
const DOC: &str = "docs/src/reference/steel-agent-turn-wiring.md";
const SUMMARY: &str = "docs/src/SUMMARY.md";
const OUTPUT: &str = "target/steel-agent-turn-wiring/receipt.json";

const REQUIRED_ADAPTER_MARKERS: &[&str] = &[
    "AgentTurnSteelPlanningConfig",
    "TurnPlanningInput",
    "plan_turn_with_steel_or_fallback",
    "DEFAULT_TURN_PLANNING_SEAM",
    "AgentTurnExecutionPlanner::Blocked",
    "prompt_hash",
    "tool_names",
    "steel.host.plan_turn",
];
const REQUIRED_TURN_MARKERS: &[&str] = &[
    "steel_turn_planning",
    "plan_agent_turn",
    "emit_agent_turn_planning_receipt",
    "steel.host.plan_turn blocked agent turn before provider request",
    "run_steel_selected_engine_turn",
    "run_turn_loop_emits_steel_plan_turn_receipt_when_configured",
    "run_turn_loop_uses_steel_selected_executor_when_default_planner_authorizes",
];
const REQUIRED_EXECUTION_MARKERS: &[&str] = &[
    "Steel-selected turn execution adapter",
    "run_engine_turn(seed, hosts).await",
    "HostAdapters",
];
const REQUIRED_DOC_MARKERS: &[&str] = &[
    "Steel Agent Turn Wiring",
    "Comparison",
    "Default",
    "Blocked",
    "no ambient filesystem",
    "Rust-owned host functions",
];
const FORBIDDEN_AGENT_IMPORTS: &[&str] = &["steel_core::", "steel::steel_vm", "steel_vm::"];
const FORBIDDEN_RECEIPT_LEAKS: &[&str] = &[
    "raw_prompt",
    "provider_payload",
    "compact_ucan",
    "script_source",
    "tool_body",
];

fn main() -> ExitCode {
    match run() {
        Ok(path) => {
            println!("steel agent turn wiring receipt written to {}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("steel agent turn wiring check failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<PathBuf, String> {
    let turn = read(AGENT_TURN)?;
    let adapter = read(ADAPTER)?;
    let execution = read(STEEL_EXECUTION)?;
    let cargo = read(AGENT_CARGO)?;
    let doc = read(DOC)?;
    let summary = read(SUMMARY)?;
    let mut errors = Vec::new();

    require_all(ADAPTER, &adapter, REQUIRED_ADAPTER_MARKERS, &mut errors);
    require_all(AGENT_TURN, &turn, REQUIRED_TURN_MARKERS, &mut errors);
    require_all(STEEL_EXECUTION, &execution, REQUIRED_EXECUTION_MARKERS, &mut errors);
    require_all(DOC, &doc, REQUIRED_DOC_MARKERS, &mut errors);
    forbid_all(AGENT_TURN, &turn, FORBIDDEN_AGENT_IMPORTS, &mut errors);
    forbid_all(ADAPTER, &adapter, FORBIDDEN_AGENT_IMPORTS, &mut errors);
    forbid_all(STEEL_EXECUTION, &execution, FORBIDDEN_AGENT_IMPORTS, &mut errors);
    forbid_all(DOC, &doc, FORBIDDEN_RECEIPT_LEAKS, &mut errors);
    if !cargo.contains("clankers-runtime") || !cargo.contains("clankers-artifacts") {
        errors.push(format!("{AGENT_CARGO} must depend on runtime/artifact DTO crates for the adapter"));
    }
    if !summary.contains("steel-agent-turn-wiring.md") {
        errors.push(format!("{SUMMARY} must link the Steel agent turn wiring doc"));
    }
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }

    let artifacts = [
        AGENT_TURN,
        ADAPTER,
        STEEL_EXECUTION,
        AGENT_CARGO,
        DOC,
        SUMMARY,
        "scripts/check-steel-agent-turn-wiring.rs",
    ]
    .iter()
    .map(|path| hash_artifact(Path::new(path)))
    .collect::<Result<Vec<_>, _>>()?;
    let receipt = json!({
        "schema": "clankers.steel_agent_turn_wiring.receipt.v1",
        "validated_surfaces": [
            "real-run-turn-loop-call-site",
            "rust-owned-adapter",
            "runtime-orchestration-delegation",
            "blocked-before-provider-request",
            "steel-selected-execution-adapter",
            "redacted-docs-and-receipts",
            "no-direct-steel-interpreter-import"
        ],
        "hashed_artifacts": artifacts,
        "guidance": "Steel Scheme selects authorized default turn execution through typed Rust seams; Rust retains provider/tool effect and fallback authority."
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
