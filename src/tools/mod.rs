//! Built-in tools
//!
//! The `Tool` trait, `ToolContext`, and related types are defined in
//! `clankers-agent` and re-exported here for backward compatibility.

// Core tool types — canonical definitions in clankers-agent
pub use clankers_agent::tool::ModelSwitchSlot;
pub use clankers_agent::tool::Tool;
pub use clankers_agent::tool::ToolContext;
pub use clankers_agent::tool::ToolDefinition;
pub use clankers_agent::tool::ToolExecutionBackend;
pub use clankers_agent::tool::ToolResult;
pub use clankers_agent::tool::ToolResultContent;
pub use clankers_agent::tool::model_switch_slot;
/// Output truncation utilities — re-exported from `clankers_util::truncation`.
pub use clankers_util::truncation;

fn protect_file_mutation(tool_name: &str, path_str: &str) -> Result<serde_json::Value, String> {
    let path = std::path::Path::new(path_str);
    let cwd = mutation_checkpoint_cwd(path);
    let request = crate::checkpoints::AutoCheckpointRequest::new(tool_name, path_str);
    let policy = if is_git_checkout(&cwd) {
        crate::checkpoints::AutoCheckpointPolicy::default()
    } else {
        crate::checkpoints::AutoCheckpointPolicy::disabled()
    };
    crate::checkpoints::ensure_pre_mutation_checkpoint(&cwd, &policy, request)
        .map(|receipt| serde_json::json!({ "auto_checkpoint": receipt }))
        .map_err(|error| error.to_string())
}

fn mutation_checkpoint_cwd(path: &std::path::Path) -> std::path::PathBuf {
    if path.is_absolute()
        && let Some(parent) = path.parent()
        && parent.exists()
    {
        return parent.to_path_buf();
    }
    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
}

fn is_git_checkout(cwd: &std::path::Path) -> bool {
    std::process::Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

pub mod progress {
    //! Progress and result streaming types — re-exported from `clankers-agent`.
    pub use clankers_agent::tool::progress::*;
}

#[allow(dead_code)]
pub(crate) const SDK_TOOL_CONTEXT_BOUNDARY_INVENTORY_REQUIREMENT: &str = "r[sdk-tool-context-boundary.inventory]";

#[allow(dead_code)]
pub(crate) const SDK_TOOL_CONTEXT_BOUNDARY_COMPAT_REQUIREMENT: &str =
    "r[sdk-tool-context-boundary.legacy-context.compatibility-only]";

#[allow(dead_code)]
pub(crate) const SDK_TOOL_CONTEXT_BOUNDARY_NEUTRAL_REQUIREMENT: &str =
    "r[sdk-tool-context-boundary.neutral-services.representative-tools]";

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ToolContextServiceFamily {
    Storage,
    Search,
    Hooks,
    Progress,
    Cancellation,
    SessionIdentity,
    PluginRuntime,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToolContextMigrationOwner {
    NeutralToolHostService,
    CompatibilityToolContext,
    PluginRuntimeAdapter,
}

#[allow(dead_code)]
pub(crate) struct ToolContextMigrationEntry {
    pub(crate) tool: &'static str,
    pub(crate) family: ToolContextServiceFamily,
    pub(crate) owner: ToolContextMigrationOwner,
    pub(crate) replacement: &'static str,
    pub(crate) status: &'static str,
}

#[allow(dead_code)]
pub(crate) const TOOL_CONTEXT_MIGRATION_INVENTORY: &[ToolContextMigrationEntry] = &[
    ToolContextMigrationEntry {
        tool: "external_memory(local)",
        family: ToolContextServiceFamily::Search,
        owner: ToolContextMigrationOwner::NeutralToolHostService,
        replacement: "ToolSearchService via ToolInvocationContext",
        status: "migrated representative search path; legacy execute remains compatibility-only",
    },
    ToolContextMigrationEntry {
        tool: "grep",
        family: ToolContextServiceFamily::Progress,
        owner: ToolContextMigrationOwner::NeutralToolHostService,
        replacement: "ToolProgressSink via ToolInvocationContext",
        status: "migrated representative progress path; legacy execute remains direct-test compatibility",
    },
    ToolContextMigrationEntry {
        tool: "grep",
        family: ToolContextServiceFamily::Cancellation,
        owner: ToolContextMigrationOwner::NeutralToolHostService,
        replacement: "ToolInvocationCancellation",
        status: "migrated representative cancellation snapshot path",
    },
    ToolContextMigrationEntry {
        tool: "read",
        family: ToolContextServiceFamily::Storage,
        owner: ToolContextMigrationOwner::CompatibilityToolContext,
        replacement: "ToolStorageService or file-cache service DTO",
        status: "remaining: file cache uses Db and session identity through ToolContext",
    },
    ToolContextMigrationEntry {
        tool: "memory",
        family: ToolContextServiceFamily::Storage,
        owner: ToolContextMigrationOwner::CompatibilityToolContext,
        replacement: "typed memory storage/search service DTO",
        status: "remaining: memory CRUD needs richer neutral storage contract",
    },
    ToolContextMigrationEntry {
        tool: "session_search",
        family: ToolContextServiceFamily::Search,
        owner: ToolContextMigrationOwner::CompatibilityToolContext,
        replacement: "ToolSearchService plus session-ledger browse DTO",
        status: "remaining: DB metadata search and JSONL browse are still legacy compatibility",
    },
    ToolContextMigrationEntry {
        tool: "process",
        family: ToolContextServiceFamily::Storage,
        owner: ToolContextMigrationOwner::CompatibilityToolContext,
        replacement: "durable process-job storage service DTO",
        status: "remaining: durable process records use Db through ToolContext",
    },
    ToolContextMigrationEntry {
        tool: "commit",
        family: ToolContextServiceFamily::Hooks,
        owner: ToolContextMigrationOwner::CompatibilityToolContext,
        replacement: "ToolHookService pre/post commit decisions",
        status: "remaining: commit-specific hooks still read HookPipeline through ToolContext",
    },
    ToolContextMigrationEntry {
        tool: "plugin_tool(stdio/wasm)",
        family: ToolContextServiceFamily::PluginRuntime,
        owner: ToolContextMigrationOwner::PluginRuntimeAdapter,
        replacement: "PluginTool neutral runtime adapter plus ToolProgressSink/Cancellation",
        status: "remaining: plugin runtime still bridges progress/cancellation from ToolContext",
    },
    ToolContextMigrationEntry {
        tool: "schedule",
        family: ToolContextServiceFamily::SessionIdentity,
        owner: ToolContextMigrationOwner::CompatibilityToolContext,
        replacement: "ToolInvocationContext metadata session_id",
        status: "remaining: schedule mutations use session identity through ToolContext",
    },
];

#[cfg(test)]
mod tool_context_migration_tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn tool_context_migration_inventory_covers_service_families_and_representatives() {
        assert_eq!(SDK_TOOL_CONTEXT_BOUNDARY_INVENTORY_REQUIREMENT, "r[sdk-tool-context-boundary.inventory]");
        assert_eq!(
            SDK_TOOL_CONTEXT_BOUNDARY_COMPAT_REQUIREMENT,
            "r[sdk-tool-context-boundary.legacy-context.compatibility-only]"
        );
        assert_eq!(
            SDK_TOOL_CONTEXT_BOUNDARY_NEUTRAL_REQUIREMENT,
            "r[sdk-tool-context-boundary.neutral-services.representative-tools]"
        );
        let families = TOOL_CONTEXT_MIGRATION_INVENTORY.iter().map(|entry| entry.family).collect::<BTreeSet<_>>();
        for family in [
            ToolContextServiceFamily::Storage,
            ToolContextServiceFamily::Search,
            ToolContextServiceFamily::Hooks,
            ToolContextServiceFamily::Progress,
            ToolContextServiceFamily::Cancellation,
            ToolContextServiceFamily::SessionIdentity,
            ToolContextServiceFamily::PluginRuntime,
        ] {
            assert!(families.contains(&family), "missing inventory family {family:?}");
        }
        assert!(TOOL_CONTEXT_MIGRATION_INVENTORY.iter().any(|entry| {
            entry.tool == "external_memory(local)" && entry.owner == ToolContextMigrationOwner::NeutralToolHostService
        }));
        assert!(
            TOOL_CONTEXT_MIGRATION_INVENTORY.iter().any(|entry| {
                entry.tool == "grep" && entry.owner == ToolContextMigrationOwner::NeutralToolHostService
            })
        );
        assert!(
            TOOL_CONTEXT_MIGRATION_INVENTORY
                .iter()
                .all(|entry| { !entry.replacement.is_empty() && !entry.status.is_empty() })
        );
    }
}

pub mod ask;
pub mod autoresearch;
pub mod bash;
pub mod browser;
pub mod checkpoint;
pub mod commit;
pub mod compress;
pub mod cost;
pub mod delegate;
pub mod devtools;
pub mod diff;
pub mod edit;
pub mod execute_code;
pub mod external_memory;
pub mod find;
pub mod git_ops;
pub mod grep;
pub mod image_gen;
pub mod loop_tool;
pub mod ls;
#[cfg(feature = "matrix-bridge")]
pub mod matrix;
pub mod mcp;
pub mod memory;
pub mod nix;
pub mod patch;
pub mod plugin_tool;
pub mod process;
pub mod procmon;
pub mod read;
pub mod review;
pub mod sandbox;
pub mod schedule;
pub mod session_search;
pub mod signal_loop;
pub mod skill_manage;
pub mod skill_view;
pub mod soul_personality;
pub mod steel_eval;
pub mod subagent;
pub mod switch_model;
pub mod todo;
pub mod tool_gateway;
pub mod tts;
pub mod validator_tool;
pub mod voice_mode;
pub mod watchdog;
pub mod web;
pub mod write;
