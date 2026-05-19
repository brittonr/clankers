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
use std::path::PathBuf;
use std::process::Command;
use std::process::ExitCode;

use serde_json::json;

const DEFAULT_OUTPUT: &str = "target/embedded-sdk-release/receipt.json";
const ERROR_EXIT: u8 = 1;

const GREEN_CRATES: &[&str] = &[
    "clanker-message",
    "clankers-engine",
    "clankers-engine-host",
    "clankers-tool-host",
    "clankers-adapters",
    "clankers-core (optional prompt lifecycle reducer)",
];

const YELLOW_APP_EDGE_SURFACES: &[&str] = &[
    "daemon/MCP/ACP control surfaces behind product-owned integration layers",
    "product-owned provider adapters",
    "product-owned session/message DTOs and storage schemas",
    "runtime extension services selected by the product",
];

const RED_EXCLUSIONS: &[&str] = &[
    "daemon protocol clients as generic SDK dependencies",
    "TUI/rendering/keybinding crates",
    "provider discovery, router daemon RPC, and OAuth stores",
    "Clankers session database ownership and JSONL restore shells",
    "plugin supervision, Matrix, iroh/P2P, and built-in Clankers tool bundles",
    "live credentials, network access, daemon startup, or shell-global service lookup",
];

const VERIFICATION_COMMANDS: &[&str] = &[
    "scripts/check-embedded-agent-sdk.sh",
    "cargo check --workspace --all-targets",
    "openspec validate embedded-composition-kits --strict --json",
    "cargo fmt --check",
    "git diff --check",
];

const DIRECT_ARTIFACTS: &[&str] = &[
    "docs/src/tutorials/embedded-agent-sdk.md",
    "docs/src/generated/embedded-sdk-api.md",
    "openspec/specs/embedded-composition-kits/spec.md",
    "scripts/check-embedded-agent-sdk.sh",
    "scripts/emit-embedded-sdk-release-receipt.rs",
    "scripts/check-embedded-sdk-api.rs",
    "scripts/check-embedded-lego-contracts.rs",
    "scripts/check-embedded-sdk-deps.rs",
    "scripts/check-embedded-adapters-deps.rs",
    "scripts/check-engine-host-feature-matrix.rs",
    "scripts/check-tool-catalog-matrix.rs",
    "scripts/check-runtime-extension-service-matrix.rs",
    "scripts/check-shell-adapter-parity-matrix.rs",
    "scripts/check-llm-contract-boundary.sh",
    "policy/embedded-lego/lego-contracts.ncl",
    "policy/embedded-lego/lego-contracts.json",
];

const EXAMPLE_DIRS: &[&str] = &[
    "examples/embedded-agent-sdk",
    "examples/embedded-minimal-kit",
    "examples/embedded-tool-kit",
    "examples/embedded-provider-adapter",
    "examples/embedded-session-store",
    "examples/embedded-product-workbench",
];

#[derive(Debug)]
struct ArtifactDigest {
    path: String,
    bytes: u64,
    blake3: String,
}

fn main() -> ExitCode {
    match run() {
        Ok(output_path) => {
            println!("embedded SDK release receipt written to {}", output_path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("embedded SDK release receipt error: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<PathBuf, String> {
    let output_path = parse_output_path(env::args().skip(1))?;
    let artifacts = collect_artifacts()?;
    let receipt = json!({
        "schema": "clankers.embedded_sdk.release_receipt.v1",
        "repo": "clankers",
        "git": {
            "commit": git_output(["rev-parse", "HEAD"]),
            "commit_date": git_output(["show", "-s", "--format=%cI", "HEAD"]),
            "status_short_branch": git_output(["status", "--short", "--branch"]),
        },
        "sdk_boundary": {
            "green_generic_sdk_crates": GREEN_CRATES,
            "yellow_app_edge_surfaces": YELLOW_APP_EDGE_SURFACES,
            "red_generic_sdk_exclusions": RED_EXCLUSIONS,
        },
        "verification_commands": VERIFICATION_COMMANDS,
        "hashed_artifacts": artifacts.iter().map(|artifact| json!({
            "path": artifact.path,
            "bytes": artifact.bytes,
            "blake3": artifact.blake3,
        })).collect::<Vec<_>>(),
        "release_guidance": {
            "capture_from_clean_checkout": true,
            "receipt_output_default": DEFAULT_OUTPUT,
            "readiness_claim": "Run the verification_commands first; this receipt records the embedded SDK boundary and hashes the docs/spec/scripts/examples used as evidence.",
        },
    });

    let parent = output_path.parent().ok_or_else(|| format!("{} has no parent directory", output_path.display()))?;
    fs::create_dir_all(parent).map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let bytes =
        serde_json::to_vec_pretty(&receipt).map_err(|error| format!("failed to encode receipt JSON: {error}"))?;
    fs::write(&output_path, [bytes.as_slice(), b"\n"].concat())
        .map_err(|error| format!("failed to write {}: {error}", output_path.display()))?;
    Ok(output_path)
}

fn parse_output_path(args: impl Iterator<Item = String>) -> Result<PathBuf, String> {
    let mut output_path = PathBuf::from(DEFAULT_OUTPUT);
    let mut pending_output = false;

    for arg in args {
        if pending_output {
            output_path = PathBuf::from(arg);
            pending_output = false;
            continue;
        }
        match arg.as_str() {
            "--output" => pending_output = true,
            "--help" | "-h" => {
                println!("usage: emit-embedded-sdk-release-receipt.rs [--output PATH]");
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }

    if pending_output {
        return Err("--output requires a path".to_string());
    }
    Ok(output_path)
}

fn collect_artifacts() -> Result<Vec<ArtifactDigest>, String> {
    let mut paths = DIRECT_ARTIFACTS.iter().map(PathBuf::from).collect::<Vec<_>>();
    for dir in EXAMPLE_DIRS {
        collect_files(Path::new(dir), &mut paths)?;
    }
    paths.sort();
    paths.dedup();

    let mut artifacts = Vec::new();
    for path in paths {
        artifacts.push(hash_artifact(&path)?);
    }
    Ok(artifacts)
}

fn collect_files(path: &Path, paths: &mut Vec<PathBuf>) -> Result<(), String> {
    if path.is_file() {
        paths.push(path.to_path_buf());
        return Ok(());
    }

    let entries = fs::read_dir(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("failed to read entry under {}: {error}", path.display()))?;
        let entry_path = entry.path();
        let name = entry_path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
        if name == "target" || name == ".git" {
            continue;
        }
        collect_files(&entry_path, paths)?;
    }
    Ok(())
}

fn hash_artifact(path: &Path) -> Result<ArtifactDigest, String> {
    let mut file = fs::File::open(path).map_err(|error| format!("failed to open {}: {error}", path.display()))?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 16 * 1024];
    let mut bytes = 0u64;

    loop {
        let read = file.read(&mut buffer).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        bytes += read as u64;
        hasher.update(&buffer[..read]);
    }

    Ok(ArtifactDigest {
        path: path.to_string_lossy().to_string(),
        bytes,
        blake3: hasher.finalize().to_hex().to_string(),
    })
}

fn git_output<const N: usize>(args: [&str; N]) -> String {
    let output = Command::new("git").args(args).output();
    match output {
        Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout).trim().to_string(),
        Ok(output) => format!("git command failed: {}", String::from_utf8_lossy(&output.stderr).trim()),
        Err(error) => format!("git command unavailable: {error}"),
    }
}
