//! Slash command handler infrastructure.
//!
//! Defines [`SlashContext`] — the context passed to slash command handlers
//! via [`SlashRegistry::dispatch()`].
//!
//! Handler implementations are organized by domain:
//! - `info` — Help, Status, Usage, Version, Quit, Leader, Export
//! - `context` — Clear, Reset, Compact, Undo, Cd, Shell
//! - `model` — Model, Think, Role
//! - `auth` — Login, Account
//! - `tools` — Tools, Plugin
//! - `swarm` — Worker, Share, Subagents, Peers
//! - `tui` — Layout, Preview, Editor, Todo, Plan, Review
//! - `memory` — SystemPrompt, Memory
//! - `branching` — Fork, Rewind, Branches, Switch, Label
//! - `session` — Session
//! - `prompt_template` — PromptTemplate

pub(crate) mod auth;
pub(crate) mod branching;
pub(crate) mod context;
pub(crate) mod info;
pub(crate) mod memory;
pub(crate) mod model;
pub(crate) mod prompt_template;
pub(crate) mod session;
pub(crate) mod swarm;
pub(crate) mod tools;
pub(crate) mod tui;

use std::sync::Arc;
use std::sync::Mutex;

use clankers_tui_types::SubagentEvent;

use crate::modes::interactive::AgentCommand;
use crate::plugin::PluginManager;
use crate::tui::app::App;

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
    /// Returns the command's metadata (name, description, help, etc.)
    fn command(&self) -> super::SlashCommand;

    /// Execute the command with the given arguments.
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>);
}
