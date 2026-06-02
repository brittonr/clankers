#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;
use std::process::ExitCode;

const ERROR_EXIT: u8 = 1;
const RUNTIME_CONTRACTS: &str = "crates/clankers-runtime/src/process_jobs.rs";
const TOOL: &str = "src/tools/process.rs";
const TOOL_ADAPTER: &str = "src/tools/process/adapter.rs";
const TOOL_PUEUE: &str = "src/tools/process/pueue.rs";

const RUNTIME_MARKERS: &[&str] = &[
    "pub enum ProcessJobToolRequest",
    "pub enum ProcessJobToolResult",
    "pub struct ProcessJobToolReceipt",
    "pub struct ProcessJobReceipt",
    "pub struct StartProcessJobRequest",
    "pub trait ProcessJobService",
    "process_job_tool_request_maps_to_operation_vocabulary",
    "process_job_tool_receipt_serialization_golden_fixtures",
];

const ADAPTER_MARKERS: &[&str] = &[
    "struct ProcessToolJsonAdapter",
    "fn process_job_tool_request(params: &Value) -> Result<ProcessJobToolRequest, ToolResult>",
    "fn start_request",
    "fn adopt_request",
    "fn process_job_filter_request",
    "fn process_job_log_range",
];

const TOOL_MARKERS: &[&str] = &[
    "mod adapter;",
    "ProcessToolJsonAdapter::process_job_tool_request(params)",
    "struct NativeProcessJobService",
    "mod pueue;",
    "struct SystemdProcessJobService",
    "fn stored_record_from_entry",
    "fn stored_record_summary",
    "fn apply_process_job_retention",
    "ProcessJobToolReceipt",
    "ProcessJobToolResult",
    "process_parser_produces_backend_neutral_request_dtos_for_all_actions",
    "native_admission_limit_rejects_at_capacity_with_typed_receipt",
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("ok: process-job boundary rail passed");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("process-job boundary rail failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), String> {
    let runtime = read(RUNTIME_CONTRACTS)?;
    for marker in RUNTIME_MARKERS {
        require_contains(&runtime, marker, RUNTIME_CONTRACTS)?;
    }
    for forbidden in ["clankers_db::", "tokio::process::Command", "std::process::Command"] {
        if runtime.contains(forbidden) {
            return Err(format!("{RUNTIME_CONTRACTS} contains adapter/backend token `{forbidden}`"));
        }
    }

    let adapter = read(TOOL_ADAPTER)?;
    for marker in ADAPTER_MARKERS {
        require_contains(&adapter, marker, TOOL_ADAPTER)?;
    }
    for forbidden in ["tokio::process", "clankers_db::", "ProcessJobReceipt", "StoredProcessJob"] {
        if adapter.contains(forbidden) {
            return Err(format!("{TOOL_ADAPTER} should parse JSON into typed requests only, but contains `{forbidden}`"));
        }
    }

    let tool = read(TOOL)?;
    for marker in TOOL_MARKERS {
        require_contains(&tool, marker, TOOL)?;
    }
    if tool.contains("params.get(\"command\")") && !tool.contains("ProcessToolJsonAdapter::process_job_tool_request(params)") {
        return Err(format!("{TOOL} parses process JSON directly instead of routing through {TOOL_ADAPTER}"));
    }

    let pueue = read(TOOL_PUEUE)?;
    for marker in ["trait PueueRunner", "struct PueueProcessJobService", "fn parse_pueue_tasks", "fn parse_pueue_log_text"] {
        require_contains(&pueue, marker, TOOL_PUEUE)?;
    }

    Ok(())
}

fn read(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("failed to read {path}: {error}"))
}

fn require_contains(text: &str, marker: &str, path: &str) -> Result<(), String> {
    if text.contains(marker) {
        Ok(())
    } else {
        Err(format!("{path} missing required marker `{marker}`"))
    }
}
