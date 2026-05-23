#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"

[dependencies]
blake3 = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
---

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::ExitCode;

use serde::Serialize;
use serde_json::Value;

const ERROR_EXIT: u8 = 1;
const SCHEMA: &str = "clankers.current_head_release_evidence_index.v1";
const DEFAULT_RESULT_DIR: &str = "target/test-harness";
const DEFAULT_OUT_DIR: &str = "target/release-evidence/current-head";
const REQUIRED_MODES: &[&str] = &["quick", "full", "deterministic", "e2e", "live", "vm", "ci"];

fn main() -> ExitCode {
    match run() {
        Ok(paths) => {
            println!("release evidence index written");
            println!("json: {}", paths.json.display());
            println!("markdown: {}", paths.markdown.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("release evidence index failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

#[derive(Debug)]
struct Config {
    result_dir: PathBuf,
    out_dir: PathBuf,
    allow_dirty: bool,
}

#[derive(Debug)]
struct OutputPaths {
    json: PathBuf,
    markdown: PathBuf,
}

#[derive(Serialize)]
struct IndexReceipt {
    schema: &'static str,
    repository: RepositoryState,
    harness_result_dir: String,
    selected_receipts: BTreeMap<String, SelectedReceipt>,
    missing_modes: Vec<String>,
    rejected_receipts: Vec<RejectedReceipt>,
    non_claims: Vec<String>,
}

#[derive(Serialize)]
struct RepositoryState {
    branch: String,
    head: String,
    head_short: String,
    upstream: Option<String>,
    ahead_behind: Option<String>,
    describe: String,
    tracked_dirty: bool,
    active_cairn_changes: Vec<String>,
    active_openspec_changes: Vec<String>,
}

#[derive(Clone, Serialize)]
struct SelectedReceipt {
    mode: String,
    run_id: String,
    finished_at: String,
    passed: u64,
    skipped: u64,
    summary: ArtifactRef,
    results: ArtifactRef,
    logs: Vec<ArtifactRef>,
    payload_commit_verified: bool,
    note: String,
}

#[derive(Clone, Serialize)]
struct ArtifactRef {
    path: String,
    blake3: String,
}

#[derive(Serialize)]
struct RejectedReceipt {
    path: String,
    reason: String,
}

fn run() -> Result<OutputPaths, String> {
    let config = parse_args()?;
    let repository = collect_repository_state()?;
    if repository.tracked_dirty && !config.allow_dirty {
        return Err(
            "tracked worktree is dirty; commit/stash changes or rerun with --allow-dirty for development-only output"
                .to_string(),
        );
    }

    let scan = scan_receipts(&config.result_dir, &repository)?;
    let missing_modes = REQUIRED_MODES
        .iter()
        .filter(|mode| !scan.selected.contains_key(**mode))
        .map(|mode| (*mode).to_string())
        .collect::<Vec<_>>();

    let index = IndexReceipt {
        schema: SCHEMA,
        repository,
        harness_result_dir: display_path(&config.result_dir),
        selected_receipts: scan.selected,
        missing_modes,
        rejected_receipts: scan.rejected,
        non_claims: vec![
            "This index summarizes local receipts; it does not run missing readiness profiles.".to_string(),
            "Older harness receipts do not record a payload commit, so selected receipt payload_commit_verified is false unless future receipt metadata proves it.".to_string(),
            "Missing modes are not readiness passes.".to_string(),
        ],
    };

    fs::create_dir_all(&config.out_dir)
        .map_err(|error| format!("failed to create {}: {error}", config.out_dir.display()))?;
    let json_path = config.out_dir.join("index.json");
    let md_path = config.out_dir.join("index.md");
    write_json(&json_path, &index)?;
    fs::write(&md_path, render_markdown(&index))
        .map_err(|error| format!("failed to write {}: {error}", md_path.display()))?;
    Ok(OutputPaths {
        json: json_path,
        markdown: md_path,
    })
}

struct ReceiptScan {
    selected: BTreeMap<String, SelectedReceipt>,
    rejected: Vec<RejectedReceipt>,
}

fn scan_receipts(result_dir: &Path, repository: &RepositoryState) -> Result<ReceiptScan, String> {
    let runs_dir = result_dir.join("runs");
    let mut candidates: BTreeMap<String, SelectedReceipt> = BTreeMap::new();
    let mut rejected = Vec::new();
    if !runs_dir.exists() {
        return Ok(ReceiptScan {
            selected: candidates,
            rejected,
        });
    }
    let entries = fs::read_dir(&runs_dir).map_err(|error| format!("failed to read {}: {error}", runs_dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("failed to read run dir entry: {error}"))?;
        let path = entry.path().join("results.json");
        if !path.is_file() {
            continue;
        }
        match parse_receipt(result_dir, &path, repository) {
            Ok(receipt) => {
                let replace = candidates
                    .get(&receipt.mode)
                    .map(|old| {
                        (receipt.finished_at.as_str(), receipt.run_id.as_str())
                            > (old.finished_at.as_str(), old.run_id.as_str())
                    })
                    .unwrap_or(true);
                if replace {
                    candidates.insert(receipt.mode.clone(), receipt);
                }
            }
            Err(reason) => rejected.push(RejectedReceipt {
                path: display_path(&path),
                reason,
            }),
        }
    }
    rejected.sort_by(|left, right| left.path.cmp(&right.path).then_with(|| left.reason.cmp(&right.reason)));
    Ok(ReceiptScan {
        selected: candidates,
        rejected,
    })
}

fn parse_receipt(result_dir: &Path, results_path: &Path, repository: &RepositoryState) -> Result<SelectedReceipt, String> {
    let text = fs::read_to_string(results_path).map_err(|error| format!("unreadable JSON: {error}"))?;
    let json: Value = serde_json::from_str(&text).map_err(|error| format!("invalid JSON: {error}"))?;
    let mode = required_str(&json, "mode")?.to_string();
    let run_id = required_str(&json, "run_id")?.to_string();
    let run_dir = required_str(&json, "run_dir")?;
    let finished_at = required_str(&json, "finished_at")?.to_string();
    let passed = required_u64(&json, "passed")?;
    let failed = required_u64(&json, "failed")?;
    let skipped = required_u64(&json, "skipped")?;
    if failed != 0 {
        return Err(format!("receipt has failed={failed}"));
    }
    if passed == 0 {
        return Err("receipt has no passed steps".to_string());
    }
    let summary_path = Path::new(run_dir).join("summary.md");
    let result_artifact = artifact(results_path)?;
    let summary_artifact = artifact(&summary_path)?;
    let mut logs = Vec::new();
    let mut seen_logs = BTreeSet::new();
    let steps = json.get("steps").and_then(Value::as_array).ok_or_else(|| "missing steps array".to_string())?;
    for step in steps {
        if step.get("status").and_then(Value::as_str) != Some("passed") {
            continue;
        }
        let log_path = required_str(step, "log")?;
        if seen_logs.insert(log_path.to_string()) {
            logs.push(artifact(Path::new(log_path))?);
        }
    }
    if logs.is_empty() {
        return Err("receipt has no passed step logs".to_string());
    }
    if !Path::new(run_dir).starts_with(result_dir) && !Path::new(run_dir).is_absolute() {
        return Err(format!("run_dir `{run_dir}` is not under result_dir `{}`", result_dir.display()));
    }
    let payload = receipt_payload_status(&json, repository);
    Ok(SelectedReceipt {
        mode,
        run_id,
        finished_at,
        passed,
        skipped,
        summary: summary_artifact,
        results: result_artifact,
        logs,
        payload_commit_verified: payload.verified,
        note: payload.note,
    })
}

struct PayloadStatus {
    verified: bool,
    note: String,
}

fn receipt_payload_status(json: &Value, repository: &RepositoryState) -> PayloadStatus {
    let Some(payload) = json.get("payload") else {
        return PayloadStatus {
            verified: false,
            note: "receipt selected from local harness output; payload commit unavailable in legacy harness schema".to_string(),
        };
    };
    let Some(commit) = payload.get("commit").and_then(Value::as_str) else {
        return PayloadStatus {
            verified: false,
            note: "receipt payload metadata is missing commit; not current-HEAD verified".to_string(),
        };
    };
    let tracked_dirty = payload.get("tracked_dirty").and_then(Value::as_bool).unwrap_or(true);
    if commit != repository.head {
        return PayloadStatus {
            verified: false,
            note: format!("receipt payload commit `{commit}` does not match indexed HEAD `{}`", repository.head),
        };
    }
    if tracked_dirty {
        return PayloadStatus {
            verified: false,
            note: "receipt payload was captured from a dirty tracked worktree; not current-HEAD verified".to_string(),
        };
    }
    PayloadStatus {
        verified: true,
        note: "receipt payload commit matches indexed HEAD and was captured from a clean tracked worktree".to_string(),
    }
}

fn artifact(path: &Path) -> Result<ArtifactRef, String> {
    if !path.is_file() {
        return Err(format!("referenced artifact missing: {}", path.display()));
    }
    let bytes = fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    Ok(ArtifactRef {
        path: display_path(path),
        blake3: blake3::hash(&bytes).to_hex().to_string(),
    })
}

fn collect_repository_state() -> Result<RepositoryState, String> {
    let branch = git(&["branch", "--show-current"])?;
    let head = git(&["rev-parse", "HEAD"])?;
    let head_short = git(&["rev-parse", "--short=12", "HEAD"])?;
    let upstream = git(&["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]).ok().filter(|s| !s.is_empty());
    let ahead_behind =
        upstream.as_ref().and_then(|_| git(&["rev-list", "--left-right", "--count", "HEAD...@{u}"]).ok());
    let describe = git(&["describe", "--tags", "--long", "--always"]).unwrap_or_else(|_| head_short.clone());
    let status = git(&["status", "--porcelain", "--untracked-files=no"])?;
    Ok(RepositoryState {
        branch,
        head,
        head_short,
        upstream,
        ahead_behind,
        describe,
        tracked_dirty: !status.trim().is_empty(),
        active_cairn_changes: active_dirs(Path::new("cairn/changes"))?,
        active_openspec_changes: active_dirs(Path::new("openspec/changes"))?,
    })
}

fn active_dirs(path: &Path) -> Result<Vec<String>, String> {
    if !path.is_dir() {
        return Ok(Vec::new());
    }
    let mut dirs = Vec::new();
    for entry in fs::read_dir(path).map_err(|error| format!("failed to read {}: {error}", path.display()))? {
        let entry = entry.map_err(|error| format!("failed to read {} entry: {error}", path.display()))?;
        if entry.path().is_dir() {
            dirs.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    dirs.sort();
    Ok(dirs)
}

fn git(args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|error| format!("failed to run git {}: {error}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!("git {} failed: {}", args.join(" "), String::from_utf8_lossy(&output.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn parse_args() -> Result<Config, String> {
    let mut result_dir = PathBuf::from(DEFAULT_RESULT_DIR);
    let mut out_dir = PathBuf::from(DEFAULT_OUT_DIR);
    let mut allow_dirty = false;
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--result-dir" => result_dir = PathBuf::from(args.next().ok_or("--result-dir requires a value")?),
            "--out-dir" => out_dir = PathBuf::from(args.next().ok_or("--out-dir requires a value")?),
            "--allow-dirty" => allow_dirty = true,
            "--help" | "-h" => {
                println!(
                    "usage: check-current-head-release-evidence.rs [--result-dir DIR] [--out-dir DIR] [--allow-dirty]"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(Config {
        result_dir,
        out_dir,
        allow_dirty,
    })
}

fn render_markdown(index: &IndexReceipt) -> String {
    let mut out = String::new();
    out.push_str("# Clankers current-HEAD release evidence index\n\n");
    out.push_str("## Payload\n\n");
    out.push_str(&format!("- schema: `{}`\n", index.schema));
    out.push_str(&format!("- branch: `{}`\n", index.repository.branch));
    out.push_str(&format!("- head: `{}`\n", index.repository.head));
    out.push_str(&format!("- describe: `{}`\n", index.repository.describe));
    out.push_str(&format!("- tracked_dirty: `{}`\n", index.repository.tracked_dirty));
    if let Some(upstream) = &index.repository.upstream {
        out.push_str(&format!("- upstream: `{upstream}`\n"));
    }
    if let Some(ahead_behind) = &index.repository.ahead_behind {
        out.push_str(&format!("- ahead_behind: `{ahead_behind}`\n"));
    }
    out.push_str("\n## Lifecycle\n\n");
    out.push_str(&format!("- active_cairn_changes: `{}`\n", join_or_none(&index.repository.active_cairn_changes)));
    out.push_str(&format!(
        "- active_openspec_changes: `{}`\n",
        join_or_none(&index.repository.active_openspec_changes)
    ));
    out.push_str("\n## Selected receipts\n\n");
    if index.selected_receipts.is_empty() {
        out.push_str("- none\n");
    } else {
        for (mode, receipt) in &index.selected_receipts {
            out.push_str(&format!(
                "- `{mode}`: run `{}` finished `{}` passed={} skipped={} payload_commit_verified={}\n",
                receipt.run_id, receipt.finished_at, receipt.passed, receipt.skipped, receipt.payload_commit_verified
            ));
            out.push_str(&format!("  - note: {}\n", receipt.note));
            out.push_str(&format!("  - summary: `{}` blake3 `{}`\n", receipt.summary.path, receipt.summary.blake3));
            out.push_str(&format!("  - results: `{}` blake3 `{}`\n", receipt.results.path, receipt.results.blake3));
        }
    }
    out.push_str("\n## Missing evidence modes\n\n");
    if index.missing_modes.is_empty() {
        out.push_str("- none\n");
    } else {
        for mode in &index.missing_modes {
            out.push_str(&format!("- `{mode}`\n"));
        }
    }
    out.push_str("\n## Rejected receipts\n\n");
    if index.rejected_receipts.is_empty() {
        out.push_str("- none\n");
    } else {
        for rejected in &index.rejected_receipts {
            out.push_str(&format!("- `{}`: {}\n", rejected.path, rejected.reason));
        }
    }
    out.push_str("\n## Non-claims\n\n");
    for claim in &index.non_claims {
        out.push_str(&format!("- {claim}\n"));
    }
    out
}

fn join_or_none(items: &[String]) -> String {
    if items.is_empty() {
        "none".to_string()
    } else {
        items.join(",")
    }
}

fn required_str<'a>(json: &'a Value, field: &str) -> Result<&'a str, String> {
    json.get(field).and_then(Value::as_str).ok_or_else(|| format!("missing string field `{field}`"))
}

fn required_u64(json: &Value, field: &str) -> Result<u64, String> {
    json.get(field).and_then(Value::as_u64).ok_or_else(|| format!("missing integer field `{field}`"))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| format!("failed to encode JSON: {error}"))?;
    let mut with_newline = bytes;
    with_newline.push(b'\n');
    fs::write(path, with_newline).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
