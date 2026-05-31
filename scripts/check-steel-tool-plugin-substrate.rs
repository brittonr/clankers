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
const RUNTIME: &str = "crates/clankers-runtime/src/steel_tool_substrate.rs";
const RUNTIME_LIB: &str = "crates/clankers-runtime/src/lib.rs";
const AGENT_ADAPTER: &str = "crates/clankers-agent/src/turn/steel_tool_substrate.rs";
const EXECUTION: &str = "crates/clankers-agent/src/turn/execution.rs";
const PORTS: &str = "crates/clankers-agent/src/turn/ports.rs";
const TOOL_TRAIT: &str = "crates/clankers-agent/src/tool.rs";
const SETTINGS: &str = "crates/clankers-config/src/settings.rs";
const PLUGIN_TOOL: &str = "src/tools/plugin_tool.rs";
const SUBAGENT_TOOL: &str = "src/tools/subagent.rs";
const DELEGATE_TOOL: &str = "src/tools/delegate/mod.rs";
const ACTIVE_TASKS: &str = "cairn/changes/steel-tool-plugin-substrate/tasks.md";
const ARCHIVED_TASKS: &str = "cairn/archive/1970-01-01-steel-tool-plugin-substrate/tasks.md";
const ACTIVE_SPEC: &str = "cairn/changes/steel-tool-plugin-substrate/specs/steel-tool-plugin-substrate/spec.md";
const CANONICAL_SPEC: &str = "cairn/specs/steel-tool-plugin-substrate/spec.md";
const OUTPUT: &str = "target/steel-tool-plugin-substrate/receipt.json";

const RUNTIME_MARKERS: &[&str] = &[
    "STEEL_TOOL_SUBSTRATE_PLAN_SCHEMA",
    "STEEL_TOOL_SUBSTRATE_RECEIPT_SCHEMA",
    "DEFAULT_TOOL_SUBSTRATE_CALL_SEAM",
    "SteelToolExecutorKind",
    "RustBuiltin",
    "WasmPlugin",
    "StdioPlugin",
    "Subagent",
    "SteelToolInvocationInput",
    "SteelToolInvocationReceipt",
    "plan_tool_invocation_with_steel_or_fallback",
    "evaluate_steel_request",
    "steel_tool_plan_payload",
    "mismatched_plan_falls_back_without_authorization",
    "block_mode_blocks_denied_executor",
];

const AGENT_MARKERS: &[&str] = &[
    "AgentToolSteelSubstrateConfig",
    "steel_tool_substrate_config_from_settings",
    "authorize_tool_invocation",
    "blocked_receipt_to_tool_result",
    "steel.host.tool.call",
    "default_settings_enable_all_executor_kinds",
    "disabled_executor_is_removed_from_profile",
];

const EXECUTION_MARKERS: &[&str] = &[
    "execute_tools_parallel_with_substrate",
    "authorize_tool_invocation",
    "blocked_receipt_to_tool_result",
    "steel_tool_substrate_blocks_before_direct_tool_execution",
];

const SETTINGS_MARKERS: &[&str] = &[
    "steelToolSubstrate",
    "SteelToolSubstrateSettings",
    "SteelToolSubstrateRolloutStage",
    "SteelToolSubstrateFallbackMode",
    "default_steel_tool_substrate_enabled",
    "steel_tool_substrate_from_json_defaults_enabled_and_validates",
];

const SPEC_MARKERS: &[&str] = &[
    "rust-builtins.semantic-parity",
    "wasm-plugins.policy-preserved",
    "stdio-plugins.lifecycle-preserved",
    "subagents.lifecycle-preserved",
    "receipts.redaction",
    "verification.runtime-dogfood",
    "checker-paths.active-archive-resolution",
];

const FORBIDDEN_DIRECT_STEEL_IMPORTS: &[&str] = &["steel_core::", "steel::steel_vm", "steel_vm::"];

fn main() -> ExitCode {
    match run() {
        Ok(path) => {
            println!("steel tool/plugin/subagent substrate receipt written to {}", path.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("steel tool/plugin/subagent substrate check failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<PathBuf, String> {
    let runtime = read(RUNTIME)?;
    let runtime_lib = read(RUNTIME_LIB)?;
    let adapter = read(AGENT_ADAPTER)?;
    let execution = read(EXECUTION)?;
    let ports = read(PORTS)?;
    let tool_trait = read(TOOL_TRAIT)?;
    let settings = read(SETTINGS)?;
    let plugin_tool = read(PLUGIN_TOOL)?;
    let subagent_tool = read(SUBAGENT_TOOL)?;
    let delegate_tool = read(DELEGATE_TOOL)?;
    let task_path = existing_task_path();
    let spec_path = existing_spec_path();
    let tasks = read(task_path)?;
    let spec = read(spec_path)?;
    let mut errors = Vec::new();

    require_all(RUNTIME, &runtime, RUNTIME_MARKERS, &mut errors);
    require_all(AGENT_ADAPTER, &adapter, AGENT_MARKERS, &mut errors);
    require_all(EXECUTION, &execution, EXECUTION_MARKERS, &mut errors);
    require_all(SETTINGS, &settings, SETTINGS_MARKERS, &mut errors);
    require_all(spec_path, &spec, SPEC_MARKERS, &mut errors);
    require(RUNTIME_LIB, &runtime_lib, "pub mod steel_tool_substrate;", &mut errors);
    require(RUNTIME_LIB, &runtime_lib, "pub use steel_tool_substrate::SteelToolExecutorKind;", &mut errors);
    require(PORTS, &ports, "steel_tool_substrate: Option<AgentToolSteelSubstrateConfig>", &mut errors);
    require(TOOL_TRAIT, &tool_trait, "fn execution_backend(&self) -> ToolExecutionBackend", &mut errors);
    require(PLUGIN_TOOL, &plugin_tool, "PluginToolBackend::Wasm", &mut errors);
    require(PLUGIN_TOOL, &plugin_tool, "PluginToolBackend::Stdio", &mut errors);
    require(SUBAGENT_TOOL, &subagent_tool, "ToolExecutionBackend::Subagent", &mut errors);
    require(DELEGATE_TOOL, &delegate_tool, "ToolExecutionBackend::Subagent", &mut errors);
    require(task_path, &tasks, "V7: Run Cairn gates", &mut errors);
    forbid_all(RUNTIME, &runtime, FORBIDDEN_DIRECT_STEEL_IMPORTS, &mut errors);
    forbid_all(AGENT_ADAPTER, &adapter, FORBIDDEN_DIRECT_STEEL_IMPORTS, &mut errors);
    forbid_all(EXECUTION, &execution, FORBIDDEN_DIRECT_STEEL_IMPORTS, &mut errors);

    if !errors.is_empty() {
        return Err(errors.join("\n"));
    }

    let artifacts = [
        RUNTIME,
        RUNTIME_LIB,
        AGENT_ADAPTER,
        EXECUTION,
        PORTS,
        TOOL_TRAIT,
        SETTINGS,
        PLUGIN_TOOL,
        SUBAGENT_TOOL,
        DELEGATE_TOOL,
        task_path,
        spec_path,
        "scripts/check-steel-tool-plugin-substrate.rs",
    ]
    .iter()
    .map(|path| hash_artifact(Path::new(path)))
    .collect::<Result<Vec<_>, _>>()?;
    let receipt = json!({
        "schema": "clankers.steel_tool_plugin_subagent_substrate.receipt.v1",
        "validated_surfaces": [
            "runtime-dto-and-steel-host-call-plan",
            "agent-dispatch-adapter",
            "rust-builtin-before-execute-blocking",
            "wasm-plugin-backend-tagging",
            "stdio-plugin-backend-tagging",
            "subagent-delegate-backend-tagging",
            "settings-default-and-kill-switch",
            "cairn-spec-traceability"
        ],
        "hashed_artifacts": artifacts,
        "guidance": "Steel plans tool/plugin/subagent calls through typed receipts; Rust owns all executor effects."
    });
    let output = PathBuf::from(OUTPUT);
    let parent = output.parent().ok_or_else(|| format!("{} has no parent", output.display()))?;
    fs::create_dir_all(parent).map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    let bytes = serde_json::to_vec_pretty(&receipt).map_err(|error| format!("failed to encode receipt: {error}"))?;
    fs::write(&output, [bytes.as_slice(), b"\n"].concat())
        .map_err(|error| format!("failed to write {}: {error}", output.display()))?;
    Ok(output)
}

fn existing_task_path() -> &'static str {
    existing_path(ACTIVE_TASKS, ARCHIVED_TASKS)
}

fn existing_spec_path() -> &'static str {
    existing_path(ACTIVE_SPEC, CANONICAL_SPEC)
}

fn existing_path(active: &'static str, archived_or_canonical: &'static str) -> &'static str {
    if Path::new(active).exists() {
        active
    } else {
        archived_or_canonical
    }
}

fn read(path: &str) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("failed to read {path}: {error}"))
}

fn require(path: &str, text: &str, marker: &str, errors: &mut Vec<String>) {
    if !text.contains(marker) {
        errors.push(format!("{path} missing marker `{marker}`"));
    }
}

fn require_all(path: &str, text: &str, markers: &[&str], errors: &mut Vec<String>) {
    for marker in markers {
        require(path, text, marker, errors);
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
    Ok(json!({
        "path": path.display().to_string(),
        "blake3": format!("b3:{}", blake3::hash(&bytes).to_hex()),
    }))
}
