//! Menu building logic and builtin contributors.

use std::collections::HashMap;

use super::types::*;
use crate::registry::Conflict;

/// Build a leader menu from contributors.
///
/// Collects all [`MenuContribution`] items, deduplicates by `(key, placement)`
/// with highest priority winning, removes hidden entries, and assembles the
/// menu tree.
pub fn build(
    contributors: &[&dyn MenuContributor],
    hidden: &HiddenSet,
) -> BuildResult {
    let mut conflicts = Vec::new();

    // 1. Collect all contributions
    let mut all_items: Vec<MenuContribution> = contributors
        .iter()
        .flat_map(|c| c.menu_items())
        .collect();

    // 2. Sort by priority (lowest first, so highest overwrites)
    all_items.sort_by_key(|i| i.priority);

    // 3. Deduplicate by (key, placement) — last writer wins
    let mut seen: HashMap<(char, MenuPlacement), MenuContribution> = HashMap::new();
    for item in all_items {
        let key = (item.key, item.placement.clone());
        if let Some(existing) = seen.get(&key) {
            conflicts.push(Conflict {
                registry: "leader_menu",
                key: format!("'{}' in {:?}", item.key, item.placement),
                winner: item.source.clone(),
                loser: existing.source.clone(),
            });
        }
        seen.insert(key, item);
    }

    // 4. Remove hidden entries
    for h in hidden {
        seen.remove(h);
    }

    // 5. Group by placement
    let mut root_items: Vec<MenuContribution> = Vec::new();
    let mut submenu_items: HashMap<String, Vec<MenuContribution>> = HashMap::new();

    for ((_, placement), item) in seen {
        match placement {
            MenuPlacement::Root => root_items.push(item),
            MenuPlacement::Submenu(ref name) => {
                submenu_items.entry(name.clone()).or_default().push(item);
            }
        }
    }

    // 6. Build submenu defs
    let mut submenus: Vec<LeaderMenuDef> = Vec::new();
    for (name, mut items) in submenu_items {
        // Sort items by key for consistent ordering
        items.sort_by_key(|i| i.key);
        submenus.push(LeaderMenuDef {
            label: name,
            items: items
                .into_iter()
                .map(|c| LeaderMenuItem {
                    key: c.key,
                    label: c.label,
                    action: c.action,
                })
                .collect(),
        });
    }

    // 7. Build root def — sort items by key for consistent ordering
    root_items.sort_by_key(|i| i.key);
    let root = LeaderMenuDef {
        label: "Leader".into(),
        items: root_items
            .into_iter()
            .map(|c| LeaderMenuItem {
                key: c.key,
                label: c.label,
                action: c.action,
            })
            .collect(),
    };

    let menu = super::LeaderMenu {
        visible: false,
        stack: Vec::new(),
        breadcrumb: Vec::new(),
        submenus,
        root,
    };

    (menu, conflicts)
}

// ---------------------------------------------------------------------------
// Builtin contributors
// ---------------------------------------------------------------------------

/// Contributes the hardcoded keymap actions and submenu openers that aren't
/// slash commands (model selector, thinking toggle, etc.)
pub struct BuiltinKeymapContributor;

impl MenuContributor for BuiltinKeymapContributor {
    fn menu_items(&self) -> Vec<MenuContribution> {
        let mut items = Vec::new();
        items.extend(root_submenu_openers());
        items.extend(root_keymap_actions());
        items.extend(session_submenu_items());
        items.extend(layout_submenu_items());
        items.extend(pane_submenu_opener());
        items.extend(pane_submenu_items());
        items
    }
}

// ── Builtin menu data (grouped by section) ─────────────────────────────

/// Helper to build a builtin MenuContribution with less boilerplate.
fn builtin(key: char, label: &str, action: LeaderAction, placement: MenuPlacement) -> MenuContribution {
    MenuContribution {
        key,
        label: label.into(),
        action,
        placement,
        priority: crate::registry::PRIORITY_BUILTIN,
        source: "builtin".into(),
    }
}

fn root_submenu_openers() -> Vec<MenuContribution> {
    vec![
        builtin('s', "session", LeaderAction::Submenu("session".into()), MenuPlacement::Root),
        builtin('l', "layout", LeaderAction::Submenu("layout".into()), MenuPlacement::Root),
    ]
}

fn root_keymap_actions() -> Vec<MenuContribution> {
    use crate::config::keybindings::{Action, CoreAction, ExtendedAction};
    vec![
        builtin('m', "model", LeaderAction::KeymapAction(Action::Extended(ExtendedAction::OpenModelSelector)), MenuPlacement::Root),
        builtin('a', "account", LeaderAction::KeymapAction(Action::Extended(ExtendedAction::OpenAccountSelector)), MenuPlacement::Root),
        builtin('t', "toggle thinking", LeaderAction::KeymapAction(Action::Extended(ExtendedAction::ToggleThinking)), MenuPlacement::Root),
        builtin('T', "show/hide thinking", LeaderAction::KeymapAction(Action::Extended(ExtendedAction::ToggleShowThinking)), MenuPlacement::Root),
        builtin('f', "search output", LeaderAction::KeymapAction(Action::Extended(ExtendedAction::SearchOutput)), MenuPlacement::Root),
        builtin('`', "toggle panel", LeaderAction::KeymapAction(Action::Extended(ExtendedAction::TogglePanelFocus)), MenuPlacement::Root),
        builtin('o', "external editor", LeaderAction::KeymapAction(Action::Extended(ExtendedAction::OpenEditor)), MenuPlacement::Root),
        builtin('c', "cancel/abort", LeaderAction::KeymapAction(Action::Core(CoreAction::Cancel)), MenuPlacement::Root),
        builtin('x', "clear input", LeaderAction::KeymapAction(Action::Core(CoreAction::ClearLine)), MenuPlacement::Root),
    ]
}

fn session_submenu_items() -> Vec<MenuContribution> {
    let p = |s: &str| MenuPlacement::Submenu(s.into());
    vec![
        builtin('n', "new", LeaderAction::SlashCommand("/new".into()), p("session")),
        builtin('r', "resume", LeaderAction::SlashCommand("/resume".into()), p("session")),
        builtin('l', "list sessions", LeaderAction::SlashCommand("/sessions".into()), p("session")),
        builtin('c', "compact", LeaderAction::SlashCommand("/compact".into()), p("session")),
    ]
}

fn layout_submenu_items() -> Vec<MenuContribution> {
    let p = || MenuPlacement::Submenu("layout".into());
    let cmd = |s: &str| LeaderAction::SlashCommand(s.into());
    vec![
        builtin('d', "default (3-column)", cmd("/layout default"), p()),
        builtin('w', "wide chat", cmd("/layout wide"), p()),
        builtin('f', "focused (no panels)", cmd("/layout focused"), p()),
        builtin('r', "right-heavy", cmd("/layout right"), p()),
        builtin('1', "toggle Todo", cmd("/layout toggle todo"), p()),
        builtin('2', "toggle Files", cmd("/layout toggle files"), p()),
        builtin('3', "toggle Subagents", cmd("/layout toggle subagents"), p()),
        builtin('4', "toggle Peers", cmd("/layout toggle peers"), p()),
        builtin('5', "toggle Processes", cmd("/layout toggle processes"), p()),
        builtin('6', "toggle Branches", cmd("/layout toggle branches"), p()),
    ]
}

fn pane_submenu_opener() -> Vec<MenuContribution> {
    vec![builtin('p', "pane", LeaderAction::Submenu("pane".into()), MenuPlacement::Root)]
}

fn pane_submenu_items() -> Vec<MenuContribution> {
    use crate::config::keybindings::{Action, ExtendedAction};
    let p = || MenuPlacement::Submenu("pane".into());
    let ext = |e: ExtendedAction| LeaderAction::KeymapAction(Action::Extended(e));
    vec![
        builtin('z', "zoom toggle", ext(ExtendedAction::PaneZoom), p()),
        builtin('v', "split vertical", ext(ExtendedAction::PaneSplitVertical), p()),
        builtin('h', "split horizontal", ext(ExtendedAction::PaneSplitHorizontal), p()),
        builtin('x', "close pane", ext(ExtendedAction::PaneClose), p()),
        builtin('=', "equalize size", ext(ExtendedAction::PaneEqualize), p()),
        builtin('+', "grow pane", ext(ExtendedAction::PaneGrow), p()),
        builtin('-', "shrink pane", ext(ExtendedAction::PaneShrink), p()),
        builtin('H', "move left", ext(ExtendedAction::PaneMoveLeft), p()),
        builtin('L', "move right", ext(ExtendedAction::PaneMoveRight), p()),
        builtin('J', "move down", ext(ExtendedAction::PaneMoveDown), p()),
        builtin('K', "move up", ext(ExtendedAction::PaneMoveUp), p()),
    ]
}

/// Convert slash commands with `leader_key` bindings into menu contributions.
pub fn slash_command_contributions(commands: &[crate::slash_commands::SlashCommand]) -> Vec<MenuContribution> {
    use crate::registry::PRIORITY_BUILTIN;

    commands
        .iter()
        .filter_map(|cmd| {
            let binding = cmd.leader_key.as_ref()?;
            Some(MenuContribution {
                key: binding.key,
                label: binding
                    .label
                    .unwrap_or(cmd.description)
                    .to_string(),
                action: LeaderAction::SlashCommand(format!("/{}", cmd.name)),
                placement: binding.placement.clone(),
                priority: PRIORITY_BUILTIN,
                source: "builtin".into(),
            })
        })
        .collect()
}

/// Wrapper to make slash commands act as a MenuContributor.
pub struct SlashCommandContributor {
    commands: Vec<crate::slash_commands::SlashCommand>,
}

impl SlashCommandContributor {
    pub fn new(commands: Vec<crate::slash_commands::SlashCommand>) -> Self {
        Self { commands }
    }
}

impl MenuContributor for SlashCommandContributor {
    fn menu_items(&self) -> Vec<MenuContribution> {
        slash_command_contributions(&self.commands)
    }
}
