#!/usr/bin/env -S nix develop -c cargo -q -Zscript
---cargo
[package]
edition = "2024"
---

use std::fs;
use std::path::Path;
use std::process::ExitCode;

const ERROR_EXIT: u8 = 1;

#[derive(Clone, Copy)]
struct BoundaryInventoryEntry {
    id: &'static str,
    path: &'static str,
    owner: &'static str,
    boundary: &'static str,
    required_markers: &'static [&'static str],
    forbidden_markers: &'static [&'static str],
}

const NEUTRAL_FORBIDDEN: &[&str] = &[
    "clankers_runtime",
    "clankers-runtime",
    "clankers_session",
    "clankers-session",
    "SessionManager",
    "JsonlSessionStore",
    "clankers_db",
    "clankers-db",
    "DaemonEvent",
    "TuiEvent",
    "ConversationBlock",
    "RuntimeError",
    "Utc::now",
];

const SDK_EXAMPLE_FORBIDDEN: &[&str] = &[
    "clankers_session",
    "clankers-session",
    "SessionManager",
    "JsonlSessionStore",
    "clankers_db",
    "clankers-db",
    "AgentMessage",
    "MessageId",
    "~/.clankers",
    "global session",
];

const INVENTORY: &[BoundaryInventoryEntry] = &[
    BoundaryInventoryEntry {
        id: "green-engine-host-ledger-dtos",
        path: "crates/clankers-engine-host/src/session_ledger.rs",
        owner: "clankers-engine-host::session_ledger",
        boundary: "green neutral ledger boundary",
        required_markers: &[
            "pub enum SessionLedgerEntry",
            "pub struct SessionLedgerMessage",
            "pub struct SessionLedgerRecord",
            "pub struct SessionLedgerReplay",
            "pub enum SessionLedgerError",
            "pub fn replay_ledger_entries",
            "ledger_messages_from_engine_messages",
            "engine_messages_from_ledger_messages",
        ],
        forbidden_markers: NEUTRAL_FORBIDDEN,
    },
    BoundaryInventoryEntry {
        id: "runtime-ledger-compat-adapter",
        path: "crates/clankers-runtime/src/ledger.rs",
        owner: "clankers-runtime::ledger",
        boundary: "runtime compatibility adapter to green ledger",
        required_markers: &[
            "pub type SessionLedgerEntry = clankers_engine_host::SessionLedgerEntry",
            "RuntimeError::SessionUnsupported",
            "clankers_engine_host::replay_ledger_entries",
        ],
        forbidden_markers: &[
            "pub struct SessionLedgerRecord",
            "Utc::now",
            "pub enum SessionLedgerEntry",
        ],
    },
    BoundaryInventoryEntry {
        id: "runtime-resume-session-path",
        path: "crates/clankers-runtime/src/session.rs",
        owner: "clankers-runtime::session",
        boundary: "neutral resume runtime",
        required_markers: &[
            "pub(crate) fn resume",
            "RuntimeError::SessionMissing",
            "record.replay()?.messages",
            "ledger_messages_from_engine_messages(&request.messages)",
            "replace_record_replay_messages",
        ],
        forbidden_markers: &["clankers_session", "SessionManager", "JsonlSessionStore", "DaemonEvent", "TuiEvent"],
    },
    BoundaryInventoryEntry {
        id: "embedded-session-store-product-dtos",
        path: "examples/embedded-session-store/src/main.rs",
        owner: "embedded-session-store example",
        boundary: "host-owned SDK store",
        required_markers: &[
            "struct ProductSession",
            "SessionLedgerMessage",
            "struct InMemoryProductSessionStore",
            "MissingSession",
            "roles_and_text",
        ],
        forbidden_markers: SDK_EXAMPLE_FORBIDDEN,
    },
    BoundaryInventoryEntry {
        id: "embedded-product-workbench-product-dtos",
        path: "examples/embedded-product-workbench/src/main.rs",
        owner: "embedded-product-workbench example",
        boundary: "host-owned product store",
        required_markers: &[
            "struct ProductSession",
            "SessionLedgerMessage",
            "struct ProductSessionStore",
            "missing_session_fails_closed_before_model_or_tool_execution",
        ],
        forbidden_markers: SDK_EXAMPLE_FORBIDDEN,
    },
    BoundaryInventoryEntry {
        id: "session-resume-brick-receipt",
        path: "scripts/check-session-resume-brick.rs",
        owner: "embedded SDK rail",
        boundary: "session ledger verification",
        required_markers: &[
            "session-resume-brick receipt",
            "FORBIDDEN_SOURCE_TOKENS",
            "run_runtime_resume_fixtures",
            "cargo",
            "session_resume",
        ],
        forbidden_markers: &[],
    },
    BoundaryInventoryEntry {
        id: "desktop-session-ledger-adapter-selected",
        path: "src/modes/session_ledger.rs",
        owner: "root session ledger adapter",
        boundary: "desktop transcript to neutral ledger adapter",
        required_markers: &[
            "desktop_messages_to_ledger_entries",
            "desktop_messages_to_serialized_seed_messages",
            "SessionLedgerEntry",
            "SessionLedgerRole::Tool",
            "AgentMessage::BranchSummary",
        ],
        forbidden_markers: &["clankers_session::SessionManager", "clankers_db", "DaemonEvent", "TuiEvent"],
    },
    BoundaryInventoryEntry {
        id: "daemon-session-builder-selected-resume-path",
        path: "src/modes/daemon/session_builder.rs",
        owner: "daemon session builder",
        boundary: "selected restore path behind neutral ledger adapter",
        required_markers: &[
            "load_recovery_seed_messages",
            "resolve_session_resume_in_dir",
            "serialize_seed_messages",
            "desktop_messages_to_serialized_seed_messages",
        ],
        forbidden_markers: &[],
    },
    BoundaryInventoryEntry {
        id: "desktop-session-store-compat",
        path: "crates/clankers-session/src/lib.rs",
        owner: "clankers-session::SessionManager",
        boundary: "desktop compatibility storage",
        required_markers: &[
            "pub struct SessionManager",
            "AgentMessage",
            "Legacy JSONL files are auto-migrated",
            "pub fn append_message",
            "load_tree",
        ],
        forbidden_markers: &[],
    },
    BoundaryInventoryEntry {
        id: "desktop-session-merge-compat",
        path: "crates/clankers-session/src/merge.rs",
        owner: "clankers-session::merge",
        boundary: "desktop branch merge storage",
        required_markers: &[
            "pub fn merge_branch",
            "pub fn merge_selective",
            "pub fn cherry_pick",
            "set_message_id",
            "self.append_message",
        ],
        forbidden_markers: &[],
    },
    BoundaryInventoryEntry {
        id: "desktop-session-setup-shell",
        path: "src/modes/session_setup.rs",
        owner: "root session setup adapter",
        boundary: "desktop setup/restore adapter",
        required_markers: &[
            "pub(crate) fn setup_session",
            "create_new_session",
            "resume_latest",
            "resume_by_id",
            "clankers_session::SessionManager::open",
        ],
        forbidden_markers: &[],
    },
    BoundaryInventoryEntry {
        id: "standalone-display-restore-parity",
        path: "src/modes/session_restore.rs",
        owner: "standalone display restore adapter",
        boundary: "desktop restore display parity",
        required_markers: &[
            "pub(crate) fn restore_display_blocks",
            "restore_display_blocks_preserves_started_at_and_finalized_hash",
            "restore_display_blocks_does_not_stamp_wall_clock_rebuild_time",
            "restore_tool_result",
            "finalized_hash",
        ],
        forbidden_markers: &[],
    },
    BoundaryInventoryEntry {
        id: "desktop-session-resume-shell",
        path: "src/modes/interactive.rs",
        owner: "root interactive resume adapter",
        boundary: "desktop restore adapter",
        required_markers: &[
            "resume_session_from_file",
            "clankers_session::SessionManager::open",
            "seed_messages",
            "AgentMessage",
            "latest_compaction_summary",
        ],
        forbidden_markers: &[],
    },
    BoundaryInventoryEntry {
        id: "controller-session-persistence-adapter",
        path: "crates/clankers-controller/src/persistence.rs",
        owner: "clankers-controller::persistence",
        boundary: "desktop persistence adapter",
        required_markers: &[
            "use clankers_session::SessionManager",
            "fn persist_messages",
            "messages: &[clanker_message::transcript::AgentMessage]",
            "index_messages_for_search",
            "persist_compaction_summary_tool_result",
        ],
        forbidden_markers: &[],
    },
    BoundaryInventoryEntry {
        id: "controller-history-replay-projection",
        path: "crates/clankers-controller/src/convert.rs",
        owner: "clankers-controller::convert",
        boundary: "display replay projection",
        required_markers: &[
            "pub fn agent_message_to_tui_events",
            "AgentMessage::User",
            "AgentMessage::ToolResult",
            "AgentMessage::CompactionSummary",
            "BranchSummary and Custom messages don't map",
            "desktop_history_replay_parity_contract_covers_tool_compaction_branch_and_semantics",
        ],
        forbidden_markers: &[],
    },
    BoundaryInventoryEntry {
        id: "attach-history-replay-app-edge",
        path: "src/modes/attach/events.rs",
        owner: "attach history replay app edge",
        boundary: "daemon attach display replay",
        required_markers: &[
            "DaemonEvent::HistoryBlock",
            "serde_json::from_value::<clanker_message::transcript::AgentMessage>",
            "agent_message_to_tui_events(&msg)",
            "DaemonEvent::HistoryEnd",
            "finalize_active_block",
        ],
        forbidden_markers: &[],
    },
];

fn main() -> ExitCode {
    match run() {
        Ok(()) => {
            println!("ok: session ledger boundary inventory covers {} paths", INVENTORY.len());
            ExitCode::SUCCESS
        }
        Err(errors) => {
            for error in errors {
                eprintln!("session ledger boundary error: {error}");
            }
            ExitCode::from(ERROR_EXIT)
        }
    }
}

fn run() -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    validate_unique_ids(&mut errors);
    for entry in INVENTORY {
        validate_entry(entry, &mut errors);
    }
    validate_docs(&mut errors);
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

fn validate_unique_ids(errors: &mut Vec<String>) {
    let mut ids = std::collections::BTreeSet::new();
    for entry in INVENTORY {
        if !ids.insert(entry.id) {
            errors.push(format!("duplicate inventory id `{}`", entry.id));
        }
    }
}

fn validate_entry(entry: &BoundaryInventoryEntry, errors: &mut Vec<String>) {
    let path = Path::new(entry.path);
    if !path.exists() {
        errors.push(format!("{} path does not exist: {}", entry.id, entry.path));
        return;
    }
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            errors.push(format!("failed to read {}: {error}", entry.path));
            return;
        }
    };
    for marker in entry.required_markers {
        if !source.contains(marker) {
            errors.push(format!(
                "{} ({}, {}) missing marker {:?} in {}",
                entry.id, entry.owner, entry.boundary, marker, entry.path
            ));
        }
    }
    for marker in entry.forbidden_markers {
        if source.contains(marker) {
            errors.push(format!(
                "{} ({}, {}) contains forbidden marker {:?} in {}",
                entry.id, entry.owner, entry.boundary, marker, entry.path
            ));
        }
    }
}

fn validate_docs(errors: &mut Vec<String>) {
    let docs_path = "docs/src/tutorials/embedded-agent-sdk.md";
    let docs = fs::read_to_string(docs_path).unwrap_or_default();
    for marker in [
        "session-resume-brick evidence is fixture backed",
        "product-owned session stores and receipt DTOs",
        "Runtime::resume_session",
        "SessionLedgerEntry",
        "scripts/check-session-ledger-boundary.rs",
    ] {
        if !docs.contains(marker) {
            errors.push(format!("{docs_path} missing session ledger docs marker {marker:?}"));
        }
    }
}

