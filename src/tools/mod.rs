//! Built-in tools
//!
//! The `Tool` trait, `ToolContext`, and related types are defined in
//! `clankers-agent` and re-exported here for backward compatibility.

// Core tool types — canonical definitions in clankers-agent
pub use clankers_agent::tool::ModelSwitchSlot;
pub use clankers_agent::tool::Tool;
pub use clankers_agent::tool::ToolContext;
pub use clankers_agent::tool::ToolDefinition;
pub use clankers_agent::tool::ToolResult;
pub use clankers_agent::tool::ToolResultContent;
pub use clankers_agent::tool::model_switch_slot;

/// Output truncation utilities — re-exported from `crate::util::truncation`.
pub use crate::util::truncation;

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
