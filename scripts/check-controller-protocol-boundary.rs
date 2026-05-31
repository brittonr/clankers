#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;
use std::process::ExitCode;

const ERROR_EXIT: u8 = 1;
const COMMAND: &str = "crates/clankers-controller/src/command.rs";
const DOMAIN_EVENT: &str = "crates/clankers-controller/src/domain_event.rs";
const CONVERT: &str = "crates/clankers-controller/src/convert.rs";
const CORE_EFFECTS: &str = "crates/clankers-controller/src/core_effects.rs";
const EFFECT_INTERPRETATION: &str = "crates/clankers-controller/src/effect_interpretation.rs";
const TRANSPORT_CONVERT: &str = "crates/clankers-controller/src/transport_convert.rs";

const COMMAND_MARKERS: &[&str] = &[
    "CoreInput::SetThinkingLevel",
    "CoreInput::CycleThinkingLevel",
    "CoreInput::SetDisabledTools",
    "CoreInput::PromptRequested",
    "CoreInput::PromptCompleted",
    "semantic_event_to_daemon_event(&event)",
    "runtime_adapter_fixture_covers_prompt_control_identity_and_semantic_projection",
    "thinking_effects_remain_core_owned",
    "disabled_tool_effects_remain_core_owned",
];

const DOMAIN_MARKERS: &[&str] = &[
    "fn agent_event_to_domain_event",
    "fn tool_content_to_domain_parts",
    "ControllerDomainEvent::",
    "DomainImage",
];

const CONVERT_MARKERS: &[&str] = &[
    "agent_event_to_domain_event(event).and_then",
    "pub fn semantic_event_to_daemon_event",
    "pub fn daemon_event_to_tui_event",
];

const CORE_EFFECT_MARKERS: &[&str] = &[
    "execute_prompt_request_effects",
    "execute_prompt_completion_effects",
    "execute_thinking_effects",
    "execute_tool_filter_request_effects",
    "execute_tool_filter_feedback_effects",
];

const INTERPRET_MARKERS: &[&str] = &[
    "fn interpret_prompt_request",
    "fn interpret_thinking_change",
    "fn interpret_tool_filter_application",
    "fn disabled_tools_changed",
];

const TRANSPORT_MARKERS: &[&str] = &[
    "pub fn control_created",
    "pub fn control_attached",
    "pub fn session_info_event",
    "pub fn session_summary",
    "pub fn attach_ok",
    "pub fn attach_error",
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("ok: controller/protocol boundary rail passed");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("controller/protocol boundary rail failed: {error}");
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), String> {
    require_markers(COMMAND, COMMAND_MARKERS)?;
    require_markers(DOMAIN_EVENT, DOMAIN_MARKERS)?;
    require_markers(CONVERT, CONVERT_MARKERS)?;
    require_markers(CORE_EFFECTS, CORE_EFFECT_MARKERS)?;
    require_markers(EFFECT_INTERPRETATION, INTERPRET_MARKERS)?;
    require_markers(TRANSPORT_CONVERT, TRANSPORT_MARKERS)?;

    let event_processing = read("crates/clankers-controller/src/event_processing.rs")?;
    require_contains(
        &event_processing,
        "use crate::convert::agent_event_to_daemon_event;",
        "crates/clankers-controller/src/event_processing.rs",
    )?;
    require_contains(
        &event_processing,
        "agent_event_to_daemon_event(event)",
        "crates/clankers-controller/src/event_processing.rs",
    )?;
    let event_processing_runtime = event_processing.split("#[cfg(test)]").next().unwrap_or(&event_processing);
    if event_processing_runtime.contains("DaemonEvent::TextDelta") || event_processing_runtime.contains("DaemonEvent::ToolCall") {
        return Err("event_processing.rs should call the shared converter instead of constructing behavior events".into());
    }

    Ok(())
}

fn require_markers(path: &str, markers: &[&str]) -> Result<(), String> {
    let text = read(path)?;
    for marker in markers {
        require_contains(&text, marker, path)?;
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
