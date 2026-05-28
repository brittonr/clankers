#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"

[dependencies]
blake3 = "1"
serde_json = "1"
---

use std::env;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::Command;
use std::process::ExitCode;

use serde_json::json;

const ERROR_EXIT: u8 = 1;
const SEMANTIC_EVENT: &str = "crates/clanker-message/src/semantic_event.rs";
const AGENT_EVENTS: &str = "crates/clankers-agent/src/events.rs";
const RUNTIME_EVENTS: &str = "crates/clankers-runtime/src/events.rs";
const CONTROLLER_DOMAIN: &str = "crates/clankers-controller/src/domain_event.rs";
const CONTROLLER_CONVERT: &str = "crates/clankers-controller/src/convert.rs";
const RECEIPT_PATH: &str = "target/embedded-sdk-release/semantic-event-stream-receipt.json";

const SEMANTIC_FORBIDDEN: &[&str] = &[
    "DaemonEvent",
    "TuiEvent",
    "clankers_protocol",
    "clanker_tui_types",
    "clankers_agent",
    "clankers_provider",
    "clanker_router",
    "CompletionRequest",
    "ProviderResponse",
];

const REQUIRED_MARKERS: &[(&str, &str)] = &[
    (SEMANTIC_EVENT, "pub enum SemanticEvent"),
    (SEMANTIC_EVENT, "PromptAccepted"),
    (SEMANTIC_EVENT, "AssistantDelta"),
    (SEMANTIC_EVENT, "ThinkingDelta"),
    (SEMANTIC_EVENT, "ToolStarted"),
    (SEMANTIC_EVENT, "ToolFinished"),
    (SEMANTIC_EVENT, "ConfirmationRequested"),
    (SEMANTIC_EVENT, "UsageUpdated"),
    (SEMANTIC_EVENT, "SemanticEventMetadata"),
    (SEMANTIC_EVENT, "semantic_event_metadata_redacts_secret_markers"),
    (AGENT_EVENTS, "pub fn to_semantic_event"),
    (RUNTIME_EVENTS, "pub fn to_semantic_event"),
    (CONTROLLER_DOMAIN, "pub(crate) type ControllerDomainEvent = SemanticEvent"),
    (CONTROLLER_CONVERT, "semantic_event_to_daemon_event"),
    (CONTROLLER_CONVERT, "semantic_event_to_tui_event"),
    (CONTROLLER_CONVERT, "semantic_event_to_json_value"),
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("semantic-event-stream receipt written to {RECEIPT_PATH}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("semantic-event-stream check failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), String> {
    ensure_tmpdir()?;
    check_required_markers()?;
    check_semantic_event_boundary()?;
    run_cargo_test(
        ["test", "-p", "clanker-message", "--lib", "semantic_event"],
        "semantic event contract fixtures",
    )?;
    run_cargo_test(
        [
            "test",
            "-p",
            "clankers-agent",
            "--lib",
            "agent_event_projects_core_semantic_order",
        ],
        "agent semantic projection fixture",
    )?;
    run_cargo_test(
        [
            "test",
            "-p",
            "clankers-runtime",
            "--lib",
            "runtime_events_project_to_shared_semantic_stream_in_order",
        ],
        "runtime semantic projection fixture",
    )?;
    run_cargo_test(
        [
            "test",
            "-p",
            "clankers-controller",
            "--lib",
            "semantic_event_projection_preserves_daemon_tui_and_json_shapes",
        ],
        "controller semantic edge projection fixture",
    )?;
    write_receipt()
}

fn check_required_markers() -> Result<(), String> {
    for (path, marker) in REQUIRED_MARKERS {
        let text = fs::read_to_string(path).map_err(|error| format!("failed to read {path}: {error}"))?;
        if !text.contains(marker) {
            return Err(format!("{path} missing required marker `{marker}`"));
        }
    }
    Ok(())
}

fn check_semantic_event_boundary() -> Result<(), String> {
    let text = fs::read_to_string(SEMANTIC_EVENT).map_err(|error| format!("failed to read {SEMANTIC_EVENT}: {error}"))?;
    for token in SEMANTIC_FORBIDDEN {
        if text.contains(token) {
            return Err(format!("semantic event contract contains forbidden token `{token}`"));
        }
    }
    Ok(())
}

fn run_cargo_test<const N: usize>(args: [&str; N], label: &str) -> Result<(), String> {
    let mut command = Command::new("cargo");
    command.env("RUSTC_WRAPPER", "");
    command.args(args);
    let status = command.status().map_err(|error| format!("failed to run {label}: {error}"))?;
    if !status.success() {
        return Err(format!("{label} failed with status {status}"));
    }
    Ok(())
}

fn ensure_tmpdir() -> Result<(), String> {
    if env::var_os("TMPDIR").is_some() {
        return Ok(());
    }
    let home = env::var("HOME").map_err(|error| format!("HOME is required when TMPDIR is unset: {error}"))?;
    let tmpdir = format!("{home}/.cargo-target/tmp");
    fs::create_dir_all(&tmpdir).map_err(|error| format!("failed to create {tmpdir}: {error}"))?;
    unsafe { env::set_var("TMPDIR", tmpdir) };
    Ok(())
}

fn write_receipt() -> Result<(), String> {
    if let Some(parent) = Path::new(RECEIPT_PATH).parent() {
        fs::create_dir_all(parent).map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let receipt = json!({
        "schema": "clankers.semantic_event_stream.receipt.v1",
        "observed_outcome": "passed",
        "expected_outcome": "semantic events cover prompt/model/tool/confirmation/usage/error/completion behavior; runtime and agent project into the shared stream; edge projections preserve covered daemon/TUI/JSON shapes with redacted metadata",
        "source_artifacts": [SEMANTIC_EVENT, AGENT_EVENTS, RUNTIME_EVENTS, CONTROLLER_DOMAIN, CONTROLLER_CONVERT, "scripts/check-semantic-event-stream.rs"],
        "sanitized_hashes": {
            SEMANTIC_EVENT: hash_file(SEMANTIC_EVENT)?,
            AGENT_EVENTS: hash_file(AGENT_EVENTS)?,
            RUNTIME_EVENTS: hash_file(RUNTIME_EVENTS)?,
            CONTROLLER_DOMAIN: hash_file(CONTROLLER_DOMAIN)?,
            CONTROLLER_CONVERT: hash_file(CONTROLLER_CONVERT)?,
        },
        "requirement_ids": [
            "semantic-event-stream.contract",
            "semantic-event-stream.inbound-projection",
            "semantic-event-stream.edge-projection",
            "semantic-event-stream.migration",
            "semantic-event-stream.verification"
        ]
    });
    let bytes = serde_json::to_vec_pretty(&receipt).map_err(|error| format!("failed to encode receipt: {error}"))?;
    fs::write(RECEIPT_PATH, [bytes.as_slice(), b"\n"].concat())
        .map_err(|error| format!("failed to write {RECEIPT_PATH}: {error}"))
}

fn hash_file(path: &str) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|error| format!("failed to open {path}: {error}"))?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 16 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|error| format!("failed to read {path}: {error}"))?;
        if read == 0 {
            return Ok(hasher.finalize().to_hex().to_string());
        }
        hasher.update(&buffer[..read]);
    }
}
