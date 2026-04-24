//! Menu building logic and builtin contributors.
//!
//! The generic build logic lives in `rat_leaderkey`. This module provides the
//! clankers-specific `BuiltinKeymapContributor` and a thin `build()` wrapper
//! that bridges the local `MenuContributor` trait to `rat_leaderkey`.

use super::types::*;

/// Build a leader menu from clankers contributors.
///
/// Collects items from the local [`MenuContributor`] trait and delegates to
/// [`rat_leaderkey::build_from_items`] for conflict resolution and assembly.
pub fn build(contributors: &[&dyn MenuContributor], hidden: &HiddenSet) -> BuildResult {
    let items: Vec<MenuContribution> = contributors.iter().flat_map(|c| c.menu_items()).collect();
    let (inner, conflicts) = rat_leaderkey::build_from_items(items, hidden);
    (super::LeaderMenu(inner), conflicts)
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
        items.extend(root_actions());
        items.extend(session_submenu_items());
        items.extend(branch_submenu_items());
        items.extend(layout_submenu_items());
        items.extend(pane_submenu_items());
        items.extend(loop_submenu_items());
        items.extend(swarm_submenu_items());
        items.extend(info_submenu_items());
        items.extend(debug_submenu_items());
        items.extend(model_submenu_items());
        items.extend(memory_submenu_items());
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
        priority: clanker_tui_types::PRIORITY_BUILTIN,
        source: "builtin".into(),
    }
}

fn root_submenu_openers() -> Vec<MenuContribution> {
    let r = MenuPlacement::Root;
    let sub = |name: &str| LeaderAction::Submenu(name.into());
    vec![
        builtin('s', "session", sub("session"), r.clone()),
        builtin('b', "branch", sub("branch"), r.clone()),
        builtin('l', "layout", sub("layout"), r.clone()),
        builtin('p', "pane", sub("pane"), r.clone()),
        builtin('L', "loop", sub("loop"), r.clone()),
        builtin('w', "swarm", sub("swarm"), r.clone()),
        builtin('i', "info", sub("info"), r.clone()),
        builtin('d', "debug", sub("debug"), r.clone()),
        builtin('M', "model/thinking", sub("model"), r.clone()),
        builtin('S', "system/memory", sub("memory"), r),
    ]
}

fn root_actions() -> Vec<MenuContribution> {
    use clanker_tui_types::Action;
    use clanker_tui_types::CoreAction;
    use clanker_tui_types::ExtendedAction;
    let r = MenuPlacement::Root;
    let ext = |e: ExtendedAction| LeaderAction::Action(Action::Extended(e));
    let cmd = |s: &str| LeaderAction::Command(s.into());
    vec![
        // Selectors / overlays
        builtin('m', "model", ext(ExtendedAction::OpenModelSelector), r.clone()),
        builtin('a', "account", ext(ExtendedAction::OpenAccountSelector), r.clone()),
        // Toggles
        builtin('t', "toggle thinking", ext(ExtendedAction::ToggleThinking), r.clone()),
        builtin('T', "show/hide thinking", ext(ExtendedAction::ToggleShowThinking), r.clone()),
        builtin('A', "auto-test", ext(ExtendedAction::ToggleAutoTest), r.clone()),
        builtin('I', "prompt improve", ext(ExtendedAction::TogglePromptImprove), r.clone()),
        builtin('P', "plan mode", cmd("/plan"), r.clone()),
        // Actions
        builtin('f', "search output", ext(ExtendedAction::SearchOutput), r.clone()),
        builtin('`', "toggle panel", ext(ExtendedAction::TogglePanelFocus), r.clone()),
        builtin('o', "external editor", ext(ExtendedAction::OpenEditor), r.clone()),
        builtin('c', "cancel/abort", LeaderAction::Action(Action::Core(CoreAction::Cancel)), r.clone()),
        builtin('x', "clear input", LeaderAction::Action(Action::Core(CoreAction::ClearLine)), r.clone()),
        builtin('u', "undo last turn", cmd("/undo"), r.clone()),
        builtin('e', "export", cmd("/export"), r.clone()),
        builtin('R', "code review", cmd("/review"), r.clone()),
        builtin('C', "compact", cmd("/compact"), r.clone()),
        builtin('?', "help", cmd("/help"), r.clone()),
        builtin('q', "quit", LeaderAction::Action(Action::Core(CoreAction::Quit)), r),
    ]
}

// ── Session submenu ─────────────────────────────────────────────────────

fn session_submenu_items() -> Vec<MenuContribution> {
    let p = || MenuPlacement::Submenu("session".into());
    let cmd = |s: &str| LeaderAction::Command(s.into());
    vec![
        builtin('n', "new session", cmd("/new"), p()),
        builtin('r', "resume session", cmd("/resume"), p()),
        builtin('l', "list sessions", cmd("/sessions"), p()),
        builtin('c', "compact", cmd("/compact"), p()),
        builtin('x', "clear history", cmd("/clear"), p()),
        builtin('R', "reset (full wipe)", cmd("/reset"), p()),
    ]
}

// ── Branch submenu ──────────────────────────────────────────────────────

fn branch_submenu_items() -> Vec<MenuContribution> {
    let p = || MenuPlacement::Submenu("branch".into());
    let cmd = |s: &str| LeaderAction::Command(s.into());
    vec![
        builtin('f', "fork", cmd("/fork"), p()),
        builtin('b', "list branches", cmd("/branches"), p()),
        builtin('s', "switch branch", cmd("/switch"), p()),
        builtin('r', "rewind", cmd("/rewind"), p()),
        builtin('c', "compare", cmd("/compare"), p()),
        builtin('l', "label", cmd("/label"), p()),
        builtin('m', "merge", cmd("/merge"), p()),
        builtin('i', "merge interactive", cmd("/merge-interactive"), p()),
        builtin('p', "cherry-pick", cmd("/cherry-pick"), p()),
    ]
}

// ── Layout submenu ──────────────────────────────────────────────────────

fn layout_submenu_items() -> Vec<MenuContribution> {
    let p = || MenuPlacement::Submenu("layout".into());
    let cmd = |s: &str| LeaderAction::Command(s.into());
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

// ── Pane submenu ────────────────────────────────────────────────────────

fn pane_submenu_items() -> Vec<MenuContribution> {
    use clanker_tui_types::Action;
    use clanker_tui_types::ExtendedAction;
    let p = || MenuPlacement::Submenu("pane".into());
    let ext = |e: ExtendedAction| LeaderAction::Action(Action::Extended(e));
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

// ── Loop submenu ────────────────────────────────────────────────────────

fn loop_submenu_items() -> Vec<MenuContribution> {
    let p = || MenuPlacement::Submenu("loop".into());
    let cmd = |s: &str| LeaderAction::Command(s.into());
    vec![
        builtin('p', "pause/resume", cmd("/loop pause"), p()),
        builtin('s', "stop", cmd("/loop stop"), p()),
        builtin('i', "status", cmd("/loop status"), p()),
    ]
}

// ── Swarm submenu ───────────────────────────────────────────────────────

fn swarm_submenu_items() -> Vec<MenuContribution> {
    let p = || MenuPlacement::Submenu("swarm".into());
    let cmd = |s: &str| LeaderAction::Command(s.into());
    vec![
        builtin('w', "spawn/list workers", cmd("/worker"), p()),
        builtin('s', "subagents", cmd("/subagents"), p()),
        builtin('p', "peers", cmd("/peers"), p()),
        builtin('h', "share session", cmd("/share"), p()),
    ]
}

// ── Info submenu ────────────────────────────────────────────────────────

fn info_submenu_items() -> Vec<MenuContribution> {
    let p = || MenuPlacement::Submenu("info".into());
    let cmd = |s: &str| LeaderAction::Command(s.into());
    vec![
        builtin('s', "status", cmd("/status"), p()),
        builtin('u', "usage", cmd("/usage"), p()),
        builtin('v', "version", cmd("/version"), p()),
        builtin('h', "hooks", cmd("/hooks"), p()),
        builtin('t', "tools", cmd("/tools"), p()),
        builtin('l', "login", cmd("/login"), p()),
        builtin('r', "role", cmd("/role"), p()),
        builtin('R', "router", cmd("/router"), p()),
    ]
}

// ── Debug submenu ───────────────────────────────────────────────────────

fn debug_submenu_items() -> Vec<MenuContribution> {
    let p = || MenuPlacement::Submenu("debug".into());
    let cmd = |s: &str| LeaderAction::Command(s.into());
    vec![
        builtin('l', "leader menu dump", cmd("/leader"), p()),
        builtin('p', "preview markdown", cmd("/preview"), p()),
        builtin('P', "plugins", cmd("/plugin"), p()),
    ]
}

// ── Model/Thinking submenu ──────────────────────────────────────────────

fn model_submenu_items() -> Vec<MenuContribution> {
    use clanker_tui_types::Action;
    use clanker_tui_types::ExtendedAction;
    let p = || MenuPlacement::Submenu("model".into());
    let ext = |e: ExtendedAction| LeaderAction::Action(Action::Extended(e));
    let cmd = |s: &str| LeaderAction::Command(s.into());
    vec![
        builtin('m', "select model", ext(ExtendedAction::OpenModelSelector), p()),
        builtin('t', "cycle thinking", ext(ExtendedAction::ToggleThinking), p()),
        builtin('T', "show/hide thinking", ext(ExtendedAction::ToggleShowThinking), p()),
        builtin('r', "role", cmd("/role"), p()),
        builtin('0', "thinking off", cmd("/think off"), p()),
        builtin('1', "thinking low", cmd("/think low"), p()),
        builtin('2', "thinking medium", cmd("/think medium"), p()),
        builtin('3', "thinking high", cmd("/think high"), p()),
        builtin('4', "thinking max", cmd("/think max"), p()),
    ]
}

// ── System/Memory submenu ───────────────────────────────────────────────

fn memory_submenu_items() -> Vec<MenuContribution> {
    let p = || MenuPlacement::Submenu("memory".into());
    let cmd = |s: &str| LeaderAction::Command(s.into());
    vec![
        builtin('s', "show system prompt", cmd("/system show"), p()),
        builtin('r', "reset system prompt", cmd("/system reset"), p()),
        builtin('m', "list memories", cmd("/memory"), p()),
    ]
}
