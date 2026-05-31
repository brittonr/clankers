#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;
use std::process::ExitCode;

const ERROR_EXIT: u8 = 1;
const BUILDER: &str = "src/modes/daemon/session_builder.rs";
const SOCKET_BRIDGE: &str = "src/modes/daemon/socket_bridge.rs";
const AGENT_PROCESS: &str = "src/modes/daemon/agent_process.rs";

const REQUIRED_BUILDER_MARKERS: &[&str] = &[
    "pub(crate) struct SessionBuilder",
    "pub(crate) struct CreateSessionPlanRequest",
    "pub(crate) struct AgentSpawnPlan",
    "pub(crate) struct SessionBuildPlan",
    "fn plan_create_session",
    "fn plan_new_keyed_session",
    "fn plan_recovered_keyed_session",
    "fn plan_recovered_catalog_session",
    "fn session_handle",
    "fn catalog_entry",
    "fn resolve_session_resume_in_dir",
];

const SOCKET_FORBIDDEN_RUNTIME_TOKENS: &[&str] = &[
    "fn resolve_session_resume",
    "SessionHandle {",
    "SessionCatalogEntry {",
    "clankers_session::SessionManager::open",
];

const AGENT_PROCESS_REQUIRED_MARKERS: &[&str] = &[
    "plan_new_keyed_session",
    "plan_recovered_keyed_session",
    "plan_recovered_catalog_session",
    "session_handle(cmd_tx.clone(), event_tx.clone())",
    "catalog_entry(spawned.automerge_path.clone().unwrap_or_default(), now)",
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("ok: daemon session-builder boundary rail passed");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("daemon session-builder boundary rail failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), String> {
    let builder = read(BUILDER)?;
    for marker in REQUIRED_BUILDER_MARKERS {
        require_contains(&builder, marker, BUILDER)?;
    }

    let socket_bridge = read(SOCKET_BRIDGE)?;
    let socket_runtime = socket_bridge.split("#[cfg(test)]\nmod tests").next().unwrap_or(&socket_bridge);
    require_contains(socket_runtime, "SessionBuilder::from_global_paths", SOCKET_BRIDGE)?;
    require_contains(socket_runtime, "plan_create_session", SOCKET_BRIDGE)?;
    require_contains(socket_runtime, "plan.session_handle", SOCKET_BRIDGE)?;
    require_contains(socket_runtime, "plan.catalog_entry", SOCKET_BRIDGE)?;
    for token in SOCKET_FORBIDDEN_RUNTIME_TOKENS {
        if socket_runtime.contains(token) {
            return Err(format!(
                "{SOCKET_BRIDGE} runtime code still owns session-building token `{token}`; move it to {BUILDER}"
            ));
        }
    }

    let agent_process = read(AGENT_PROCESS)?;
    let agent_runtime = agent_process.split("#[cfg(test)]").next().unwrap_or(&agent_process);
    for marker in AGENT_PROCESS_REQUIRED_MARKERS {
        require_contains(agent_runtime, marker, AGENT_PROCESS)?;
    }

    Ok(())
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
