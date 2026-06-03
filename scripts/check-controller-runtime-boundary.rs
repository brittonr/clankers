#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;
use std::path::Path;
use std::process::ExitCode;

const ERROR_EXIT: u8 = 1;

struct SourceCheck {
    id: &'static str,
    path: &'static str,
    owner: &'static str,
    required: &'static [&'static str],
    forbidden: &'static [&'static str],
}

const CHECKS: &[SourceCheck] = &[
    SourceCheck {
        id: "controller-field-inventory",
        path: "crates/clankers-controller/src/lib.rs",
        owner: "SessionController compatibility shell",
        required: &[
            "pub struct SessionController",
            "pub(crate) agent: Option<Agent>",
            "pub session_manager: Option<SessionManager>",
            "hook_pipeline: Option<Arc<HookPipeline>>",
            "outgoing: Vec<DaemonEvent>",
            "search_index: Option<Arc<clankers_db::search_index::SearchIndex>>",
            "Production command handling must wrap this value in",
            "AgentBackedRuntimeAdapter",
        ],
        forbidden: &[],
    },
    SourceCheck {
        id: "runtime-adapter-owner",
        path: "crates/clankers-controller/src/runtime_adapter.rs",
        owner: "ControllerRuntimeAdapter",
        required: &[
            "pub trait ControllerRuntimeAdapter",
            "pub struct AgentBackedRuntimeAdapter",
            "pub struct FakeRuntimeAdapter",
            "fn submit_prompt(&mut self, request: RuntimePromptRequest)",
            "fn apply_control(&mut self, request: RuntimeControlRequest)",
            "fake_runtime_adapter_records_prompts_and_controls_without_desktop_services",
            "agent_backed_runtime_adapter_projects_agent_prompt_events_and_completion",
        ],
        forbidden: &["clankers_session::SessionManager", "SearchIndex"],
    },
    SourceCheck {
        id: "command-runtime-adapter-path",
        path: "crates/clankers-controller/src/command.rs",
        owner: "command policy runtime seam",
        required: &[
            "submit_prompt_with_runtime_adapter",
            "handle_command_with_runtime_adapter_for_test",
            "apply_control_with_runtime_adapter",
            "RuntimeControlRequest::Abort",
            "RuntimeControlRequest::SetThinkingLevel",
            "RuntimeControlRequest::SetDisabledTools",
            "fake_runtime_command_fixture_records_prompt_controls_and_session_identity",
            "runtime_adapter_fixture_covers_prompt_control_identity_and_semantic_projection",
        ],
        forbidden: &["CompletionRequest {", "EngineModelRequest {"],
    },
    SourceCheck {
        id: "persistence-compat-owner",
        path: "crates/clankers-controller/src/persistence.rs",
        owner: "controller persistence compatibility adapter",
        required: &[
            "use clankers_session::SessionManager",
            "fn persist_messages",
            "index_messages_for_search",
            "persist_compaction_summary_tool_result",
        ],
        forbidden: &[],
    },
    SourceCheck {
        id: "projection-owner",
        path: "crates/clankers-controller/src/convert.rs",
        owner: "controller projection conversion module",
        required: &[
            "agent_event_to_daemon_event",
            "semantic_event_to_daemon_event",
            "daemon_event_to_tui_event",
            "semantic_event_to_tui_event",
            "agent_message_to_tui_events",
        ],
        forbidden: &[],
    },
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("ok: controller runtime boundary covers {} owners", CHECKS.len());
            ExitCode::SUCCESS
        }
        Err(errors) => {
            for error in errors {
                eprintln!("controller runtime boundary error: {error}");
            }
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    for check in CHECKS {
        validate_source(check, &mut errors);
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

fn validate_source(check: &SourceCheck, errors: &mut Vec<String>) {
    let path = Path::new(check.path);
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            errors.push(format!("{} failed to read {}: {error}", check.id, check.path));
            return;
        }
    };
    for marker in check.required {
        if !source.contains(marker) {
            errors.push(format!("{} ({}) missing marker {:?} in {}", check.id, check.owner, marker, check.path));
        }
    }
    for marker in check.forbidden {
        if source.contains(marker) {
            errors.push(format!("{} ({}) contains forbidden marker {:?} in {}", check.id, check.owner, marker, check.path));
        }
    }
}
