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
const AGENT_SOURCE: &str = "crates/clankers-agent/src/turn/mod.rs";
const FCIS_SOURCE: &str = "crates/clankers-controller/tests/fcis_shell_boundaries.rs";
const RECEIPT_PATH: &str = "target/embedded-sdk-release/shell-adapter-parity-matrix-receipt.json";
const TEST_FILTER: &str = "shell_adapter_parity";

const REQUIRED_MARKERS: &[&str] = &[
    "ShellAdapterParityCase",
    "MatrixEntrypoint",
    "MatrixPromptSource",
    "MatrixStoreMode",
    "MatrixConfirmationOutcome",
    "MatrixDisabledToolPolicy",
    "MatrixToolResultClass",
    "MatrixModelResultClass",
    "MatrixEventTranslation",
    "shell_adapter_parity_matrix_names_required_axes",
    "standalone_agent_shell_adapter_parity_cases_preserve_engine_inputs_and_terminal_outcomes",
    "shell_adapter_parity_matrix_evidence_is_present_and_source_bounded",
    "StandaloneAgent",
    "ControllerDaemonAdapter",
    "EmbeddedBatchAdapter",
    "HostSupplied",
    "ResumeSeed",
    "DeniedByCapabilityGate",
    "MissingTool",
    "DaemonTranslated",
    "EmbeddedSemantic",
];

const CASE_IDS: &[&str] = &[
    "SAPM-001-standalone-host-prompt-stop",
    "SAPM-002-controller-resume-capability-denial",
    "SAPM-003-embedded-batch-user-filter-missing-tool",
    "SAPM-004-standalone-shell-assembled-approved-tool",
    "SAPM-005-controller-terminal-failure-event",
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("shell adapter parity matrix receipt written to {RECEIPT_PATH}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("shell adapter parity matrix check failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), String> {
    ensure_tmpdir()?;
    let agent = fs::read_to_string(AGENT_SOURCE).map_err(|error| format!("read {AGENT_SOURCE}: {error}"))?;
    let fcis = fs::read_to_string(FCIS_SOURCE).map_err(|error| format!("read {FCIS_SOURCE}: {error}"))?;
    let joined = format!("{agent}\n{fcis}");
    let missing: Vec<_> = REQUIRED_MARKERS.iter().copied().filter(|needle| !joined.contains(needle)).collect();
    if !missing.is_empty() {
        let mut message = String::from("shell adapter parity matrix freshness failed:");
        for item in missing {
            message.push_str(&format!("\n  - missing {item}"));
        }
        return Err(message);
    }

    let mut command = Command::new("cargo");
    command.env("RUSTC_WRAPPER", "");
    command.args(["test", "-p", "clankers-agent", "--lib", TEST_FILTER]);
    let status = command
        .status()
        .map_err(|error| format!("failed to run shell adapter parity tests: {error}"))?;
    if !status.success() {
        return Err(format!("shell adapter parity tests failed with status {status}"));
    }

    write_receipt()
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
        "schema": "clankers.embedded_lego.behavioral_receipt.v1",
        "case_id": "shell-adapter-parity-matrix",
        "axes": {
            "entrypoints": ["standalone_agent", "controller_daemon_adapter", "embedded_batch_adapter"],
            "prompt_sources": ["host_supplied", "resume_seed", "shell_assembled"],
            "event_translations": ["native_agent", "daemon_translated", "embedded_semantic"]
        },
        "expected_outcome": "shell adapter parity cases preserve engine inputs, terminal outcomes, and source-bounded evidence",
        "observed_outcome": "passed",
        "source_artifacts": [AGENT_SOURCE, FCIS_SOURCE, "scripts/check-shell-adapter-parity-matrix.rs"],
        "sanitized_hashes": {
            AGENT_SOURCE: hash_file(AGENT_SOURCE)?,
            FCIS_SOURCE: hash_file(FCIS_SOURCE)?,
        },
        "owner": "embedded-sdk",
        "requirement_ids": ["behavioral-lego-parity-rails.conversion.runtime-shell-matrices"],
        "cases": CASE_IDS,
    });
    let bytes = serde_json::to_vec_pretty(&receipt)
        .map_err(|error| format!("failed to encode {RECEIPT_PATH}: {error}"))?;
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
