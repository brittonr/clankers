#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;
use std::path::Path;
use std::process::ExitCode;

const ERROR_EXIT: u8 = 1;
const SELECTED_KIT: &str = "session-ledger-resume";

struct SourceCheck {
    id: &'static str,
    path: &'static str,
    support_label: &'static str,
    required: &'static [&'static str],
    forbidden: &'static [&'static str],
}

const UNRELATED_SURFACES: &[&str] = &[
    "clankers_provider",
    "clanker_router",
    "clankers_plugin",
    "clanker_tui_types",
    "clankers_protocol",
    "openai",
    "OAuth",
    "PluginManager",
    "DaemonEvent",
];

const CHECKS: &[SourceCheck] = &[
    SourceCheck {
        id: "runtime-public-facade-labels",
        path: "crates/clankers-runtime/src/lib.rs",
        support_label: "yellow composition facade with green selected kit reexports",
        required: &[
            "pub use ledger::SessionLedgerEntry",
            "pub use ledger::SessionLedgerMessage",
            "pub use ledger::SessionLedgerRecord",
            "pub use ledger::SessionLedgerReplay",
            "pub use runtime::Runtime",
            "pub use services::RuntimeServices",
            "session_resume_two_backends_restore_ordered_ledger_context",
            "session_resume_missing_or_unsupported_store_fails_before_model",
        ],
        forbidden: &[],
    },
    SourceCheck {
        id: "selected-kit-neutral-ledger",
        path: "crates/clankers-runtime/src/ledger.rs",
        support_label: "green SDK kit: neutral session ledger/resume DTOs",
        required: &[
            "pub enum SessionLedgerEntry",
            "pub struct SessionLedgerMessage",
            "pub struct SessionLedgerRecord",
            "pub fn replay_ledger_entries",
            "ledger_messages_from_engine_messages",
        ],
        forbidden: UNRELATED_SURFACES,
    },
    SourceCheck {
        id: "selected-kit-session-runtime",
        path: "crates/clankers-runtime/src/session.rs",
        support_label: "green SDK kit runtime: host-owned resume execution",
        required: &[
            "pub(crate) fn resume",
            "RuntimeError::SessionMissing",
            "record.replay()?.messages",
            "replace_record_replay_messages",
            "RuntimeError::SessionUnsupported",
        ],
        forbidden: &["clankers_session", "SessionManager", "DaemonEvent", "TuiEvent", "PluginManager"],
    },
    SourceCheck {
        id: "fail-closed-session-service-defaults",
        path: "crates/clankers-runtime/src/services.rs",
        support_label: "yellow host services: disabled defaults fail closed",
        required: &[
            "pub trait SessionStore",
            "pub struct DisabledSessionStore",
            "RuntimeError::SessionUnsupported(\"session store disabled\".to_string())",
            "pub struct InMemorySessionStore",
        ],
        forbidden: &["ClankersPaths::get", "auth.json", "PluginManager"],
    },
    SourceCheck {
        id: "selected-kit-docs",
        path: "docs/src/tutorials/embedded-agent-sdk.md",
        support_label: "docs support label for session ledger kit",
        required: &[
            "Runtime::resume_session",
            "SessionLedgerEntry",
            "SessionLedgerMessage",
            "resume-required sessions fail closed",
        ],
        forbidden: &[],
    },
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("ok: runtime facade split selected kit `{SELECTED_KIT}` covers {} surfaces", CHECKS.len());
            ExitCode::SUCCESS
        }
        Err(errors) => {
            for error in errors {
                eprintln!("runtime facade split error: {error}");
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
            errors.push(format!("{} ({}) missing marker {:?} in {}", check.id, check.support_label, marker, check.path));
        }
    }
    for marker in check.forbidden {
        if source.contains(marker) {
            errors.push(format!("{} ({}) contains forbidden marker {:?} in {}", check.id, check.support_label, marker, check.path));
        }
    }
}
