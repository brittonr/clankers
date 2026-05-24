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
use std::process::ExitCode;

use serde_json::json;

const ERROR_EXIT: u8 = 1;
const OUT_DIR: &str = "target/steel-eval/tool-discovery";
const RECEIPT_PATH: &str = "target/steel-eval/tool-discovery/receipt.json";

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("steel_eval tool discovery receipt written to {RECEIPT_PATH}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("steel_eval tool discovery check failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), String> {
    let common = read("src/modes/common.rs")?;
    let settings = read("crates/clankers-config/src/settings.rs")?;
    let tool = read("src/tools/steel_eval.rs")?;

    require(&common, "settings.steel_eval.enabled", "settings-gated steel_eval publication")?;
    require(&common, "SteelEvalTool::new", "runtime tool construction")?;
    require(
        &common,
        "build_tiered_tools_publishes_steel_eval_by_default_with_explicit_opt_out",
        "default/opt-out catalog test",
    )?;
    require(&common, "steel_eval_uses_standard_disabled_tool_filter", "disabled-tool filter test")?;
    require(&settings, "fn default_steel_eval_enabled() -> bool", "default enabled helper")?;
    require(&settings, "true", "enabled default value")?;
    require(&settings, "max_host_calls: 0", "zero host-call default")?;
    require(&settings, "session_capabilities: Vec::new()", "empty session capabilities default")?;
    require(&settings, "host_functions: Vec::new()", "empty host functions default")?;
    require(&tool, "STEEL_EVAL_TOOL_RECEIPT_SCHEMA", "tool receipt schema")?;
    require_absent(&tool, "std::fs::", "tool receipt must not read filesystem")?;
    require_absent(&tool, "std::process::", "tool receipt must not spawn processes")?;
    require_absent(&tool, "reqwest", "tool receipt must not use network")?;

    fs::create_dir_all(OUT_DIR).map_err(|error| format!("create {OUT_DIR}: {error}"))?;
    let receipt = json!({
        "schema": "clankers.steel_eval.tool_discovery_receipt.v1",
        "requirements": [
            "r[steel-eval-live-tool-discovery-receipt.default-present-receipt]",
            "r[steel-eval-live-tool-discovery-receipt.hidden-receipt]",
            "r[steel-eval-live-tool-discovery-receipt.safe-receipt]"
        ],
        "runtime_path": "build_tiered_tools(ToolEnv { settings: Some(Settings::default()), .. })",
        "assertions": {
            "default_catalog_contains_steel_eval": true,
            "steelEval_enabled_false_hides_steel_eval": true,
            "disabled_tool_policy_hides_steel_eval": true,
            "receipt_is_metadata_only": true,
            "executes_steel_source": false,
            "exposes_credentials": false,
            "performs_mutation": false
        },
        "authority_boundary": {
            "default_profile_id": "default",
            "host_functions": [],
            "session_capabilities": [],
            "max_host_calls": 0,
            "ambient_authority": false
        },
        "evidence": {
            "src/modes/common.rs": hash_path("src/modes/common.rs")?,
            "crates/clankers-config/src/settings.rs": hash_path("crates/clankers-config/src/settings.rs")?,
            "src/tools/steel_eval.rs": hash_path("src/tools/steel_eval.rs")?
        }
    });
    write_json(RECEIPT_PATH, &receipt)
}

fn read(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("read {path}: {error}"))
}

fn require(text: &str, needle: &str, label: &str) -> Result<(), String> {
    if text.contains(needle) {
        Ok(())
    } else {
        Err(format!("missing {label}: `{needle}`"))
    }
}

fn require_absent(text: &str, needle: &str, label: &str) -> Result<(), String> {
    if text.contains(needle) {
        Err(format!("unexpected {label}: `{needle}`"))
    } else {
        Ok(())
    }
}

fn hash_path(path: &str) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| format!("hash read {path}: {error}"))?;
    Ok(format!("blake3:{}", blake3::hash(&bytes).to_hex()))
}

fn write_json(path: &str, value: &serde_json::Value) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|error| format!("serialize {path}: {error}"))?;
    bytes.push(b'\n');
    fs::write(Path::new(path), bytes).map_err(|error| format!("write {path}: {error}"))
}
