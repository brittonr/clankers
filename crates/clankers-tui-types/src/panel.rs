//! Panel identity and action types.

use serde::Deserialize;
use serde::Serialize;

/// Unique identifier for a panel. Used by the layout engine and focus tracker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PanelId {
    Todo,
    Files,
    Subagents,
    Peers,
    Processes,
    Branches,
}

impl PanelId {
    pub fn label(self) -> &'static str {
        match self {
            PanelId::Todo => "Todo",
            PanelId::Files => "Files",
            PanelId::Subagents => "Subagents",
            PanelId::Peers => "Peers",
            PanelId::Processes => "Processes",
            PanelId::Branches => "Branches",
        }
    }

    /// All known panel IDs (for iteration / config validation).
    pub const ALL: &'static [PanelId] = &[
        PanelId::Todo,
        PanelId::Files,
        PanelId::Subagents,
        PanelId::Peers,
        PanelId::Processes,
        PanelId::Branches,
    ];
}

impl std::fmt::Display for PanelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Actions that a panel can emit back to the application.
/// Follows the ratatui Component pattern: event handlers return
/// `Option<PanelAction>` rather than mutating app state directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanelAction {
    /// The key was consumed internally — no further handling needed.
    Consumed,
    /// The panel wants to give up focus (e.g. Esc pressed).
    Unfocus,
    /// The panel wants to run a slash command.
    SlashCommand(String),
    /// The panel wants to switch focus to a different panel.
    FocusPanel(PanelId),
    /// Switch to a conversation branch by block ID.
    SwitchBranch(usize),
    /// Focus a subagent's dedicated BSP pane (by subagent ID).
    FocusSubagent(String),
}

/// Which UI region a mouse event landed in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HitRegion {
    /// The main messages / chat area.
    Messages,
    /// The text editor / input area.
    Editor,
    /// A side panel.
    Panel(PanelId),
    /// A subagent's dedicated pane.
    Subagent(String),
    /// The status bar.
    StatusBar,
    /// A scrollbar in a panel.
    PanelScrollbar(PanelId),
    /// A scrollbar in the messages area.
    MessagesScrollbar,
    /// A scrollbar in a subagent pane.
    SubagentScrollbar(String),
    /// Outside any tracked region.
    None,
}
