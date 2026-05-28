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
const PROVIDER_BRIDGE_SOURCE: &str = "crates/clankers-provider/src/router_request_bridge.rs";
const RECEIPT_PATH: &str = "target/embedded-sdk-release/runtime-extension-service-matrix-receipt.json";
const TEST_FILTER: &str = "runtime_extension_service_matrix_";
const PROVIDER_CONTRACT_FILTER: &str = "provider_model_contract_literal";
const DESKTOP_PROVIDER_FILTER: &str = "runtime_services::tests::desktop_runtime_provider_router";
const PROVIDER_BRIDGE_FILTER: &str = "router_request_bridge";

const REQUIRED_MARKERS: &[&str] = &[
    "runtime_extension_service_matrix_default_safe_fails_closed_independently",
    "runtime_extension_service_matrix_mixed_injected_absent_no_ambient_fallback",
    "runtime_extension_service_matrix_injected_error_receipts_are_redacted",
    "runtime_extension_service_matrix_safe_receipts_redact_success_denial_and_error",
    "desktop_runtime_mixed_injected_services_do_not_fall_back_to_ambient",
    "desktop_runtime_provider_router_projects_retryable_and_terminal_failures",
    "desktop_runtime_provider_router_preserves_codex_and_openai_model_prefixes",
    "provider_model_contract_literal_fixtures_cover_request_stream_failures_and_usage",
    "completion_request_from_bridge_input",
    "ProviderModelRequest",
    "ProviderModelResponse",
    "ProviderStreamEvent",
    "ProviderModelStatus::RetryableFailure",
    "ProviderModelStatus::TerminalFailure",
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
    let provider_bridge = fs::read_to_string(PROVIDER_BRIDGE_SOURCE)
        .map_err(|error| format!("read {PROVIDER_BRIDGE_SOURCE}: {error}"))?;
    let joined = format!("{runtime}\n{desktop}\n{provider_bridge}");
    let missing: Vec<_> = REQUIRED_MARKERS.iter().copied().filter(|needle| !joined.contains(needle)).collect();
    if !missing.is_empty() {
        let mut message = String::from("runtime extension service matrix freshness failed:");
        for item in missing {
            message.push_str(&format!("\n  - missing {item}"));
        }
        return Err(message);
    }

    run_cargo_test(["test", "-p", "clankers-runtime", "--lib", TEST_FILTER], "runtime extension matrix tests")?;
    run_cargo_test(
        ["test", "-p", "clankers-runtime", "--lib", PROVIDER_CONTRACT_FILTER],
        "provider contract fixture tests",
    )?;
    run_cargo_test(["test", "-p", "clankers", "--lib", DESKTOP_PROVIDER_FILTER], "desktop provider adapter tests")?;
    run_cargo_test(
        ["test", "-p", "clankers-provider", "--lib", PROVIDER_BRIDGE_FILTER],
        "provider bridge ownership tests",
    )?;

    write_receipt()
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
        "schema": "clankers.embedded_lego.behavioral_receipt.v1",
        "case_id": "runtime-extension-service-matrix",
        "axes": {
            "services": ["provider_router", "auth_store", "credential_pool", "runtime"],
            "states": ["disabled", "injected", "succeeded", "failed", "unavailable"],
            "provider_model_status": ["completed", "retryable_failure", "terminal_failure", "cancelled"]
        },
        "expected_outcome": "runtime extension services fail closed independently; provider model services expose neutral request/stream/failure DTOs and desktop adapters preserve provider/router policy ownership",
        "observed_outcome": "passed",
        "source_artifacts": [RUNTIME_SOURCE, DESKTOP_SOURCE, PROVIDER_BRIDGE_SOURCE, "scripts/check-runtime-extension-service-matrix.rs"],
        "sanitized_hashes": {
            RUNTIME_SOURCE: hash_file(RUNTIME_SOURCE)?,
            DESKTOP_SOURCE: hash_file(DESKTOP_SOURCE)?,
            PROVIDER_BRIDGE_SOURCE: hash_file(PROVIDER_BRIDGE_SOURCE)?,
        },
        "owner": "embedded-sdk",
        "requirement_ids": [
            "behavioral-lego-parity-rails.conversion.runtime-shell-matrices",
            "provider-router-runtime-services.model-contract",
            "provider-router-runtime-services.desktop-adapter",
            "provider-router-runtime-services.verification"
        ],
        "cases": CASE_IDS,
    });
    let bytes =
        serde_json::to_vec_pretty(&receipt).map_err(|error| format!("failed to encode {RECEIPT_PATH}: {error}"))?;
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
