#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;

const ERROR_EXIT: u8 = 1;
const INVENTORY_PATH: &str = "docs/src/generated/embedded-sdk-api.md";
const GUIDE_PATH: &str = "docs/src/tutorials/embedded-agent-sdk.md";
const MESSAGE_COMPAT_SOURCE: &str = "crates/clanker-message/src/message.rs";
const TRANSCRIPT_SOURCE: &str = "crates/clanker-message/src/transcript.rs";

const REQUIRED_ROWS: &[ExpectedRow] = &[
    ExpectedRow::new("content", "module", "supported", "crates/clanker-message/src/lib.rs"),
    ExpectedRow::new("message", "module", "unsupported-internal", "crates/clanker-message/src/lib.rs"),
    ExpectedRow::new("transcript", "module", "unsupported-internal", "crates/clanker-message/src/lib.rs"),
    ExpectedRow::new("Content", "enum", "supported", "crates/clanker-message/src/content.rs"),
    ExpectedRow::new("ImageSource", "enum", "supported", "crates/clanker-message/src/content.rs"),
    ExpectedRow::new("StopReason", "enum", "supported", "crates/clanker-message/src/content.rs"),
    ExpectedRow::new("ToolDefinition", "struct", "supported", "crates/clanker-message/src/contracts.rs"),
    ExpectedRow::new("ThinkingConfig", "struct", "supported", "crates/clanker-message/src/contracts.rs"),
    ExpectedRow::new("ThinkingLevel", "enum", "supported", "crates/clanker-message/src/contracts.rs"),
    ExpectedRow::new("Usage", "struct", "supported", "crates/clanker-message/src/contracts.rs"),
    ExpectedRow::new("SemanticEvent", "enum", "supported", "crates/clanker-message/src/semantic_event.rs"),
    ExpectedRow::new("SemanticEventMetadata", "struct", "supported", "crates/clanker-message/src/semantic_event.rs"),
    ExpectedRow::new("StreamEvent", "enum", "optional-support", "crates/clanker-message/src/streaming.rs"),
    ExpectedRow::new("ContentDelta", "enum", "optional-support", "crates/clanker-message/src/streaming.rs"),
    ExpectedRow::new("ToolResult", "struct", "optional-support", "crates/clanker-message/src/tool_result.rs"),
    ExpectedRow::new("AgentMessage", "enum", "unsupported-internal", "crates/clanker-message/src/transcript.rs"),
    ExpectedRow::new("MessageId", "struct", "unsupported-internal", "crates/clanker-message/src/transcript.rs"),
    ExpectedRow::new("generate_id", "function", "unsupported-internal", "crates/clanker-message/src/transcript.rs"),
    ExpectedRow::new(
        "BashExecutionMessage",
        "struct",
        "unsupported-internal",
        "crates/clanker-message/src/transcript.rs",
    ),
    ExpectedRow::new(
        "BranchSummaryMessage",
        "struct",
        "unsupported-internal",
        "crates/clanker-message/src/transcript.rs",
    ),
    ExpectedRow::new(
        "CompactionSummaryMessage",
        "struct",
        "unsupported-internal",
        "crates/clanker-message/src/transcript.rs",
    ),
    ExpectedRow::new("CustomMessage", "struct", "unsupported-internal", "crates/clanker-message/src/transcript.rs"),
    ExpectedRow::new("UserMessage", "struct", "experimental", "crates/clanker-message/src/transcript.rs"),
    ExpectedRow::new("AssistantMessage", "struct", "experimental", "crates/clanker-message/src/transcript.rs"),
    ExpectedRow::new("ToolResultMessage", "struct", "experimental", "crates/clanker-message/src/transcript.rs"),
];

const GREEN_PUBLIC_API_ROOTS: &[&str] = &[
    "crates/clankers-engine/src",
    "crates/clankers-engine-host/src",
    "crates/clankers-tool-host/src",
    "crates/clankers-adapters/src",
];

const FORBIDDEN_GREEN_PUBLIC_TOKENS: &[&str] = &[
    "AgentMessage",
    "MessageId",
    "UserMessage",
    "AssistantMessage",
    "ToolResultMessage",
    "BashExecutionMessage",
    "BranchSummaryMessage",
    "CompactionSummaryMessage",
    "CustomMessage",
    "generate_id",
    "chrono::",
    "DateTime<",
    "Utc",
];

const EMBEDDED_EXAMPLE_ROOTS: &[&str] = &[
    "examples/embedded-agent-sdk",
    "examples/embedded-minimal-kit",
    "examples/embedded-tool-kit",
    "examples/embedded-provider-adapter",
    "examples/embedded-session-store",
    "examples/embedded-product-workbench",
];

#[derive(Debug, Clone, Copy)]
struct ExpectedRow {
    entry: &'static str,
    kind: &'static str,
    stability: &'static str,
    source: &'static str,
}

impl ExpectedRow {
    const fn new(entry: &'static str, kind: &'static str, stability: &'static str, source: &'static str) -> Self {
        Self {
            entry,
            kind,
            stability,
            source,
        }
    }
}

#[derive(Debug, Clone)]
struct InventoryRow {
    kind: String,
    stability: String,
    source: String,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("ok: message contract boundary rail passed");
            ExitCode::SUCCESS
        }
        Err(errors) => {
            for error in errors {
                eprintln!("message contract boundary error: {error}");
            }
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    let inventory = match parse_inventory(INVENTORY_PATH) {
        Ok(inventory) => inventory,
        Err(error) => {
            errors.push(error);
            BTreeMap::new()
        }
    };
    validate_inventory_rows(&inventory, &mut errors);
    validate_message_sources(&mut errors);
    validate_guide(&mut errors);
    validate_examples_avoid_transcripts(&mut errors);
    validate_green_public_apis_avoid_transcripts(&mut errors);
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

fn parse_inventory(path: &str) -> Result<BTreeMap<String, Vec<InventoryRow>>, String> {
    let text = read(path)?;
    let mut rows: BTreeMap<String, Vec<InventoryRow>> = BTreeMap::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("| `") {
            continue;
        }
        let cells = trimmed.trim_matches('|').split('|').map(|cell| cell.trim().to_string()).collect::<Vec<_>>();
        if cells.len() != 5 {
            return Err(format!("{path} has malformed inventory row: {line}"));
        }
        let entry = strip_code(&cells[0]);
        let crate_name = strip_code(&cells[1]);
        if crate_name != "clanker-message" {
            continue;
        }
        rows.entry(entry).or_default().push(InventoryRow {
            kind: cells[2].clone(),
            stability: cells[3].clone(),
            source: strip_code(&cells[4]),
        });
    }
    Ok(rows)
}

fn validate_inventory_rows(inventory: &BTreeMap<String, Vec<InventoryRow>>, errors: &mut Vec<String>) {
    for expected in REQUIRED_ROWS {
        match inventory.get(expected.entry) {
            Some(rows)
                if rows.iter().any(|row| {
                    row.kind == expected.kind
                        && row.stability == expected.stability
                        && row.source == expected.source
                }) => {}
            Some(rows) => errors.push(format!(
                "inventory row `{}` drifted: expected kind/stability/source {}/{}/{}, found {:?}",
                expected.entry, expected.kind, expected.stability, expected.source, rows,
            )),
            None => errors.push(format!("inventory missing clanker-message row `{}`", expected.entry)),
        }
    }
}

fn validate_message_sources(errors: &mut Vec<String>) {
    let lib = read("crates/clanker-message/src/lib.rs").unwrap_or_default();
    for marker in ["pub mod content;", "pub mod transcript;", "pub mod message;"] {
        if !lib.contains(marker) {
            errors.push(format!("clanker-message lib.rs missing marker `{marker}`"));
        }
    }

    let message = read(MESSAGE_COMPAT_SOURCE).unwrap_or_default();
    for marker in [
        "Compatibility module for legacy",
        "pub use crate::content::Content;",
        "pub use crate::transcript::AgentMessage;",
    ] {
        if !message.contains(marker) {
            errors.push(format!("{MESSAGE_COMPAT_SOURCE} missing compatibility marker `{marker}`"));
        }
    }

    let transcript = read(TRANSCRIPT_SOURCE).unwrap_or_default();
    for marker in [
        "Clankers transcript compatibility records",
        "not the generic embedded SDK message contract",
        "pub enum AgentMessage",
        "pub struct MessageId",
    ] {
        if !transcript.contains(marker) {
            errors.push(format!("{TRANSCRIPT_SOURCE} missing transcript marker `{marker}`"));
        }
    }
}

fn validate_guide(errors: &mut Vec<String>) {
    let guide = read(GUIDE_PATH).unwrap_or_default();
    for marker in [
        "splits stable SDK contracts from Clankers transcript compatibility records",
        "legacy `clanker_message::message::*` module remains only as a compatibility import path",
        "Validation command: `scripts/check-message-contract-boundary.rs`",
    ] {
        if !guide.contains(marker) {
            errors.push(format!("{GUIDE_PATH} missing message-boundary marker `{marker}`"));
        }
    }
}

fn validate_examples_avoid_transcripts(errors: &mut Vec<String>) {
    for root in EMBEDDED_EXAMPLE_ROOTS {
        for path in rust_files_under(Path::new(root), errors) {
            let text = read_path(&path).unwrap_or_default();
            for token in FORBIDDEN_GREEN_PUBLIC_TOKENS {
                if text.contains(token) {
                    errors
                        .push(format!("embedded example {} uses transcript-internal token `{token}`", path.display()));
                }
            }
        }
    }
}

fn validate_green_public_apis_avoid_transcripts(errors: &mut Vec<String>) {
    for root in GREEN_PUBLIC_API_ROOTS {
        for path in rust_files_under(Path::new(root), errors) {
            let text = read_path(&path).unwrap_or_default();
            for (index, line) in text.lines().enumerate() {
                let trimmed = line.trim_start();
                if !trimmed.starts_with("pub ") {
                    continue;
                }
                for token in FORBIDDEN_GREEN_PUBLIC_TOKENS {
                    if line.contains(token) {
                        errors.push(format!(
                            "green SDK public API {}:{} exposes transcript-internal token `{token}`",
                            path.display(),
                            index + 1,
                        ));
                    }
                }
            }
        }
    }
}

fn rust_files_under(root: &Path, errors: &mut Vec<String>) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rust_files(root, &mut files, errors);
    files.sort();
    files
}

fn collect_rust_files(path: &Path, files: &mut Vec<PathBuf>, errors: &mut Vec<String>) {
    if path.is_file() {
        if path.extension() == Some(OsStr::new("rs")) {
            files.push(path.to_path_buf());
        }
        return;
    }
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) => {
            errors.push(format!("failed to read {}: {error}", path.display()));
            return;
        }
    };
    for entry in entries {
        match entry {
            Ok(entry) => collect_rust_files(&entry.path(), files, errors),
            Err(error) => errors.push(format!("failed to read dir entry under {}: {error}", path.display())),
        }
    }
}

fn read(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("failed to read {path}: {error}"))
}

fn read_path(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("failed to read {}: {error}", path.display()))
}

fn strip_code(text: &str) -> String {
    text.trim().trim_matches('`').to_string()
}
