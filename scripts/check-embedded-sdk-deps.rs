#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
[dependencies]
serde_json = "1"
---

use std::collections::BTreeSet;
use std::fs;
use std::process::Command;

use serde_json::Value;

const EXAMPLE_MANIFEST: &str = "examples/embedded-agent-sdk/Cargo.toml";
const EXAMPLE_PACKAGE: &str = "embedded-agent-sdk-example";
const GUIDE_PATH: &str = "docs/src/tutorials/embedded-agent-sdk.md";
const SUCCESS_EXIT: i32 = 0;
const ERROR_EXIT: i32 = 1;

const REQUIRED_DIRECT_DEPS: &[&str] = &[
    "clanker-message",
    "clankers-engine",
    "clankers-engine-host",
    "clankers-tool-host",
    "serde_json",
];

const ALLOWED_DIRECT_DEPS: &[&str] = REQUIRED_DIRECT_DEPS;

const FORBIDDEN_GRAPH_CRATES: &[&str] = &[
    "clankers-agent",
    "clankers-controller",
    "clankers-provider",
    "clanker-router",
    "clankers-db",
    "clankers-protocol",
    "clankers-tui",
    "clankers-prompts",
    "clankers-skills",
    "clankers-config",
    "clankers-agent-defs",
    "ratatui",
    "crossterm",
    "iroh",
];

const SDK_MANIFESTS_WITHOUT_FEATURES: &[(&str, &str)] = &[
    ("clankers-engine", "crates/clankers-engine/Cargo.toml"),
    ("clankers-engine-host", "crates/clankers-engine-host/Cargo.toml"),
    ("clankers-tool-host", "crates/clankers-tool-host/Cargo.toml"),
];

#[derive(Debug)]
struct CheckReport {
    errors: Vec<String>,
    package_count: usize,
}

fn read_text(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("failed to read {path}: {error}"))
}

fn command_stdout(command: &mut Command, label: &str) -> Result<String, String> {
    let output = command.output().map_err(|error| format!("failed to run {label}: {error}"))?;
    if output.status.code().unwrap_or(ERROR_EXIT) != SUCCESS_EXIT {
        return Err(format!(
            "{label} failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout).map_err(|error| format!("{label} emitted non-UTF8 stdout: {error}"))
}

fn cargo_metadata() -> Result<Value, String> {
    let stdout = command_stdout(
        Command::new("cargo")
            .arg("metadata")
            .arg("--format-version")
            .arg("1")
            .arg("--locked")
            .arg("--manifest-path")
            .arg(EXAMPLE_MANIFEST),
        "cargo metadata for embedded SDK example",
    )?;
    serde_json::from_str(&stdout).map_err(|error| format!("failed to parse cargo metadata JSON: {error}"))
}

fn cargo_check_example() -> Result<(), String> {
    command_stdout(
        Command::new("cargo")
            .arg("check")
            .arg("--quiet")
            .arg("--locked")
            .arg("--manifest-path")
            .arg(EXAMPLE_MANIFEST),
        "cargo check for embedded SDK example",
    )?;
    Ok(())
}

fn package_names(metadata: &Value) -> Result<BTreeSet<String>, String> {
    let packages = metadata
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| "cargo metadata JSON missing packages array".to_string())?;
    let mut names = BTreeSet::new();
    for package in packages {
        let name = package
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| "cargo metadata package missing name".to_string())?;
        names.insert(name.to_string());
    }
    Ok(names)
}

fn example_direct_deps(metadata: &Value) -> Result<BTreeSet<String>, String> {
    let packages = metadata
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| "cargo metadata JSON missing packages array".to_string())?;
    let package = packages
        .iter()
        .find(|package| package.get("name").and_then(Value::as_str) == Some(EXAMPLE_PACKAGE))
        .ok_or_else(|| format!("cargo metadata missing package {EXAMPLE_PACKAGE}"))?;
    let deps = package
        .get("dependencies")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("package {EXAMPLE_PACKAGE} missing dependencies array"))?;
    let mut names = BTreeSet::new();
    for dep in deps {
        let name = dep
            .get("rename")
            .and_then(Value::as_str)
            .or_else(|| dep.get("name").and_then(Value::as_str))
            .ok_or_else(|| "dependency missing name".to_string())?;
        names.insert(name.to_string());
    }
    Ok(names)
}

fn validate_standalone_manifest(errors: &mut Vec<String>) {
    match read_text(EXAMPLE_MANIFEST) {
        Ok(text) => {
            if !text.contains("[workspace]") {
                errors.push(format!("{EXAMPLE_MANIFEST} must declare [workspace] so it stays standalone"));
            }
        }
        Err(error) => errors.push(error),
    }
}

fn validate_direct_deps(errors: &mut Vec<String>, direct_deps: &BTreeSet<String>) {
    for required in REQUIRED_DIRECT_DEPS {
        if !direct_deps.contains(*required) {
            errors.push(format!("example missing required direct dependency `{required}`"));
        }
    }

    let allowed: BTreeSet<&str> = ALLOWED_DIRECT_DEPS.iter().copied().collect();
    for dep in direct_deps {
        if !allowed.contains(dep.as_str()) {
            errors.push(format!("example has undocumented direct dependency `{dep}`"));
        }
    }
}

fn validate_forbidden_graph(errors: &mut Vec<String>, package_names: &BTreeSet<String>) {
    for forbidden in FORBIDDEN_GRAPH_CRATES {
        if package_names.contains(*forbidden) {
            errors.push(format!("example dependency graph includes forbidden crate `{forbidden}`"));
        }
    }
}

fn validate_feature_policy(errors: &mut Vec<String>) {
    let guide = match read_text(GUIDE_PATH) {
        Ok(guide) => guide,
        Err(error) => {
            errors.push(error);
            return;
        }
    };

    for (crate_name, manifest_path) in SDK_MANIFESTS_WITHOUT_FEATURES {
        match read_text(manifest_path) {
            Ok(manifest) => {
                if has_features_section(&manifest) {
                    errors.push(format!(
                        "{manifest_path} declares [features], but {GUIDE_PATH} documents no optional features for `{crate_name}`"
                    ));
                }
            }
            Err(error) => errors.push(error),
        }

        let documented_phrase = format!("`{crate_name}`: no optional features");
        if !guide.contains(&documented_phrase) {
            errors.push(format!("{GUIDE_PATH} missing feature policy phrase: {documented_phrase}"));
        }
    }
}

fn has_features_section(manifest: &str) -> bool {
    manifest.lines().map(str::trim).any(|line| line == "[features]")
}

fn run() -> Result<CheckReport, String> {
    let metadata = cargo_metadata()?;
    cargo_check_example()?;

    let names = package_names(&metadata)?;
    let direct_deps = example_direct_deps(&metadata)?;
    let mut errors = Vec::new();
    validate_standalone_manifest(&mut errors);
    validate_direct_deps(&mut errors, &direct_deps);
    validate_forbidden_graph(&mut errors, &names);
    validate_feature_policy(&mut errors);

    Ok(CheckReport {
        errors,
        package_count: names.len(),
    })
}

fn main() {
    match run() {
        Ok(report) if report.errors.is_empty() => {
            println!(
                "ok: embedded SDK example dependency graph has {} packages and excludes forbidden runtime crates",
                report.package_count
            );
        }
        Ok(report) => {
            for error in report.errors {
                eprintln!("{error}");
            }
            std::process::exit(ERROR_EXIT);
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(ERROR_EXIT);
        }
    }
}
