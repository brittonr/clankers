//! Leader menu types and traits.

use std::collections::HashSet;

use crate::actions::Action;

// ---------------------------------------------------------------------------
// Leader actions — things the leader menu can trigger
// ---------------------------------------------------------------------------

/// An action that a leader menu item can trigger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeaderAction {
    /// Trigger a normal-mode `Action` (reuses the existing action enum).
    KeymapAction(Action),
    /// Execute a slash command string (e.g. "/new", "/compact").
    SlashCommand(String),
    /// Open a named submenu.
    Submenu(String),
}

// ---------------------------------------------------------------------------
// Menu item
// ---------------------------------------------------------------------------

/// A single entry in the leader key menu.
#[derive(Debug, Clone)]
pub struct LeaderMenuItem {
    /// The key to press (single char, e.g. 's', 'm').
    pub key: char,
    /// Display label.
    pub label: String,
    /// What happens when this item is selected.
    pub action: LeaderAction,
}

// ---------------------------------------------------------------------------
// Menu definition (a flat level of items)
// ---------------------------------------------------------------------------

/// A named menu (root or submenu).
#[derive(Debug, Clone)]
pub struct LeaderMenuDef {
    pub label: String,
    pub items: Vec<LeaderMenuItem>,
}

// ---------------------------------------------------------------------------
// Dynamic registration types
// ---------------------------------------------------------------------------

/// Where a menu item should appear.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MenuPlacement {
    /// Top-level root menu.
    Root,
    /// Inside a named submenu (created if it doesn't exist).
    Submenu(String),
}

/// A single contribution to the leader menu from any source.
#[derive(Debug, Clone)]
pub struct MenuContribution {
    /// Key to press (single char).
    pub key: char,
    /// Display label.
    pub label: String,
    /// What happens when selected.
    pub action: LeaderAction,
    /// Where this item appears.
    pub placement: MenuPlacement,
    /// Priority for conflict resolution (higher wins).
    pub priority: u16,
    /// Source identifier for diagnostics ("builtin", plugin name, "config").
    pub source: String,
}

/// Anything that contributes items to the leader menu.
pub trait MenuContributor {
    fn menu_items(&self) -> Vec<MenuContribution>;
}

/// Type alias for hidden entries set.
pub type HiddenSet = HashSet<(char, MenuPlacement)>;
