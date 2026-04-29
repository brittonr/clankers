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

pub mod progress {
    //! Progress and result streaming types — re-exported from `clankers-agent`.
    pub use clankers_agent::tool::progress::*;
}

pub mod ask;
pub mod autoresearch;
pub mod bash;
pub mod commit;
pub mod compress;
pub mod cost;
pub mod delegate;
pub mod devtools;
pub mod diff;
pub mod edit;
pub mod execute_code;
pub mod find;
pub mod git_ops;
pub mod grep;
pub mod image_gen;
pub mod loop_tool;
pub mod ls;
pub mod matrix;
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
pub mod subagent;
pub mod switch_model;
pub mod todo;
pub mod tts;
pub mod validator_tool;
pub mod watchdog;
pub mod web;
pub mod write;
