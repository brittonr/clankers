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
const CONTROLLER_RUNTIME_ADAPTER: &str = "crates/clankers-controller/src/runtime_adapter.rs";
const CONTROLLER_COMMAND: &str = "crates/clankers-controller/src/command.rs";
const LEGO_RAIL: &str = "scripts/check-lego-architecture-boundaries.rs";
const FCIS_RAIL: &str = "crates/clankers-controller/tests/fcis_shell_boundaries.rs";
const BASELINE: &str = "policy/lego-architecture/dependency-ownership-baseline.json";
const RECEIPT_PATH: &str = "target/embedded-sdk-release/root-controller-runtime-adapters-receipt.json";

const REQUIRED_MARKERS: &[(&str, &str)] = &[
    (CONTROLLER_RUNTIME_ADAPTER, "pub trait ControllerRuntimeAdapter"),
    (CONTROLLER_RUNTIME_ADAPTER, "RuntimePromptRequest"),
    (CONTROLLER_RUNTIME_ADAPTER, "RuntimeControlRequest"),
    (CONTROLLER_RUNTIME_ADAPTER, "FakeRuntimeAdapter"),
    (CONTROLLER_COMMAND, "submit_prompt_with_runtime_adapter"),
    (CONTROLLER_COMMAND, "apply_control_with_runtime_adapter"),
    (CONTROLLER_COMMAND, "runtime_adapter_fixture_covers_prompt_control_identity_and_semantic_projection"),
    (LEGO_RAIL, "dependency_owner_receipts"),
    (BASELINE, "owner_receipts"),
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("root-controller-runtime-adapters receipt written to {RECEIPT_PATH}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("root-controller-runtime-adapters check failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), String> {
    ensure_tmpdir()?;
    check_required_markers()?;
    run_cargo_test(
        ["test", "-p", "clankers-controller", "--lib", "runtime_adapter"],
        "runtime adapter contract fixture",
    )?;
    run_cargo_test(
        [
            "test",
            "-p",
            "clankers-controller",
            "--lib",
            "runtime_adapter_fixture_covers_prompt_control_identity_and_semantic_projection",
        ],
        "controller fake-service command lifecycle fixture",
    )?;
    run_cargo_test(
        ["test", "-p", "clankers-controller", "--test", "fcis_shell_boundaries"],
        "controller FCIS boundary rail",
    )?;
    run_script(LEGO_RAIL)?;
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

fn run_script(path: &str) -> Result<(), String> {
    let mut command = Command::new(path);
    command.env("RUSTC_WRAPPER", "");
    let status = command.status().map_err(|error| format!("failed to run {path}: {error}"))?;
    if !status.success() {
        return Err(format!("{path} failed with status {status}"));
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
        "schema": "clankers.root_controller_runtime_adapters.receipt.v1",
        "observed_outcome": "passed",
        "expected_outcome": "root/controller dependency budgets carry owner receipts, and controller prompt/control lifecycle runs through fake runtime services without sockets, TUI, providers, or desktop storage",
        "source_artifacts": [CONTROLLER_RUNTIME_ADAPTER, CONTROLLER_COMMAND, LEGO_RAIL, FCIS_RAIL, BASELINE, "scripts/check-root-controller-runtime-adapters.rs"],
        "sanitized_hashes": {
            CONTROLLER_RUNTIME_ADAPTER: hash_file(CONTROLLER_RUNTIME_ADAPTER)?,
            CONTROLLER_COMMAND: hash_file(CONTROLLER_COMMAND)?,
            LEGO_RAIL: hash_file(LEGO_RAIL)?,
            BASELINE: hash_file(BASELINE)?,
        },
        "requirement_ids": [
            "root-controller-runtime-adapters.root-shell",
            "root-controller-runtime-adapters.controller-shell",
            "root-controller-runtime-adapters.dependency-budget",
            "root-controller-runtime-adapters.verification"
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
