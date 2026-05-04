//! CLI subcommand handlers.
//!
//! Each submodule handles one top-level command group. The shared
//! [`CommandContext`] bundles resolved paths, settings, and model
//! configuration so handlers don't need to re-resolve them.

pub mod acp;
pub mod auth;
pub mod batch;
pub mod checkpoint;
pub mod config;
pub mod daemon;
pub mod gateway;
pub mod mcp;
pub mod self_evolution;
pub mod soul;
pub mod voice;

pub mod plugin;
pub mod rpc;
pub mod session;
#[cfg(feature = "zellij-share")]
pub mod share;
pub mod token;

use crate::config::ClankersPaths;
use crate::config::ProjectPaths;
use crate::config::Settings;

/// Shared context passed to every subcommand handler.
///
/// Resolved once in `main()` and threaded through to avoid
/// re-resolving paths or reloading settings in each handler.
pub struct CommandContext {
    pub paths: ClankersPaths,
    pub project_paths: ProjectPaths,
    pub settings: Settings,
    pub model: String,
    pub system_prompt: String,
    pub cwd: String,
    /// CLI-level API key override
    pub api_key: Option<String>,
    /// CLI-level API base URL override
    pub api_base: Option<String>,
    /// CLI-level account override
    pub account: Option<String>,
}
