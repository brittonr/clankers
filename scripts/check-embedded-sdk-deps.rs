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
const SESSION_STORE_MANIFEST: &str = "examples/embedded-session-store/Cargo.toml";
const SESSION_STORE_PACKAGE: &str = "embedded-session-store-example";
const PRODUCT_WORKBENCH_MANIFEST: &str = "examples/embedded-product-workbench/Cargo.toml";
const PRODUCT_WORKBENCH_PACKAGE: &str = "embedded-product-workbench-example";
const GUIDE_PATH: &str = "docs/src/tutorials/embedded-agent-sdk.md";
const WORKSPACE_LAYER_POLICY: &str = "policy/workspace-layering/layers.json";
const EMBEDDABLE_MAX_LAYER_RANK: u64 = 1;
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

const SESSION_STORE_REQUIRED_DIRECT_DEPS: &[&str] = &[
    "clanker-message",
    "clankers-adapters",
    "clankers-engine",
    "clankers-engine-host",
    "serde_json",
];

const SESSION_STORE_ALLOWED_DIRECT_DEPS: &[&str] = SESSION_STORE_REQUIRED_DIRECT_DEPS;

const PRODUCT_WORKBENCH_REQUIRED_DIRECT_DEPS: &[&str] = &[
    "clanker-message",
    "clankers-adapters",
    "clankers-engine",
    "clankers-engine-host",
    "clankers-tool-host",
    "serde_json",
];

const PRODUCT_WORKBENCH_ALLOWED_DIRECT_DEPS: &[&str] = PRODUCT_WORKBENCH_REQUIRED_DIRECT_DEPS;

const EXTERNAL_FORBIDDEN_GRAPH_CRATES: &[&str] = &["chrono", "hex", "rand", "ratatui", "crossterm", "iroh"];

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

fn cargo_metadata(manifest: &str, label: &str) -> Result<Value, String> {
    let stdout = command_stdout(
        Command::new("cargo")
            .arg("metadata")
            .arg("--format-version")
            .arg("1")
            .arg("--locked")
            .arg("--manifest-path")
            .arg(manifest),
        label,
    )?;
    serde_json::from_str(&stdout).map_err(|error| format!("failed to parse cargo metadata JSON: {error}"))
}

fn cargo_check_example(manifest: &str, label: &str) -> Result<(), String> {
    command_stdout(
        Command::new("cargo")
            .arg("check")
            .arg("--quiet")
            .arg("--locked")
            .arg("--manifest-path")
            .arg(manifest),
        label,
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

fn example_direct_deps(metadata: &Value, package_name: &str) -> Result<BTreeSet<String>, String> {
    let packages = metadata
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| "cargo metadata JSON missing packages array".to_string())?;
    let package = packages
        .iter()
        .find(|package| package.get("name").and_then(Value::as_str) == Some(package_name))
        .ok_or_else(|| format!("cargo metadata missing package {package_name}"))?;
    let deps = package
        .get("dependencies")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("package {package_name} missing dependencies array"))?;
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

fn validate_standalone_manifest(errors: &mut Vec<String>, manifest_path: &str) {
    match read_text(manifest_path) {
        Ok(text) => {
            if !text.contains("[workspace]") {
                errors.push(format!("{manifest_path} must declare [workspace] so it stays standalone"));
            }
        }
        Err(error) => errors.push(error),
    }
}

fn validate_direct_deps(
    errors: &mut Vec<String>,
    direct_deps: &BTreeSet<String>,
    required_deps: &[&str],
    allowed_deps: &[&str],
    label: &str,
) {
    for required in required_deps {
        if !direct_deps.contains(*required) {
            errors.push(format!("{label} missing required direct dependency `{required}`"));
        }
    }

    let allowed: BTreeSet<&str> = allowed_deps.iter().copied().collect();
    for dep in direct_deps {
        if !allowed.contains(dep.as_str()) {
            errors.push(format!("{label} has undocumented direct dependency `{dep}`"));
        }
    }
}

fn workspace_forbidden_graph_crates() -> Result<BTreeSet<String>, String> {
    let policy_text = read_text(WORKSPACE_LAYER_POLICY)?;
    let policy: Value = serde_json::from_str(&policy_text)
        .map_err(|error| format!("failed to parse {WORKSPACE_LAYER_POLICY}: {error}"))?;
    let layers = policy
        .get("layers")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{WORKSPACE_LAYER_POLICY} missing layers array"))?;
    let mut forbidden = BTreeSet::new();
    for layer in layers {
        let rank = layer
            .get("rank")
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("{WORKSPACE_LAYER_POLICY} layer missing rank"))?;
        if rank <= EMBEDDABLE_MAX_LAYER_RANK {
            continue;
        }
        let packages = layer
            .get("packages")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("{WORKSPACE_LAYER_POLICY} layer missing packages"))?;
        for package in packages {
            let package =
                package.as_str().ok_or_else(|| format!("{WORKSPACE_LAYER_POLICY} package entry must be a string"))?;
            forbidden.insert(package.to_string());
        }
    }
    Ok(forbidden)
}

fn validate_forbidden_graph(
    errors: &mut Vec<String>,
    package_names: &BTreeSet<String>,
    workspace_forbidden: &BTreeSet<String>,
) {
    for forbidden in workspace_forbidden {
        if package_names.contains(forbidden) {
            errors.push(format!(
                "example dependency graph includes workspace crate `{forbidden}` above embeddable layer rank {EMBEDDABLE_MAX_LAYER_RANK}; update {WORKSPACE_LAYER_POLICY} or the example dependency boundary"
            ));
        }
    }
    for forbidden in EXTERNAL_FORBIDDEN_GRAPH_CRATES {
        if package_names.contains(*forbidden) {
            errors.push(format!("example dependency graph includes forbidden external crate `{forbidden}`"));
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
    let metadata = cargo_metadata(EXAMPLE_MANIFEST, "cargo metadata for embedded SDK example")?;
    cargo_check_example(EXAMPLE_MANIFEST, "cargo check for embedded SDK example")?;
    let session_metadata = cargo_metadata(SESSION_STORE_MANIFEST, "cargo metadata for embedded session-store example")?;
    cargo_check_example(SESSION_STORE_MANIFEST, "cargo check for embedded session-store example")?;
    let product_workbench_metadata =
        cargo_metadata(PRODUCT_WORKBENCH_MANIFEST, "cargo metadata for embedded product-workbench example")?;
    cargo_check_example(PRODUCT_WORKBENCH_MANIFEST, "cargo check for embedded product-workbench example")?;

    let names = package_names(&metadata)?;
    let direct_deps = example_direct_deps(&metadata, EXAMPLE_PACKAGE)?;
    let session_names = package_names(&session_metadata)?;
    let session_direct_deps = example_direct_deps(&session_metadata, SESSION_STORE_PACKAGE)?;
    let product_workbench_names = package_names(&product_workbench_metadata)?;
    let product_workbench_direct_deps = example_direct_deps(&product_workbench_metadata, PRODUCT_WORKBENCH_PACKAGE)?;
    let workspace_forbidden = workspace_forbidden_graph_crates()?;
    let mut errors = Vec::new();
    validate_standalone_manifest(&mut errors, EXAMPLE_MANIFEST);
    validate_direct_deps(&mut errors, &direct_deps, REQUIRED_DIRECT_DEPS, ALLOWED_DIRECT_DEPS, EXAMPLE_PACKAGE);
    validate_forbidden_graph(&mut errors, &names, &workspace_forbidden);
    validate_standalone_manifest(&mut errors, SESSION_STORE_MANIFEST);
    validate_direct_deps(
        &mut errors,
        &session_direct_deps,
        SESSION_STORE_REQUIRED_DIRECT_DEPS,
        SESSION_STORE_ALLOWED_DIRECT_DEPS,
        SESSION_STORE_PACKAGE,
    );
    validate_forbidden_graph(&mut errors, &session_names, &workspace_forbidden);
    validate_standalone_manifest(&mut errors, PRODUCT_WORKBENCH_MANIFEST);
    validate_direct_deps(
        &mut errors,
        &product_workbench_direct_deps,
        PRODUCT_WORKBENCH_REQUIRED_DIRECT_DEPS,
        PRODUCT_WORKBENCH_ALLOWED_DIRECT_DEPS,
        PRODUCT_WORKBENCH_PACKAGE,
    );
    validate_forbidden_graph(&mut errors, &product_workbench_names, &workspace_forbidden);
    validate_feature_policy(&mut errors);

    Ok(CheckReport {
        errors,
        package_count: names.len() + session_names.len() + product_workbench_names.len(),
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
