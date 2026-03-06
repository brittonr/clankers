//! Slash command handler infrastructure.
//!
//! Defines [`SlashContext`] — the context passed to the dispatch function
//! in `slash_commands::dispatch()`.
//!
//! Handler implementations are organized by domain:
//! - `info` — Help, Status, Usage, Version, Quit
//! - `context` — Clear, Reset, Compact, Undo
//! - `model` — Model, Think, Role
//! - `navigation` — Cd, Shell
//! - `export` — Export
//! - `auth` — Login, Account
//! - `tools` — Tools, Plugin
//! - `swarm` — Worker, Share, Subagents, Peers
//! - `tui` — Layout, Preview, Editor, Todo, Plan, Review
//! - `memory` — SystemPrompt, Memory
//! - `branching` — Fork, Rewind, Branches, Switch, Label
//! - `session` — Session
//! - `prompt_template` — PromptTemplate

pub mod auth;
pub mod branching;
pub mod context;
pub mod export;
pub mod info;
pub mod memory;
pub mod model;
pub mod navigation;
pub mod prompt_template;
pub mod session;
pub mod swarm;
pub mod tools;
pub mod tui;

use std::sync::Arc;
use std::sync::Mutex;

use crate::modes::interactive::AgentCommand;
use crate::plugin::PluginManager;
use crate::tui::app::App;
use crate::tui::components::subagent_event::SubagentEvent;

/// Context passed to every slash command handler.
#[allow(private_interfaces)]
pub struct SlashContext<'a> {
    pub app: &'a mut App,
    pub cmd_tx: &'a tokio::sync::mpsc::UnboundedSender<AgentCommand>,
    pub plugin_manager: Option<&'a Arc<Mutex<PluginManager>>>,
    pub panel_tx: &'a tokio::sync::mpsc::UnboundedSender<SubagentEvent>,
    pub db: &'a Option<crate::db::Db>,
    pub session_manager: &'a mut Option<crate::session::SessionManager>,
}

/// A slash command handler.
pub trait SlashHandler: Send + Sync {
    /// Execute the command with the given arguments.
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>);
}
