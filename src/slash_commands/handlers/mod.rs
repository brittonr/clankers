//! Slash command handler infrastructure.
//!
//! Defines [`SlashContext`] — the context passed to the dispatch function
//! in `slash_commands::dispatch()`.

use std::sync::Arc;
use std::sync::Mutex;

use crate::modes::interactive::AgentCommand;
use crate::plugin::PluginManager;
use crate::tui::app::App;
use crate::tui::components::subagent_event::SubagentEvent;

/// Context passed to the slash command dispatch function.
#[allow(private_interfaces)]
pub struct SlashContext<'a> {
    pub app: &'a mut App,
    pub cmd_tx: &'a tokio::sync::mpsc::UnboundedSender<AgentCommand>,
    pub plugin_manager: Option<&'a Arc<Mutex<PluginManager>>>,
    pub panel_tx: &'a tokio::sync::mpsc::UnboundedSender<SubagentEvent>,
    pub db: &'a Option<crate::db::Db>,
    pub session_manager: &'a mut Option<crate::session::SessionManager>,
}
