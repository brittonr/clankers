#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;
use std::path::Path;
use std::process::ExitCode;

const ERROR_EXIT: u8 = 1;
const FORBIDDEN_COMPAT_FILES: &[&str] = &[
    "src/agent.rs",
    "src/agent/mod.rs",
    "src/config.rs",
    "src/config/mod.rs",
    "src/provider.rs",
    "src/provider/mod.rs",
    "src/util.rs",
    "src/util/mod.rs",
];

const ROOT_FORBIDDEN: &[&str] = &[
    "pub mod agent;",
    "pub mod config;",
    "pub mod provider;",
    "pub mod util;",
    "pub use clankers_db as db;",
    "pub use clanker_message as message;",
    "pub use clankers_model_selection as model_selection;",
    "pub use clankers_procmon as procmon;",
    "pub use clankers_session;",
    "pub use clankers_tui as tui;",
];

const FORBIDDEN_PATHS: &[&str] = &[
    "crate::agent::",
    "crate::config::",
    "crate::provider::",
    "crate::util::",
    "crate::tui::",
    "crate::db::",
    "crate::message::",
    "crate::model_selection::",
    "crate::procmon::",
    "clankers::agent::",
    "clankers::config::",
    "clankers::provider::",
    "clankers::util::",
    "clankers::tui::",
    "clankers::db::",
    "clankers::message::",
    "clankers::model_selection::",
    "clankers::procmon::",
    "clankers::clankers_session::",
];

const FORBIDDEN_PLUGIN_REEXPORT_PATHS: &[&str] = &[
    "crate::plugin::PluginManager",
    "crate::plugin::PluginState",
    "crate::plugin::PluginRuntimeMode",
    "crate::plugin::PluginHostFacade",
    "crate::plugin::PluginInfo",
    "crate::plugin::StdioHostEvent",
    "crate::plugin::StdioToolCallEvent",
    "crate::plugin::bridge::",
    "crate::plugin::ui::",
    "crate::plugin::sandbox::",
    "crate::plugin::hooks::",
    "crate::plugin::host::",
    "crate::plugin::manifest::",
    "crate::plugin::registry",
    "crate::plugin::enable_plugin",
    "crate::plugin::reload_plugin",
    "crate::plugin::reload_all_plugins",
    "crate::plugin::shutdown_plugin_runtime",
    "crate::plugin::configure_stdio_runtime",
    "crate::plugin::start_stdio_plugins",
    "crate::plugin::start_stdio_tool_call",
    "crate::plugin::cancel_stdio_tool_call",
    "crate::plugin::abandon_stdio_tool_call",
    "crate::plugin::drain_stdio_host_events",
    "crate::plugin::send_stdio_event",
    "clankers::plugin::",
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("ok: root compatibility reexport rail passed");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("root compatibility reexport rail failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), String> {
    for path in FORBIDDEN_COMPAT_FILES {
        if Path::new(path).exists() {
            return Err(format!("compatibility wrapper file still exists: {path}"));
        }
    }

    let lib = read("src/lib.rs")?;
    for marker in ROOT_FORBIDDEN {
        if lib.contains(marker) {
            return Err(format!("src/lib.rs still exposes compatibility marker `{marker}`"));
        }
    }

    let plugin_mod = read("src/plugin/mod.rs")?;
    if plugin_mod.contains("pub use clankers_plugin::") {
        return Err("src/plugin/mod.rs should not re-export clankers-plugin symbols".into());
    }
    require_contains(&plugin_mod, "pub mod contributions;", "src/plugin/mod.rs")?;
    require_contains(&plugin_mod, "build_protocol_plugin_summaries", "src/plugin/mod.rs")?;

    let session_mod = read("src/session/mod.rs")?;
    if session_mod.contains("pub use clankers_session") {
        return Err("src/session/mod.rs should not re-export clankers-session symbols".into());
    }
    require_contains(&session_mod, "pub mod merge_view;", "src/session/mod.rs")?;

    let mut violations = Vec::new();
    scan_rs_tree(Path::new("src"), &mut violations)?;
    scan_rs_tree(Path::new("tests"), &mut violations)?;
    if !violations.is_empty() {
        return Err(format!("forbidden compatibility paths remain:\n{}", violations.join("\n")));
    }

    Ok(())
}

fn scan_rs_tree(path: &Path, violations: &mut Vec<String>) -> Result<(), String> {
    if path.is_dir() {
        for entry in fs::read_dir(path).map_err(|error| format!("failed to read {}: {error}", path.display()))? {
            let entry = entry.map_err(|error| format!("failed to read entry in {}: {error}", path.display()))?;
            scan_rs_tree(&entry.path(), violations)?;
        }
        return Ok(());
    }

    if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
        return Ok(());
    }

    let display = path.display().to_string();
    if display == "src/lib.rs" || display.starts_with("src/agent") || display.starts_with("src/config") || display.starts_with("src/provider") || display.starts_with("src/util") {
        return Ok(());
    }

    let text = fs::read_to_string(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    for marker in FORBIDDEN_PATHS.iter().chain(FORBIDDEN_PLUGIN_REEXPORT_PATHS.iter()) {
        if text.contains(marker) {
            violations.push(format!("{} contains `{marker}`", path.display()));
        }
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
