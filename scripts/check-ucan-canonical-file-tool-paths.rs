#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"

[dependencies]
blake3 = "1"
serde_json = "1"
---

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;

use serde_json::json;

const ERROR_EXIT: u8 = 1;
const CAPABILITY_GATE: &str = "src/capability_gate.rs";
const SESSION_STORE: &str = "src/modes/daemon/session_store.rs";
const QUIC_BRIDGE: &str = "src/modes/daemon/quic_bridge.rs";
const CHECKER: &str = "scripts/check-ucan-canonical-file-tool-paths.rs";
const ACTIVE_SPEC: &str = "cairn/changes/ucan-canonical-file-tool-paths/specs/ucan-basalt-daemon-auth/spec.md";
const CANONICAL_SPEC: &str = "cairn/specs/ucan-basalt-daemon-auth/spec.md";
const ACTIVE_TASKS: &str = "cairn/changes/ucan-canonical-file-tool-paths/tasks.md";
const ARCHIVED_TASKS: &str = "cairn/archive/1970-01-01-ucan-canonical-file-tool-paths/tasks.md";
const OUTPUT: &str = "target/ucan-canonical-file-tool-paths/receipt.json";

const REQUIRED_GATE_MARKERS: &[&str] = &[
    "file_authority_root: Option<PathBuf>",
    "with_file_authority_root",
    "public_file_tool_authorization_path",
    "rooted_relative_path",
    "absolute_root_path",
    "path_parts",
    "requires session file root for relative file path",
    "escapes session file root",
    "public_ucan_relative_file_path_resolves_under_file_root",
    "public_ucan_relative_file_path_requires_file_root_and_denies_escape",
    "public_ucan_absolute_file_path_keeps_explicit_resource_semantics",
    "legacy_ucan_gate_preserves_tool_only_default_capabilities_behavior",
];
const REQUIRED_SESSION_STORE_MARKERS: &[&str] = &[
    "std::env::current_dir()",
    "with_file_authority_root(cwd)",
    "public_tool_authorization_for_file_root",
];
const REQUIRED_QUIC_MARKERS: &[&str] = &[
    "cwd,",
    "public_tool_authorization_for_file_root(credential, file_root)",
];
const REQUIRED_SPEC_MARKERS: &[&str] = &[
    "r[ucan-basalt-daemon-auth.vocabulary.canonical-file-tool-paths.root-context]",
    "r[ucan-basalt-daemon-auth.vocabulary.canonical-file-tool-paths.relative-resolution]",
    "r[ucan-basalt-daemon-auth.vocabulary.canonical-file-tool-paths.traversal-denial]",
    "r[ucan-basalt-daemon-auth.vocabulary.canonical-file-tool-paths.absolute-paths]",
    "r[ucan-basalt-daemon-auth.tool-gate.canonical-file-tool-paths.call-time]",
    "r[ucan-basalt-daemon-auth.verification.canonical-file-path-checker]",
];
const REQUIRED_TASK_MARKERS: &[&str] = &[
    "I1. Extend the public UCAN tool authorization context with a session/project file root",
    "I2. Add a pure public file-path normalization helper",
    "I3. Route public UCAN `file/read` and `file/write` request construction through the canonicalization helper",
    "V1. Add focused root/capability-gate tests proving `src/lib.rs` resolves under the session root",
    "V2. Add and run a deterministic checker receipt",
    "V3. Run focused Rust tests",
];
const FORBIDDEN_GATE_MARKERS: &[&str] = &[
    "canonicalize()",
    "std::fs::canonicalize",
    "tokio::fs::canonicalize",
];

fn main() -> ExitCode {
    match run() {
        Ok(path) => {
            println!("UCAN canonical file-tool path receipt written to {}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("UCAN canonical file-tool path check failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<PathBuf, String> {
    let gate = read(CAPABILITY_GATE)?;
    let session_store = read(SESSION_STORE)?;
    let quic_bridge = read(QUIC_BRIDGE)?;
    let spec_path = first_existing(&[ACTIVE_SPEC, CANONICAL_SPEC])?;
    let spec = read(&spec_path)?;
    let tasks_path = first_existing(&[ACTIVE_TASKS, ARCHIVED_TASKS])?;
    let tasks = read(&tasks_path)?;
    let mut errors = Vec::new();

    require_all(CAPABILITY_GATE, &gate, REQUIRED_GATE_MARKERS, &mut errors);
    require_all(SESSION_STORE, &session_store, REQUIRED_SESSION_STORE_MARKERS, &mut errors);
    require_all(QUIC_BRIDGE, &quic_bridge, REQUIRED_QUIC_MARKERS, &mut errors);
    require_all(&spec_path, &spec, REQUIRED_SPEC_MARKERS, &mut errors);
    require_all(&tasks_path, &tasks, REQUIRED_TASK_MARKERS, &mut errors);
    forbid_all(CAPABILITY_GATE, &gate, FORBIDDEN_GATE_MARKERS, &mut errors);
    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }

    let artifacts = [CAPABILITY_GATE, SESSION_STORE, QUIC_BRIDGE, CHECKER, &spec_path, &tasks_path]
        .iter()
        .map(|path| hash_artifact(Path::new(path)))
        .collect::<Result<Vec<_>, _>>()?;
    let receipt = json!({
        "schema": "clankers.ucan_canonical_file_tool_paths.receipt.v1",
        "validated_surfaces": [
            "public-auth-file-root-threading",
            "relative-file-path-root-resolution",
            "parent-traversal-denial",
            "absolute-file-path-resource-preservation",
            "legacy-local-gate-non-regression"
        ],
        "redaction": {
            "raw_compact_ucan_tokens": false,
            "signing_keys": false,
            "prompts": false,
            "provider_payloads": false,
            "file_contents": false,
            "tool_input_bodies": false
        },
        "hashed_artifacts": artifacts,
    });
    let output_path = PathBuf::from(OUTPUT);
    let parent = output_path.parent().ok_or_else(|| format!("{} has no parent", output_path.display()))?;
    fs::create_dir_all(parent).map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let bytes = serde_json::to_vec_pretty(&receipt).map_err(|error| format!("failed to encode receipt: {error}"))?;
    fs::write(&output_path, [bytes.as_slice(), b"\n"].concat())
        .map_err(|error| format!("failed to write {}: {error}", output_path.display()))?;
    Ok(output_path)
}

fn first_existing(paths: &[&str]) -> Result<String, String> {
    paths
        .iter()
        .find(|path| Path::new(path).exists())
        .map(|path| (*path).to_owned())
        .ok_or_else(|| format!("none of these paths exist: {}", paths.join(", ")))
}

fn read(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("failed to read {path}: {error}"))
}

fn require_all(path: &str, text: &str, markers: &[&str], errors: &mut Vec<String>) {
    for marker in markers {
        if !text.contains(marker) {
            errors.push(format!("{path} missing marker `{marker}`"));
        }
    }
}

fn forbid_all(path: &str, text: &str, markers: &[&str], errors: &mut Vec<String>) {
    for marker in markers {
        if text.contains(marker) {
            errors.push(format!("{path} contains forbidden marker `{marker}`"));
        }
    }
}

fn hash_artifact(path: &Path) -> Result<serde_json::Value, String> {
    let bytes = fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let hash = blake3::hash(&bytes).to_hex().to_string();
    Ok(json!({
        "path": path.display().to_string(),
        "bytes": bytes.len(),
        "blake3": hash,
    }))
}
