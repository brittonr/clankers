#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;
use std::process::ExitCode;

const ERROR_EXIT: u8 = 1;
const EFFECTS: &str = "src/slash_commands/effects.rs";
const ATTACH_COMMANDS: &str = "src/modes/attach/commands.rs";
const SESSION_POLICY: &str = "src/modes/session_command_policy.rs";

const EFFECT_MARKERS: &[&str] = &[
    "enum SlashEffect",
    "enum SlashUiEffect",
    "enum SlashPluginEffect",
    "fn attach_client_effects",
    "fn plugin_list_effect",
    "fn forward_to_daemon_effect",
    "fn agent_command_effect",
    "fn apply_standalone_slash_effect",
    "standalone_interpreter_applies_ui_session_and_noop_effects",
];

const ATTACH_MARKERS: &[&str] = &[
    "apply_attach_slash_effects",
    "apply_attach_slash_effect",
    "slash_commands::effects::attach_client_effects",
    "slash_commands::effects::plugin_list_effect",
    "slash_commands::effects::forward_to_daemon_effect",
    "slash_commands::effects::agent_command_effect",
];

const SESSION_POLICY_MARKERS: &[&str] = &[
    "struct SessionCommandEffect",
    "fn set_thinking_level_effect",
    "fn cycle_thinking_level_effect",
    "fn disabled_tools_effect",
    "fn manual_compaction_effect",
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("ok: slash effect boundary rail passed");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("slash effect boundary rail failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), String> {
    let effects = read(EFFECTS)?;
    for marker in EFFECT_MARKERS {
        require_contains(&effects, marker, EFFECTS)?;
    }
    for marker in ["SlashEffect::Ui", "SlashEffect::Session", "SlashEffect::Plugin", "SlashEffect::Noop"] {
        require_contains(&effects, marker, EFFECTS)?;
    }

    let attach = read(ATTACH_COMMANDS)?;
    for marker in ATTACH_MARKERS {
        require_contains(&attach, marker, ATTACH_COMMANDS)?;
    }
    require_absent(
        &attach,
        "SessionCommand::SlashCommand {\n                command: command.to_string(),",
        ATTACH_COMMANDS,
        "attach slash forwarding should be declared through slash effects",
    )?;

    let policy = read(SESSION_POLICY)?;
    for marker in SESSION_POLICY_MARKERS {
        require_contains(&policy, marker, SESSION_POLICY)?;
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

fn require_absent(text: &str, marker: &str, path: &str, reason: &str) -> Result<(), String> {
    if text.contains(marker) {
        Err(format!("{path} contains forbidden marker `{marker}`: {reason}"))
    } else {
        Ok(())
    }
}
