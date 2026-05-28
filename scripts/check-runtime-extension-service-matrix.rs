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
const RUNTIME_SOURCE: &str = "crates/clankers-runtime/src/lib.rs";
const DESKTOP_SOURCE: &str = "src/runtime_services.rs";
const RECEIPT_PATH: &str = "target/embedded-sdk-release/runtime-extension-service-matrix-receipt.json";
const TEST_FILTER: &str = "runtime_extension_service_matrix_";

const REQUIRED_MARKERS: &[&str] = &[
    "runtime_extension_service_matrix_default_safe_fails_closed_independently",
    "runtime_extension_service_matrix_mixed_injected_absent_no_ambient_fallback",
    "runtime_extension_service_matrix_injected_error_receipts_are_redacted",
    "runtime_extension_service_matrix_safe_receipts_redact_success_denial_and_error",
    "desktop_runtime_mixed_injected_services_do_not_fall_back_to_ambient",
    "provider_router",
    "auth_store",
    "credential_pool",
    "runtime",
    "ExtensionRuntimeKind::Plugin",
    "ExtensionRuntimeKind::Mcp",
    "ExtensionRuntimeKind::Gateway",
    "disabled",
    "injected",
    "ExtensionStatus::Succeeded",
    "ExtensionStatus::Failed",
    "ExtensionStatus::Unavailable",
    "contains_secret_markers",
    "serde_json::to_string",
    "execute_calls",
    "publish_calls",
];

const CASE_IDS: &[&str] = &[
    "runtime-extension-service.default-safe-fail-closed",
    "runtime-extension-service.mixed-injected-no-ambient-fallback",
    "runtime-extension-service.injected-error-receipts-redacted",
    "runtime-extension-service.success-denial-error-safe-receipts",
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("runtime extension service matrix receipt written to {RECEIPT_PATH}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("runtime extension service matrix check failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), String> {
    ensure_tmpdir()?;
    let runtime = fs::read_to_string(RUNTIME_SOURCE).map_err(|error| format!("read {RUNTIME_SOURCE}: {error}"))?;
    let desktop = fs::read_to_string(DESKTOP_SOURCE).map_err(|error| format!("read {DESKTOP_SOURCE}: {error}"))?;
    let joined = format!("{runtime}\n{desktop}");
    let missing: Vec<_> = REQUIRED_MARKERS.iter().copied().filter(|needle| !joined.contains(needle)).collect();
    if !missing.is_empty() {
        let mut message = String::from("runtime extension service matrix freshness failed:");
        for item in missing {
            message.push_str(&format!("\n  - missing {item}"));
        }
        return Err(message);
    }

    let mut command = Command::new("cargo");
    command.env("RUSTC_WRAPPER", "");
    command.args(["test", "-p", "clankers-runtime", "--lib", TEST_FILTER]);
    let status = command
        .status()
        .map_err(|error| format!("failed to run runtime extension matrix tests: {error}"))?;
    if !status.success() {
        return Err(format!("runtime extension matrix tests failed with status {status}"));
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
        "case_id": "runtime-extension-service-matrix",
        "axes": {
            "services": ["provider_router", "auth_store", "credential_pool", "runtime"],
            "states": ["disabled", "injected", "succeeded", "failed", "unavailable"]
        },
        "expected_outcome": "runtime extension services fail closed independently and never fall back to ambient desktop services",
        "observed_outcome": "passed",
        "source_artifacts": [RUNTIME_SOURCE, DESKTOP_SOURCE, "scripts/check-runtime-extension-service-matrix.rs"],
        "sanitized_hashes": {
            RUNTIME_SOURCE: hash_file(RUNTIME_SOURCE)?,
            DESKTOP_SOURCE: hash_file(DESKTOP_SOURCE)?,
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
