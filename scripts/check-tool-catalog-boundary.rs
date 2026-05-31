#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;
use std::process::ExitCode;

const ERROR_EXIT: u8 = 1;
const COMMON: &str = "src/modes/common.rs";
const CATALOG: &str = "src/modes/tool_catalog.rs";

const REQUIRED_FAMILIES: &[&str] = &[
    "family: \"core\"",
    "family: \"orchestration\"",
    "family: \"specialty\"",
    "family: \"daemon-session\"",
    "family: \"matrix\"",
    "family: \"plugin\"",
    "family: \"extension-runtime\"",
    "family: \"mcp\"",
];

const REQUIRED_BUILDERS: &[&str] = &[
    "fn build_core_tools",
    "fn build_orchestration_tools",
    "fn build_specialty_tools",
    "fn build_daemon_session_tools",
    "fn build_matrix_tools",
    "pub(crate) fn build_plugin_tools",
    "fn build_extension_runtime_tools",
    "fn build_mcp_tools",
];

const COMMON_FORBIDDEN_CONSTRUCTORS: &[&str] = &[
    "ReadTool::new",
    "WriteTool::new",
    "EditTool::new",
    "PatchTool::new",
    "ExecuteCodeTool::new",
    "ProcessTool::new",
    "BashTool::new",
    "SubagentTool::new",
    "DelegateTool::new",
    "TodoTool::new",
    "PluginTool::new",
    "PluginTool::new_stdio",
    "ValidatorTool::new",
    "build_browser_tool_from_settings",
    "build_external_memory_tool_from_settings",
    "SteelEvalTool::new",
    "build_tools_from_settings",
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("ok: tool catalog boundary rail passed");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("tool catalog boundary rail failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), String> {
    let catalog = fs::read_to_string(CATALOG).map_err(|error| format!("failed to read {CATALOG}: {error}"))?;
    for marker in REQUIRED_FAMILIES {
        require_contains(&catalog, marker, CATALOG)?;
    }
    for marker in REQUIRED_BUILDERS {
        require_contains(&catalog, marker, CATALOG)?;
    }

    let common = fs::read_to_string(COMMON).map_err(|error| format!("failed to read {COMMON}: {error}"))?;
    let runtime_common = common.split("#[cfg(test)]\nmod tests").next().unwrap_or(&common);
    require_contains(runtime_common, "crate::modes::tool_catalog::build_builtin_tiered_tools", COMMON)?;
    require_contains(runtime_common, "crate::modes::tool_catalog::build_plugin_tools", COMMON)?;
    require_contains(runtime_common, "crate::modes::tool_catalog::build_all_tiered_tools", COMMON)?;
    for token in COMMON_FORBIDDEN_CONSTRUCTORS {
        if runtime_common.contains(token) {
            return Err(format!(
                "{COMMON} runtime code still owns concrete tool constructor `{token}`; move it to {CATALOG}"
            ));
        }
    }

    Ok(())
}

fn require_contains(text: &str, marker: &str, path: &str) -> Result<(), String> {
    if text.contains(marker) {
        Ok(())
    } else {
        Err(format!("{path} missing required marker `{marker}`"))
    }
}
