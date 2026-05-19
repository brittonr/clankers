#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::process::ExitCode;

const ERROR_EXIT: u8 = 1;
const CARGO_TREE_EDGES: &str = "normal";

const ENGINE_PACKAGE: &str = "clankers-engine";
const ENGINE_HOST_PACKAGE: &str = "clankers-engine-host";
const TOOL_HOST_PACKAGE: &str = "clankers-tool-host";
const MESSAGE_PACKAGE: &str = "clanker-message";

const ENGINE_FORBIDDEN_CRATES: &[&str] = &[
    "clankers-core",
    "clankers-provider",
    "clanker-router",
    "tokio",
    "reqwest",
    "redb",
    "iroh",
    "ratatui",
    "crossterm",
    "portable-pty",
    "clankers-agent",
];

const HOST_FORBIDDEN_DIRECT_CRATES: &[&str] = &[
    "clankers-agent",
    "clankers-core",
    "clankers-controller",
    "clankers-provider",
    "clanker-router",
    "clankers-db",
    "clankers-hooks",
    "clankers-plugin",
    "clankers-protocol",
    "clanker-tui-types",
    "clankers-tui",
    "ratatui",
    "crossterm",
    "portable-pty",
    "iroh",
    "redb",
    "reqwest",
    "hyper",
    "h2",
    "tower",
    "axum",
    "tokio",
    "async-std",
    "smol",
    "actix-rt",
    "reqwest-eventsource",
    "eventsource-stream",
    "chrono",
    "time",
    "uuid",
    "ulid",
    "clankers-config",
    "clankers-model-selection",
];

const HOST_FORBIDDEN_TRANSITIVE_CRATES: &[&str] = &[
    "clankers-agent",
    "clankers-core",
    "clankers-controller",
    "clankers-provider",
    "clanker-router",
    "clankers-db",
    "clankers-hooks",
    "clankers-plugin",
    "clankers-protocol",
    "clanker-tui-types",
    "clankers-tui",
    "ratatui",
    "crossterm",
    "portable-pty",
    "iroh",
    "redb",
    "reqwest",
    "hyper",
    "h2",
    "tower",
    "axum",
    "tokio",
    "async-std",
    "smol",
    "actix-rt",
    "reqwest-eventsource",
    "eventsource-stream",
    "clankers-config",
    "clankers-model-selection",
];

const MESSAGE_FORBIDDEN_CRATES: &[&str] = &[
    "clanker-router",
    "clankers-provider",
    "tokio",
    "reqwest",
    "reqwest-eventsource",
    "redb",
    "fs4",
    "iroh",
    "axum",
    "tower-http",
    "ratatui",
    "crossterm",
    "portable-pty",
];

const ENGINE_FORBIDDEN_SOURCE_TOKENS: &[&str] = &[
    "core_state",
    "CoreState",
    "CoreEffectId",
    "clankers_core",
    "clankers_provider",
    "clanker_router",
    "clankers_protocol",
    "clanker_tui_types",
    "clankers_db",
    "CompletionRequest",
    "CompletionResponse",
    "ProviderResponse",
    "tokio::runtime::Handle",
    "tokio::task::JoinHandle",
    "reqwest::Client",
    "AgentMessage",
    "MessageId",
    "Utc",
    "DateTime",
    "Instant::now",
    "SystemTime",
    "OnceLock",
    "OnceCell",
    "LazyLock",
    "lazy_static",
    "service_locator",
    "global_service",
    "singleton",
];

const HOST_FORBIDDEN_SOURCE_TOKENS: &[&str] = &[
    "clankers_agent",
    "clankers_provider",
    "clanker_router",
    "clankers_protocol",
    "clanker_tui_types",
    "clankers_db",
    "clankers_config",
    "CompletionRequest",
    "CompletionResponse",
    "ProviderResponse",
    "tokio::runtime::Handle",
    "tokio::task::JoinHandle",
    "reqwest::Client",
    "AgentMessage",
    "MessageId",
    "Utc",
    "DateTime",
    "Instant::now",
    "SystemTime",
    "OnceLock",
    "OnceCell",
    "LazyLock",
    "lazy_static",
    "service_locator",
    "global_service",
    "singleton",
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("llm contract boundary error: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), String> {
    check_tree_excludes(ENGINE_PACKAGE, ENGINE_FORBIDDEN_CRATES)?;
    check_tree_excludes(ENGINE_HOST_PACKAGE, HOST_FORBIDDEN_TRANSITIVE_CRATES)?;
    check_tree_excludes(TOOL_HOST_PACKAGE, HOST_FORBIDDEN_TRANSITIVE_CRATES)?;
    check_tree_excludes(MESSAGE_PACKAGE, MESSAGE_FORBIDDEN_CRATES)?;
    check_direct_normal_deps_exclude(ENGINE_HOST_PACKAGE, HOST_FORBIDDEN_DIRECT_CRATES)?;
    check_direct_normal_deps_exclude(TOOL_HOST_PACKAGE, HOST_FORBIDDEN_DIRECT_CRATES)?;
    check_source_excludes_tokens("crates/clankers-engine/src", ENGINE_FORBIDDEN_SOURCE_TOKENS)?;
    check_source_excludes_tokens("crates/clankers-engine-host/src", HOST_FORBIDDEN_SOURCE_TOKENS)?;
    check_source_excludes_tokens("crates/clankers-tool-host/src", HOST_FORBIDDEN_SOURCE_TOKENS)?;
    Ok(())
}

fn check_tree_excludes(package_name: &str, forbidden: &[&str]) -> Result<(), String> {
    let output = command_output("cargo", ["tree", "-p", package_name, "--edges", CARGO_TREE_EDGES])?;
    let found = forbidden
        .iter()
        .filter(|crate_name| output.contains(&format!("{crate_name} v")))
        .copied()
        .collect::<Vec<_>>();
    if !found.is_empty() {
        return Err(format!(
            "forbidden dependency in {package_name} normal-edge tree: {}\n\n--- cargo tree -p {package_name} --edges {CARGO_TREE_EDGES} ---\n{output}",
            found.join(", ")
        ));
    }
    println!("ok: {package_name} normal-edge tree excludes forbidden crates");
    Ok(())
}

fn check_direct_normal_deps_exclude(package_name: &str, forbidden: &[&str]) -> Result<(), String> {
    let output = command_output("cargo", ["tree", "-p", package_name, "--edges", "normal", "--depth", "1"])?;
    let found = forbidden
        .iter()
        .filter(|crate_name| output.contains(&format!("{crate_name} v")))
        .copied()
        .collect::<Vec<_>>();
    if !found.is_empty() {
        return Err(format!(
            "forbidden direct normal dependency in {package_name}: {}\n\n--- cargo tree -p {package_name} --edges normal --depth 1 ---\n{output}",
            found.join(", ")
        ));
    }
    println!("ok: {package_name} direct normal deps exclude forbidden crates");
    Ok(())
}

fn check_source_excludes_tokens(relative_dir: &str, forbidden: &[&str]) -> Result<(), String> {
    let mut matches = Vec::new();
    for path in rust_files(Path::new(relative_dir))? {
        let text = fs::read_to_string(&path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        for token in forbidden {
            for (index, line) in text.lines().enumerate() {
                if line.contains(token) {
                    matches.push(format!(
                        "forbidden source token in {}: {token}: {}:{}",
                        path.display(),
                        index + 1,
                        line.trim()
                    ));
                }
            }
        }
    }
    if !matches.is_empty() {
        return Err(matches.join("\n"));
    }
    println!("ok: {relative_dir} excludes forbidden source tokens");
    Ok(())
}

fn rust_files(root: &Path) -> Result<Vec<std::path::PathBuf>, String> {
    let mut files = Vec::new();
    collect_rust_files(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_rust_files(path: &Path, files: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
    if path.is_file() {
        if path.extension() == Some(OsStr::new("rs")) {
            files.push(path.to_path_buf());
        }
        return Ok(());
    }
    for entry in fs::read_dir(path).map_err(|error| format!("failed to read {}: {error}", path.display()))? {
        let entry = entry.map_err(|error| format!("failed to read entry under {}: {error}", path.display()))?;
        collect_rust_files(&entry.path(), files)?;
    }
    Ok(())
}

fn command_output<const N: usize>(program: &str, args: [&str; N]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|error| format!("failed to run {program}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{program} failed with status {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
