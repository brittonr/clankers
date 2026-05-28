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
const CONFIG_CORE: &str = "crates/clankers-config/src/core.rs";
const RUNTIME_PROMPT: &str = "crates/clankers-runtime/src/prompt.rs";
const RUNTIME_SERVICES: &str = "crates/clankers-runtime/src/services.rs";
const DESKTOP_SERVICES: &str = "src/runtime_services.rs";
const RECEIPT_PATH: &str = "target/embedded-sdk-release/config-prompt-skill-services-receipt.json";

const CONFIG_FORBIDDEN: &[&str] = &[
    "ratatui::",
    "clankers_tui::",
    "clanker_tui_types::",
    "clanker_router::",
    "clankers_ucan::",
    "MenuContributor",
];

const REQUIRED_MARKERS: &[(&str, &str)] = &[
    (CONFIG_CORE, "NeutralSettingsSummary"),
    (CONFIG_CORE, "NeutralKeymapConfig"),
    (CONFIG_CORE, "PromptServiceConfig"),
    (RUNTIME_PROMPT, "pub trait PromptSourceService"),
    (RUNTIME_PROMPT, "SkillSnippet"),
    (RUNTIME_PROMPT, "PromptSourceKind::Skill"),
    (RUNTIME_SERVICES, "pub trait SkillStore"),
    (RUNTIME_SERVICES, "SkillResolutionRequest"),
    (DESKTOP_SERVICES, "desktop_runtime_skill_service_resolves_explicit_roots_without_content_leaks"),
    (DESKTOP_SERVICES, "clankers_skills::discover_skills"),
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("config-prompt-skill-services receipt written to {RECEIPT_PATH}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("config-prompt-skill-services check failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), String> {
    ensure_tmpdir()?;
    check_required_markers()?;
    check_config_core_forbidden_tokens()?;
    run_cargo_test(
        [
            "test",
            "-p",
            "clankers-config",
            "--lib",
            "config_core_services_are_display_neutral",
        ],
        "config neutral core test",
    )?;
    run_cargo_test(
        ["test", "-p", "clankers-runtime", "--lib", "config_prompt_skill_service"],
        "runtime prompt/skill service fixtures",
    )?;
    run_cargo_test(
        [
            "test",
            "-p",
            "clankers-runtime",
            "--lib",
            "prompt_source_service_injection",
        ],
        "prompt source service injection fixture",
    )?;
    run_cargo_test(
        [
            "test",
            "-p",
            "clankers",
            "--lib",
            "runtime_services::tests::desktop_runtime_skill_service_resolves_explicit_roots_without_content_leaks",
        ],
        "desktop skill service fixture",
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

fn check_config_core_forbidden_tokens() -> Result<(), String> {
    let text = fs::read_to_string(CONFIG_CORE).map_err(|error| format!("failed to read {CONFIG_CORE}: {error}"))?;
    for token in CONFIG_FORBIDDEN {
        if text.contains(token) {
            return Err(format!("display-neutral config core contains forbidden token `{token}`"));
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
        "schema": "clankers.config_prompt_skill_services.receipt.v1",
        "observed_outcome": "passed",
        "expected_outcome": "config core stays display-neutral; prompt and skill sources are explicit services with safe embedded defaults and desktop adapter parity",
        "source_artifacts": [CONFIG_CORE, RUNTIME_PROMPT, RUNTIME_SERVICES, DESKTOP_SERVICES, "scripts/check-config-prompt-skill-services.rs"],
        "sanitized_hashes": {
            CONFIG_CORE: hash_file(CONFIG_CORE)?,
            RUNTIME_PROMPT: hash_file(RUNTIME_PROMPT)?,
            RUNTIME_SERVICES: hash_file(RUNTIME_SERVICES)?,
            DESKTOP_SERVICES: hash_file(DESKTOP_SERVICES)?,
        },
        "requirement_ids": [
            "config-prompt-skill-services.config-core",
            "config-prompt-skill-services.prompt-service",
            "config-prompt-skill-services.skill-service",
            "config-prompt-skill-services.desktop-adapter",
            "config-prompt-skill-services.verification"
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
