#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;
use std::path::Path;
use std::process::ExitCode;

const ERROR_EXIT: u8 = 1;
const DISPLAY_TOKENS: &[&str] = &["clanker_tui_types", "clankers_protocol", "DaemonEvent", "TuiEvent", "PluginUiState"];

struct Responsibility {
    id: &'static str,
    path: &'static str,
    owner: &'static str,
    classification: &'static str,
    required: &'static [&'static str],
    forbidden: &'static [&'static str],
}

const RESPONSIBILITIES: &[Responsibility] = &[
    Responsibility {
        id: "manifest-schema-neutral",
        path: "crates/clankers-plugin/src/manifest.rs",
        owner: "PluginManifest",
        classification: "neutral manifest schema/validation",
        required: &["pub struct PluginManifest", "PluginKind", "validate", "stdio"],
        forbidden: DISPLAY_TOKENS,
    },
    Responsibility {
        id: "host-facade-runtime-query",
        path: "crates/clankers-plugin/src/host_facade.rs",
        owner: "PluginHostFacade",
        classification: "runtime inventory facade",
        required: &["pub struct PluginHostFacade", "summaries", "active_plugins", "PluginRuntimeSummary"],
        forbidden: &["DaemonEvent", "TuiEvent"],
    },
    Responsibility {
        id: "stdio-runtime-owner",
        path: "crates/clankers-plugin/src/stdio_runtime.rs",
        owner: "stdio supervisor/runtime",
        classification: "desktop/app-edge stdio runtime dispatch",
        required: &["start_stdio_plugin", "start_stdio_tool_call", "cancel_stdio_tool_call", "PluginKind::Stdio"],
        forbidden: &["load_wasm", "Extism"],
    },
    Responsibility {
        id: "stdio-protocol-neutral",
        path: "crates/clankers-plugin/src/stdio_protocol.rs",
        owner: "stdio JSON protocol DTOs",
        classification: "neutral runtime protocol DTOs",
        required: &["HostToPluginFrame", "PluginToHostFrame", "ToolInvoke", "ToolResult"],
        forbidden: DISPLAY_TOKENS,
    },
    Responsibility {
        id: "desktop-ui-projection-edge",
        path: "crates/clankers-plugin/src/ui.rs",
        owner: "plugin UI projection edge",
        classification: "desktop display DTO adapter",
        required: &["pub use clanker_tui_types::PluginUiState", "PluginNotification", "Widget"],
        forbidden: &[],
    },
    Responsibility {
        id: "dispatch-matrix-rail",
        path: "scripts/check-plugin-runtime-dispatch.rs",
        owner: "plugin runtime dispatch matrix rail",
        classification: "verification rail",
        required: &["REQUIRED_KINDS", "extism", "stdio", "built-in", "product-owned", "stdio_sent_to_wasm_loader"],
        forbidden: &[],
    },
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("ok: plugin runtime boundary covers {} responsibilities", RESPONSIBILITIES.len());
            ExitCode::SUCCESS
        }
        Err(errors) => {
            for error in errors {
                eprintln!("plugin runtime boundary error: {error}");
            }
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    for responsibility in RESPONSIBILITIES {
        validate_source(responsibility, &mut errors);
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

fn validate_source(responsibility: &Responsibility, errors: &mut Vec<String>) {
    let path = Path::new(responsibility.path);
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            errors.push(format!("{} failed to read {}: {error}", responsibility.id, responsibility.path));
            return;
        }
    };
    for marker in responsibility.required {
        if !source.contains(marker) {
            errors.push(format!(
                "{} ({}, {}) missing marker {:?} in {}",
                responsibility.id, responsibility.owner, responsibility.classification, marker, responsibility.path
            ));
        }
    }
    for marker in responsibility.forbidden {
        if source.contains(marker) {
            errors.push(format!(
                "{} ({}, {}) contains forbidden display/runtime marker {:?} in {}",
                responsibility.id, responsibility.owner, responsibility.classification, marker, responsibility.path
            ));
        }
    }
}
