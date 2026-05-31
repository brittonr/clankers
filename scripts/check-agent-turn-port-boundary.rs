#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;
use std::process::ExitCode;

const ERROR_EXIT: u8 = 1;
const PORTS: &str = "crates/clankers-agent/src/turn/ports.rs";
const TURN: &str = "crates/clankers-agent/src/turn/mod.rs";
const EXECUTION: &str = "crates/clankers-agent/src/turn/execution.rs";

const REQUIRED_PORT_MARKERS: &[&str] = &[
    "trait AgentModelPort",
    "trait AgentToolPort",
    "trait AgentCostPort",
    "trait AgentCancellationPort",
    "struct AgentRuntimeServices",
    "struct ProviderModelPort",
    "struct ControllerToolPort",
    "struct CostTrackerPort",
    "struct TokenCancellationPort",
    "DESKTOP_AGENT_SERVICE_RECEIPTS",
    "AgentRuntimeServiceKind::ModelExecution",
    "AgentRuntimeServiceKind::ToolRegistry",
    "AgentRuntimeServiceKind::Storage",
    "AgentRuntimeServiceKind::PromptContext",
    "AgentRuntimeServiceKind::Hooks",
    "AgentRuntimeServiceKind::Skills",
    "AgentRuntimeServiceKind::Cost",
    "AgentRuntimeServiceKind::Cancellation",
];

const TURN_REQUIRED_MARKERS: &[&str] = &[
    "pub(crate) struct TurnLoopContext<'a>",
    "pub(crate) services: AgentRuntimeServices<'a>",
    "pub(crate) async fn run_turn_loop",
    "services.model",
    "services.tools",
    "services.cost",
    "services.cancellation",
    "fake_runtime_service_bundle_turn_runs_without_desktop_systems",
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("ok: agent turn port boundary rail passed");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("agent turn port boundary rail failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), String> {
    let ports = read(PORTS)?;
    for marker in REQUIRED_PORT_MARKERS {
        require_contains(&ports, marker, PORTS)?;
    }

    let turn = read(TURN)?;
    for marker in TURN_REQUIRED_MARKERS.iter().filter(|marker| !marker.contains("fake_runtime")) {
        require_contains(&turn, marker, TURN)?;
    }
    require_contains(&turn, "fake_runtime_service_bundle_turn_runs_without_desktop_systems", TURN)?;

    for forbidden in ["&dyn Provider", "HashMap<String, Arc<dyn Tool>>", "Option<clankers_db::Db>"] {
        if run_turn_loop_signature(&turn).contains(forbidden) {
            return Err(format!("{TURN} run_turn_loop signature still exposes concrete desktop dependency `{forbidden}`"));
        }
    }
    require_contains(&read(EXECUTION)?, "completion_request_from_engine_request", EXECUTION)?;

    Ok(())
}

fn run_turn_loop_signature(turn_runtime: &str) -> &str {
    let Some(start) = turn_runtime.find("pub(crate) async fn run_turn_loop") else {
        return "";
    };
    let tail = &turn_runtime[start..];
    let Some(end) = tail.find(" -> ") else {
        return tail;
    };
    &tail[..end]
}

fn read(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("failed to read {path}: {error}"))
}

fn require_contains(text: &str, marker: &str, path: &str) -> Result<(), String> {
    if text.contains(marker) {
        Ok(())
    } else {
        Err(format!("{path} missing required marker `{marker}`"))
    }
}
