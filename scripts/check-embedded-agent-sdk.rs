#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::env;
use std::path::Path;
use std::process::Command;
use std::process::ExitCode;

const ERROR_EXIT: u8 = 1;
const AGENT_TURN_TEST_FILTER: &str = "turn::tests::";

const RUST_CHECKS: &[&str] = &[
    "scripts/check-embedded-sdk-api.rs",
    "scripts/check-brick-inventory-stability.rs",
    "scripts/check-behavioral-lego-rails.rs",
    "scripts/check-embedded-lego-contracts.rs",
    "scripts/check-real-product-dogfood.rs",
    "scripts/check-provider-adapter-kit.rs",
    "scripts/check-session-resume-brick.rs",
    "scripts/check-tool-catalog-manifest.rs",
    "scripts/check-capability-pack-composition.rs",
    "scripts/check-plugin-runtime-dispatch.rs",
    "scripts/check-embedded-sdk-deps.rs",
    "scripts/check-embedded-adapters-deps.rs",
    "scripts/check-llm-contract-boundary.rs",
    "scripts/check-engine-host-feature-matrix.rs",
    "scripts/check-tool-catalog-matrix.rs",
    "scripts/check-runtime-extension-service-matrix.rs",
    "scripts/check-shell-adapter-parity-matrix.rs",
    "scripts/check-batch-eval-runner-kit.rs",
    "scripts/check-slash-command-routing-kit.rs",
    "scripts/check-tui-action-menu-kit.rs",
    "scripts/check-daemon-event-translation-kit.rs",
    "scripts/check-controller-continuation-policy-kit.rs",
    "scripts/check-observability-audit-receipt-kit.rs",
    "scripts/check-self-evolution-receipt-chain-kit.rs",
    "scripts/check-process-job-profile-kit.rs",
    "scripts/emit-embedded-sdk-release-receipt.rs",
];

const EXAMPLE_MANIFESTS: &[&str] = &[
    "examples/embedded-agent-sdk/Cargo.toml",
    "examples/embedded-minimal-kit/Cargo.toml",
    "examples/embedded-tool-kit/Cargo.toml",
    "examples/embedded-provider-adapter/Cargo.toml",
    "examples/embedded-session-store/Cargo.toml",
    "examples/embedded-product-workbench/Cargo.toml",
    "examples/prompt-assembly-kit/Cargo.toml",
    "examples/confirmation-broker-kit/Cargo.toml",
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("embedded-agent-sdk acceptance error: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), String> {
    ensure_repo_root()?;
    ensure_tmpdir()?;
    for script in RUST_CHECKS {
        run_step(script, [*script])?;
    }
    for manifest in EXAMPLE_MANIFESTS {
        run_cargo(["run", "--locked", "--manifest-path", manifest])?;
    }
    run_cargo(["test", "-p", "clankers-adapters", "--lib"])?;
    run_cargo(["test", "-p", "clankers-adapters", "--lib", "replaceable"])?;
    run_cargo(["test", "-p", "clankers-adapters", "--lib", "tool_catalog_metadata"])?;
    run_cargo(["test", "-p", "clankers-adapters", "--lib", "tool_catalog_validation"])?;
    run_cargo(["test", "-p", "clankers-adapters", "--lib", "capability_pack"])?;
    run_cargo(["test", "-p", "clankers-engine-host", "--lib"])?;
    run_cargo(["test", "-p", "clankers-agent", "--lib", AGENT_TURN_TEST_FILTER])?;
    run_cargo(["test", "-p", "clankers-runtime", "--lib", "tool_catalog_"])?;
    run_cargo([
        "test",
        "-p",
        "clankers-runtime",
        "--lib",
        "runtime_extension_service_matrix_",
    ])?;
    run_cargo(["test", "-p", "clankers-controller", "--test", "fcis_shell_boundaries"])?;
    println!("\nembedded-agent-sdk acceptance passed");
    Ok(())
}

fn ensure_repo_root() -> Result<(), String> {
    if !Path::new("Cargo.toml").is_file() || !Path::new("scripts").is_dir() {
        return Err("run from the Clankers repository root".to_string());
    }
    Ok(())
}

fn ensure_tmpdir() -> Result<(), String> {
    if env::var_os("TMPDIR").is_some() {
        return Ok(());
    }
    let home = env::var("HOME").map_err(|error| format!("HOME is required when TMPDIR is unset: {error}"))?;
    let tmpdir = format!("{home}/.cargo-target/tmp");
    std::fs::create_dir_all(&tmpdir).map_err(|error| format!("failed to create {tmpdir}: {error}"))?;
    unsafe { env::set_var("TMPDIR", tmpdir) };
    Ok(())
}

fn run_cargo<const N: usize>(args: [&str; N]) -> Result<(), String> {
    let mut command = Command::new("cargo");
    command.env("RUSTC_WRAPPER", "");
    command.args(args);
    run_command("cargo", &mut command)
}

fn run_step<const N: usize>(label: &str, args: [&str; N]) -> Result<(), String> {
    let mut command = Command::new(args[0]);
    command.args(&args[1..]);
    run_command(label, &mut command)
}

fn run_command(label: &str, command: &mut Command) -> Result<(), String> {
    println!("\n==> {label}");
    let status = command.status().map_err(|error| format!("failed to run {label}: {error}"))?;
    if !status.success() {
        return Err(format!("{label} failed with status {status}"));
    }
    Ok(())
}
