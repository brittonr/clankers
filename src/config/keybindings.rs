//! Modal keymap configuration with preset support (helix, vim)
//!
//! Two modes: **Normal** (navigation, block operations with bare keys) and
//! **Insert** (typing into the editor, only modified keys trigger actions).
//!
//! The interactive event loop resolves key events through the active keymap
//! with the current `InputMode`, instead of hardcoding key checks.

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyModifiers;
use serde::Deserialize;
use serde::Serialize;

// ---------------------------------------------------------------------------
// Input mode
// ---------------------------------------------------------------------------

/// Modal editing mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum InputMode {
    /// Navigation mode — bare keys trigger actions, no text insertion.
    #[default]
    Normal,
    /// Typing mode — bare keys insert characters, modified keys trigger actions.
    Insert,
}

impl fmt::Display for InputMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Normal => write!(f, "NORMAL"),
            Self::Insert => write!(f, "INSERT"),
        }
    }
}

// ---------------------------------------------------------------------------
// Actions — every semantic operation the TUI supports
// ---------------------------------------------------------------------------

/// Core actions that cannot be extended by plugins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoreAction {
    // ── Mode switching ───────────────────────────────
    EnterInsert,
    EnterCommand,
    EnterNormal,

    // ── Core ──────────────────────────────────────────
    Submit,
    NewLine,
    Cancel,
    Quit,

    // ── Editor movement ──────────────────────────────
    MoveLeft,
    MoveRight,
    MoveHome,
    MoveEnd,

    // ── Editor editing ───────────────────────────────
    DeleteBack,
    DeleteForward,
    DeleteWord,
    ClearLine,

    // ── History ──────────────────────────────────────
    HistoryUp,
    HistoryDown,

    // ── Scrolling ────────────────────────────────────
    ScrollUp,
    ScrollDown,
    ScrollPageUp,
    ScrollPageDown,
    ScrollToTop,
    ScrollToBottom,

    // ── Block navigation ─────────────────────────────
    FocusPrevBlock,
    FocusNextBlock,
    Unfocus,

    // ── Menu navigation (slash command autocomplete) ─
    MenuUp,
    MenuDown,
    MenuAccept,
    MenuClose,

    // ── Clipboard paste ──────────────────────────────
    PasteImage,
}

/// Semantic actions that keybindings can trigger.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Action {
    /// Core action (hardcoded, cannot be extended)
    #[serde(skip)]
    Core(CoreAction),
    /// Extended action — compile-time checked, no stringly-typed dispatch
    Extended(ExtendedAction),
}

/// All extended actions as a proper enum. Eliminates stringly-typed dispatch
/// and gives compile-time exhaustiveness checking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExtendedAction {
    // Search
    SearchOutput,
    SearchNext,
    SearchPrev,
    // Block operations
    ToggleBlockCollapse,
    CollapseAllBlocks,
    ExpandAllBlocks,
    CopyBlock,
    RerunBlock,
    EditBlock,
    // Branch / panel navigation
    BranchPrev,
    BranchNext,
    // Toggles
    ToggleBlockIds,
    ToggleThinking,
    ToggleShowThinking,
    TogglePanelFocus,
    ToggleCostOverlay,
    ToggleSessionPopup,
    ToggleBranchPanel,
    // Panel operations
    PanelNextTab,
    PanelPrevTab,
    PanelScrollUp,
    PanelScrollDown,
    PanelClearDone,
    PanelKill,
    PanelRemove,
    // Selectors / menus
    OpenLeaderMenu,
    OpenModelSelector,
    OpenAccountSelector,
    OpenBranchSwitcher,
    OpenEditor,
    // Clipboard
    PasteImage,
    // Pane tiling
    PaneZoom,
    PaneSplitHorizontal,
    PaneSplitVertical,
    PaneClose,
    PaneEqualize,
    PaneGrow,
    PaneShrink,
    PaneMoveLeft,
    PaneMoveRight,
    PaneMoveUp,
    PaneMoveDown,
}

// Name mapping table for ExtendedAction
const EXTENDED_ACTION_NAMES: &[(ExtendedAction, &[&str])] = &[
    (ExtendedAction::SearchOutput, &["search_output", "search", "find"]),
    (ExtendedAction::SearchNext, &["search_next", "next_match"]),
    (ExtendedAction::SearchPrev, &["search_prev", "prev_match"]),
    (ExtendedAction::ToggleBlockCollapse, &["toggle_block_collapse", "toggle_collapse"]),
    (ExtendedAction::CollapseAllBlocks, &["collapse_all_blocks", "collapse_all"]),
    (ExtendedAction::ExpandAllBlocks, &["expand_all_blocks", "expand_all"]),
    (ExtendedAction::CopyBlock, &["copy_block"]),
    (ExtendedAction::RerunBlock, &["rerun_block"]),
    (ExtendedAction::EditBlock, &["edit_block"]),
    (ExtendedAction::BranchPrev, &["branch_prev"]),
    (ExtendedAction::BranchNext, &["branch_next"]),
    (ExtendedAction::ToggleBlockIds, &["toggle_block_ids", "toggle_ids"]),
    (ExtendedAction::ToggleThinking, &["toggle_thinking"]),
    (ExtendedAction::ToggleShowThinking, &["toggle_show_thinking"]),
    (ExtendedAction::TogglePanelFocus, &["toggle_panel_focus", "panel_focus"]),
    (ExtendedAction::ToggleCostOverlay, &["toggle_cost_overlay", "cost_overlay"]),
    (ExtendedAction::ToggleSessionPopup, &["toggle_session_popup", "session_popup"]),
    (ExtendedAction::ToggleBranchPanel, &["toggle_branch_panel", "branch_panel"]),
    (ExtendedAction::PanelNextTab, &["panel_next_tab", "panel_next"]),
    (ExtendedAction::PanelPrevTab, &["panel_prev_tab", "panel_prev"]),
    (ExtendedAction::PanelScrollUp, &["panel_scroll_up"]),
    (ExtendedAction::PanelScrollDown, &["panel_scroll_down"]),
    (ExtendedAction::PanelClearDone, &["panel_clear_done", "panel_clear"]),
    (ExtendedAction::PanelKill, &["panel_kill"]),
    (ExtendedAction::PanelRemove, &["panel_remove"]),
    (ExtendedAction::OpenLeaderMenu, &["open_leader_menu", "leader_menu", "leader"]),
    (ExtendedAction::OpenModelSelector, &["open_model_selector", "model_selector"]),
    (ExtendedAction::OpenAccountSelector, &["open_account_selector", "account_selector"]),
    (ExtendedAction::OpenBranchSwitcher, &["open_branch_switcher", "branch_switcher"]),
    (ExtendedAction::OpenEditor, &["open_editor", "editor"]),
    (ExtendedAction::PasteImage, &["paste_image"]),
    (ExtendedAction::PaneZoom, &["pane_zoom", "zoom", "zoom_toggle"]),
    (ExtendedAction::PaneSplitHorizontal, &["pane_split_horizontal"]),
    (ExtendedAction::PaneSplitVertical, &["pane_split_vertical"]),
    (ExtendedAction::PaneClose, &["pane_close"]),
    (ExtendedAction::PaneEqualize, &["pane_equalize"]),
    (ExtendedAction::PaneGrow, &["pane_grow"]),
    (ExtendedAction::PaneShrink, &["pane_shrink"]),
    (ExtendedAction::PaneMoveLeft, &["pane_move_left"]),
    (ExtendedAction::PaneMoveRight, &["pane_move_right"]),
    (ExtendedAction::PaneMoveUp, &["pane_move_up"]),
    (ExtendedAction::PaneMoveDown, &["pane_move_down"]),
];

impl ExtendedAction {
    /// Parse from a string name (for keymap config and leader menu).
    pub fn from_name(s: &str) -> Option<Self> {
        EXTENDED_ACTION_NAMES
            .iter()
            .find(|(_, names)| names.contains(&s))
            .map(|(action, _)| *action)
    }

    /// Canonical string name (for serialization and display).
    pub fn name(self) -> &'static str {
        EXTENDED_ACTION_NAMES
            .iter()
            .find(|(action, _)| *action == self)
            .map(|(_, names)| names[0])
            .expect("all ExtendedActions must have names")
    }
}

impl std::fmt::Display for ExtendedAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

// Serde: serialize as the canonical string name
impl Serialize for ExtendedAction {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

impl<'de> Deserialize<'de> for ExtendedAction {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::from_name(&s).ok_or_else(|| serde::de::Error::custom(format!("unknown extended action: {s}")))
    }
}

// ---------------------------------------------------------------------------
// ActionRegistry — tracks extended actions
// ---------------------------------------------------------------------------

/// Metadata for an extended action.
#[derive(Debug, Clone)]
pub struct ExtendedActionDef {
    pub name: String,
    pub description: String,
}

/// Registry for extended actions (plugins, user config).
#[derive(Debug, Clone, Default)]
pub struct ActionRegistry {
    actions: HashMap<String, ExtendedActionDef>,
}

impl ActionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an extended action.
    pub fn register(&mut self, name: &str, description: &str) {
        self.actions.insert(
            name.to_string(),
            ExtendedActionDef {
                name: name.to_string(),
                description: description.to_string(),
            },
        );
    }

    /// Get all registered actions.
    pub fn all(&self) -> impl Iterator<Item = &ExtendedActionDef> {
        self.actions.values()
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

// ---------------------------------------------------------------------------
// Key combo
// ---------------------------------------------------------------------------

/// A single key combination (e.g. `Ctrl+Shift+K`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyCombo {
    pub code: KeyCode,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl KeyCombo {
    pub fn from_event(event: &KeyEvent) -> Self {
        Self {
            code: event.code,
            ctrl: event.modifiers.contains(KeyModifiers::CONTROL),
            alt: event.modifiers.contains(KeyModifiers::ALT),
            shift: event.modifiers.contains(KeyModifiers::SHIFT),
        }
    }
}

// Key string parser lookup table
const KEY_CODE_NAMES: &[(&str, KeyCode)] = &[
    ("enter", KeyCode::Enter),
    ("return", KeyCode::Enter),
    ("cr", KeyCode::Enter),
    ("esc", KeyCode::Esc),
    ("escape", KeyCode::Esc),
    ("tab", KeyCode::Tab),
    ("backspace", KeyCode::Backspace),
    ("bs", KeyCode::Backspace),
    ("delete", KeyCode::Delete),
    ("del", KeyCode::Delete),
    ("up", KeyCode::Up),
    ("down", KeyCode::Down),
    ("left", KeyCode::Left),
    ("right", KeyCode::Right),
    ("home", KeyCode::Home),
    ("end", KeyCode::End),
    ("pageup", KeyCode::PageUp),
    ("pgup", KeyCode::PageUp),
    ("pagedown", KeyCode::PageDown),
    ("pgdn", KeyCode::PageDown),
    ("space", KeyCode::Char(' ')),
    ("spc", KeyCode::Char(' ')),
    ("/", KeyCode::Char('/')),
];

/// Parse a human-readable key string like `"Ctrl+K"`, `"Alt+Enter"`, `"e"`.
fn parse_key_string(s: &str) -> Option<KeyCombo> {
    let parts: Vec<&str> = s.split('+').map(str::trim).collect();
    let key_str = parts.last()?;
    
    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    
    for part in &parts[..parts.len() - 1] {
        match part.to_lowercase().as_str() {
            "ctrl" => ctrl = true,
            "alt" => alt = true,
            "shift" => shift = true,
            _ => {}
        }
    }

    let key_lower = key_str.to_lowercase();
    let code = KEY_CODE_NAMES
        .iter()
        .find(|(name, _)| *name == key_lower)
        .map(|(_, code)| *code)
        .or_else(|| {
            if key_str.len() == 1 {
                key_str.chars().next().map(KeyCode::Char)
            } else {
                None
            }
        })?;

    Some(KeyCombo { code, ctrl, alt, shift })
}

// ---------------------------------------------------------------------------
// Preset enum
// ---------------------------------------------------------------------------

/// Which keymap preset to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum KeymapPreset {
    #[default]
    Helix,
    Vim,
}

impl fmt::Display for KeymapPreset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Helix => write!(f, "helix"),
            Self::Vim => write!(f, "vim"),
        }
    }
}

// ---------------------------------------------------------------------------
// Keymap — mode-aware binding table
// ---------------------------------------------------------------------------

/// Mode-aware keymap. Separate binding tables for normal and insert modes.
#[derive(Debug, Clone)]
pub struct Keymap {
    normal: HashMap<KeyCombo, Action>,
    insert: HashMap<KeyCombo, Action>,
    pub preset: KeymapPreset,
}

impl Keymap {
    /// Resolve a key event in the given mode. Returns `None` for unmapped keys.
    pub fn resolve(&self, mode: InputMode, event: &KeyEvent) -> Option<Action> {
        let combo = KeyCombo::from_event(event);
        match mode {
            InputMode::Normal => self.normal.get(&combo).cloned(),
            InputMode::Insert => self.insert.get(&combo).cloned(),
        }
    }

    /// Build from a preset + optional per-mode user overrides.
    pub fn build(
        preset: KeymapPreset,
        normal_overrides: &HashMap<String, String>,
        insert_overrides: &HashMap<String, String>,
    ) -> Self {
        let (mut normal, mut insert) = match preset {
            KeymapPreset::Helix => (helix_normal(), helix_insert()),
            KeymapPreset::Vim => (vim_normal(), vim_insert()),
        };

        apply_overrides(&mut normal, normal_overrides);
        apply_overrides(&mut insert, insert_overrides);

        Self { normal, insert, preset }
    }

    /// List all bindings for a mode (for /help display).
    pub fn describe(&self, mode: InputMode) -> Vec<(String, Action)> {
        let table = match mode {
            InputMode::Normal => &self.normal,
            InputMode::Insert => &self.insert,
        };
        let mut out: Vec<(String, Action)> = table.iter().map(|(k, a)| (format_key_combo(k), a.clone())).collect();
        out.sort_by(|a, b| format!("{:?}", a.1).cmp(&format!("{:?}", b.1)));
        out
    }
}

impl Default for Keymap {
    fn default() -> Self {
        Self::build(KeymapPreset::default(), &HashMap::new(), &HashMap::new())
    }
}

fn apply_overrides(map: &mut HashMap<KeyCombo, Action>, overrides: &HashMap<String, String>) {
    for (key_str, action_str) in overrides {
        if let (Some(combo), Some(action)) = (parse_key_string(key_str), parse_action(action_str)) {
            map.insert(combo, action);
        }
    }
}

// ===========================================================================
// Keymap building helpers
// ===========================================================================

/// Helper to create a KeyCombo
fn kc(code: KeyCode, ctrl: bool, alt: bool, shift: bool) -> KeyCombo {
    KeyCombo { code, ctrl, alt, shift }
}

/// Helper type for key binding entries
type KeyBinding = (KeyCode, bool, bool, bool, Action);

/// Build a hashmap from a slice of key bindings
fn build_keymap(bindings: &[KeyBinding]) -> HashMap<KeyCombo, Action> {
    bindings
        .iter()
        .map(|(code, ctrl, alt, shift, action)| (kc(*code, *ctrl, *alt, *shift), action.clone()))
        .collect()
}

/// Merge multiple keymaps into one
fn merge_keymaps(maps: &[HashMap<KeyCombo, Action>]) -> HashMap<KeyCombo, Action> {
    let mut result = HashMap::new();
    for map in maps {
        result.extend(map.clone());
    }
    result
}

// ===========================================================================
// Normal mode presets
// ===========================================================================

/// Bindings shared by all presets in normal mode.
fn common_normal() -> HashMap<KeyCombo, Action> {
    use CoreAction::*;
    use ExtendedAction as EA;
    
    build_keymap(&[
        // ── Mode switching ───────────────────────────────
        (KeyCode::Char('i'), false, false, false, Action::Core(EnterInsert)),
        (KeyCode::Char('/'), false, false, false, Action::Core(EnterCommand)),

        // ── Cancel / quit ────────────────────────────────
        (KeyCode::Char('c'), true, false, false, Action::Core(Cancel)),
        (KeyCode::Char('q'), false, false, false, Action::Core(Quit)),

        // ── Scrolling ────────────────────────────────────
        (KeyCode::PageUp, false, false, false, Action::Core(ScrollPageUp)),
        (KeyCode::PageDown, false, false, false, Action::Core(ScrollPageDown)),

        // ── Block operations (universal) ─────────────────
        (KeyCode::Tab, false, false, false, Action::Extended(EA::ToggleBlockCollapse)),
        (KeyCode::Char('y'), false, false, false, Action::Extended(EA::CopyBlock)),
        (KeyCode::Char('e'), false, false, false, Action::Extended(EA::EditBlock)),
        (KeyCode::Char('r'), false, false, false, Action::Extended(EA::RerunBlock)),
        (KeyCode::Esc, false, false, false, Action::Core(Unfocus)),

        // ── Toggles ──────────────────────────────────────
        (KeyCode::Char('t'), true, false, false, Action::Extended(EA::ToggleThinking)),
        (KeyCode::Char('T'), false, false, true, Action::Extended(EA::ToggleShowThinking)),
        (KeyCode::Char('C'), false, false, true, Action::Extended(EA::ToggleCostOverlay)),
        (KeyCode::Char('I'), false, false, true, Action::Extended(EA::ToggleBlockIds)),

        // ── Popups / panels ──────────────────────────────
        (KeyCode::Char('s'), false, false, false, Action::Extended(EA::ToggleSessionPopup)),
        (KeyCode::Char('b'), false, false, false, Action::Extended(EA::ToggleBranchPanel)),
        (KeyCode::Char('B'), false, false, true, Action::Extended(EA::OpenBranchSwitcher)),

        // ── Selectors ─────────────────────────────────────
        (KeyCode::Char('m'), false, false, false, Action::Extended(EA::OpenModelSelector)),
        (KeyCode::Char('a'), false, false, false, Action::Extended(EA::OpenAccountSelector)),

        // ── Leader key (Space) ──────────────────────────
        (KeyCode::Char(' '), false, false, false, Action::Extended(EA::OpenLeaderMenu)),

        // ── External editor ──────────────────────────────
        (KeyCode::Char('o'), false, false, false, Action::Extended(EA::OpenEditor)),

        // ── Search ────────────────────────────────────────
        (KeyCode::Char('f'), false, false, false, Action::Extended(EA::SearchOutput)),
        (KeyCode::Char('f'), true, false, false, Action::Extended(EA::SearchOutput)),
        (KeyCode::Char('n'), false, false, false, Action::Extended(EA::SearchNext)),
        (KeyCode::Char('N'), false, false, true, Action::Extended(EA::SearchPrev)),

        // ── Subagent / Todo panel ────────────────────────
        (KeyCode::Char('`'), false, false, false, Action::Extended(EA::TogglePanelFocus)),
        (KeyCode::Char('`'), true, false, false, Action::Extended(EA::TogglePanelFocus)),
        (KeyCode::Char('x'), true, false, false, Action::Extended(EA::PanelClearDone)),

        // ── Clipboard paste (text or image) ─────────────
        (KeyCode::Char('v'), true, false, false, Action::Core(PasteImage)),
        (KeyCode::Char('v'), true, false, true, Action::Core(PasteImage)),
    ])
}

/// Helix normal mode navigation bindings
fn helix_normal_nav() -> HashMap<KeyCombo, Action> {
    use CoreAction::*;
    use ExtendedAction as EA;
    
    build_keymap(&[
        // ── Block navigation (arrows + jk) ───────────────
        (KeyCode::Up, false, false, false, Action::Core(FocusPrevBlock)),
        (KeyCode::Down, false, false, false, Action::Core(FocusNextBlock)),
        (KeyCode::Char('k'), false, false, false, Action::Core(FocusPrevBlock)),
        (KeyCode::Char('j'), false, false, false, Action::Core(FocusNextBlock)),

        // ── Branch navigation (arrows + hl) ──────────────
        (KeyCode::Left, false, false, false, Action::Extended(EA::BranchPrev)),
        (KeyCode::Right, false, false, false, Action::Extended(EA::BranchNext)),
        (KeyCode::Char('h'), false, false, false, Action::Extended(EA::BranchPrev)),
        (KeyCode::Char('l'), false, false, false, Action::Extended(EA::BranchNext)),

        // ── Scrolling ────────────────────────────────────
        (KeyCode::Char('u'), true, false, false, Action::Core(ScrollPageUp)),
        (KeyCode::Char('d'), true, false, false, Action::Core(ScrollPageDown)),

        // ── Collapse / expand all ────────────────────────
        (KeyCode::Char('K'), false, false, true, Action::Extended(EA::CollapseAllBlocks)),
        (KeyCode::Char('L'), false, false, true, Action::Extended(EA::ExpandAllBlocks)),

        // ── Scroll extremes ──────────────────────────────
        (KeyCode::Char('g'), false, false, false, Action::Core(ScrollToTop)),
        (KeyCode::Char('G'), false, false, true, Action::Core(ScrollToBottom)),
    ])
}

/// Vim normal mode navigation bindings
fn vim_normal_nav() -> HashMap<KeyCombo, Action> {
    use CoreAction::*;
    use ExtendedAction as EA;
    
    build_keymap(&[
        // ── Block navigation (jk + arrows) ───────────────
        (KeyCode::Char('k'), false, false, false, Action::Core(FocusPrevBlock)),
        (KeyCode::Char('j'), false, false, false, Action::Core(FocusNextBlock)),
        (KeyCode::Up, false, false, false, Action::Core(FocusPrevBlock)),
        (KeyCode::Down, false, false, false, Action::Core(FocusNextBlock)),

        // ── Branch navigation (hl + arrows) ──────────────
        (KeyCode::Char('h'), false, false, false, Action::Extended(EA::BranchPrev)),
        (KeyCode::Char('l'), false, false, false, Action::Extended(EA::BranchNext)),
        (KeyCode::Left, false, false, false, Action::Extended(EA::BranchPrev)),
        (KeyCode::Right, false, false, false, Action::Extended(EA::BranchNext)),

        // ── Scrolling ────────────────────────────────────
        (KeyCode::Char('u'), true, false, false, Action::Core(ScrollPageUp)),
        (KeyCode::Char('d'), true, false, false, Action::Core(ScrollPageDown)),

        // ── Collapse / expand all ────────────────────────
        (KeyCode::Char('K'), false, false, true, Action::Extended(EA::CollapseAllBlocks)),
        (KeyCode::Char('L'), false, false, true, Action::Extended(EA::ExpandAllBlocks)),

        // ── Scroll extremes ──────────────────────────────
        (KeyCode::Char('g'), false, false, false, Action::Core(ScrollToTop)),
        (KeyCode::Char('G'), false, false, true, Action::Core(ScrollToBottom)),
    ])
}

/// Helix normal mode.
fn helix_normal() -> HashMap<KeyCombo, Action> {
    merge_keymaps(&[common_normal(), helix_normal_nav()])
}

/// Vim normal mode.
fn vim_normal() -> HashMap<KeyCombo, Action> {
    merge_keymaps(&[common_normal(), vim_normal_nav()])
}

// ===========================================================================
// Insert mode presets
// ===========================================================================

/// Bindings shared by all presets in insert mode.
fn common_insert() -> HashMap<KeyCombo, Action> {
    use CoreAction::*;
    use ExtendedAction as EA;
    
    build_keymap(&[
        // ── Mode switching ───────────────────────────────
        (KeyCode::Esc, false, false, false, Action::Core(EnterNormal)),

        // ── Submit / newline ─────────────────────────────
        (KeyCode::Enter, false, false, false, Action::Core(Submit)),
        (KeyCode::Enter, false, true, false, Action::Core(NewLine)),

        // ── Cancel / quit ────────────────────────────────
        (KeyCode::Char('c'), true, false, false, Action::Core(Cancel)),
        (KeyCode::Char('d'), true, false, false, Action::Core(Quit)),

        // ── Basic editing ────────────────────────────────
        (KeyCode::Backspace, false, false, false, Action::Core(DeleteBack)),
        (KeyCode::Delete, false, false, false, Action::Core(DeleteForward)),

        // ── Arrow movement ───────────────────────────────
        (KeyCode::Left, false, false, false, Action::Core(MoveLeft)),
        (KeyCode::Right, false, false, false, Action::Core(MoveRight)),
        (KeyCode::Home, false, false, false, Action::Core(MoveHome)),
        (KeyCode::End, false, false, false, Action::Core(MoveEnd)),

        // ── History ──────────────────────────────────────
        (KeyCode::Up, false, false, false, Action::Core(HistoryUp)),
        (KeyCode::Down, false, false, false, Action::Core(HistoryDown)),

        // ── Scrolling (Ctrl+arrows) ──────────────────────
        (KeyCode::Up, true, false, false, Action::Core(ScrollUp)),
        (KeyCode::Down, true, false, false, Action::Core(ScrollDown)),
        (KeyCode::PageUp, false, false, false, Action::Core(ScrollPageUp)),
        (KeyCode::PageDown, false, false, false, Action::Core(ScrollPageDown)),
        (KeyCode::Home, true, false, false, Action::Core(ScrollToTop)),
        (KeyCode::End, true, false, false, Action::Core(ScrollToBottom)),

        // ── Menu navigation (Ctrl+j/k, Ctrl+n/p, Tab) ───
        (KeyCode::Char('k'), true, false, false, Action::Core(MenuUp)),
        (KeyCode::Char('j'), true, false, false, Action::Core(MenuDown)),
        (KeyCode::Char('p'), true, false, false, Action::Core(MenuUp)),
        (KeyCode::Char('n'), true, false, false, Action::Core(MenuDown)),
        (KeyCode::Tab, false, false, false, Action::Core(MenuAccept)),

        // ── Search ────────────────────────────────────────
        (KeyCode::Char('f'), true, false, false, Action::Extended(EA::SearchOutput)),

        // ── Panel focus ────────────────────────────────────
        (KeyCode::Char('`'), true, false, false, Action::Extended(EA::TogglePanelFocus)),

        // ── Toggles ───────────────────────────────────────
        (KeyCode::Char('C'), true, false, true, Action::Extended(EA::ToggleCostOverlay)),
        (KeyCode::Char('s'), true, false, false, Action::Extended(EA::ToggleSessionPopup)),
        (KeyCode::Char('b'), true, false, false, Action::Extended(EA::ToggleBranchPanel)),
        (KeyCode::Char('i'), true, false, false, Action::Extended(EA::ToggleBlockIds)),

        // ── Selectors (Ctrl+M model, Ctrl+A account) ────
        (KeyCode::Char('m'), true, false, false, Action::Extended(EA::OpenModelSelector)),
        (KeyCode::Char('a'), true, false, false, Action::Extended(EA::OpenAccountSelector)),

        // ── Clipboard paste (text or image) ─────────────
        (KeyCode::Char('v'), true, false, false, Action::Core(PasteImage)),
        (KeyCode::Char('v'), true, false, true, Action::Core(PasteImage)),

        // ── External editor ──────────────────────────────
        (KeyCode::Char('o'), true, false, false, Action::Extended(EA::OpenEditor)),
    ])
}

/// Readline-style editing shortcuts (shared by helix and vim)
fn readline_shortcuts() -> HashMap<KeyCombo, Action> {
    use CoreAction::*;
    
    build_keymap(&[
        (KeyCode::Char('w'), true, false, false, Action::Core(DeleteWord)),
        (KeyCode::Char('u'), true, false, false, Action::Core(ClearLine)),
        (KeyCode::Char('a'), true, false, false, Action::Core(MoveHome)),
        (KeyCode::Char('e'), true, false, false, Action::Core(MoveEnd)),
    ])
}

/// Helix insert mode (identical to vim insert)
fn helix_insert() -> HashMap<KeyCombo, Action> {
    merge_keymaps(&[common_insert(), readline_shortcuts()])
}

/// Vim insert mode (identical to helix insert)
fn vim_insert() -> HashMap<KeyCombo, Action> {
    merge_keymaps(&[common_insert(), readline_shortcuts()])
}

// ---------------------------------------------------------------------------
// Action parsing
// ---------------------------------------------------------------------------

/// Core action name mappings
const CORE_ACTION_NAMES: &[(CoreAction, &[&str])] = &[
    (CoreAction::EnterInsert, &["enter_insert"]),
    (CoreAction::EnterCommand, &["enter_command"]),
    (CoreAction::EnterNormal, &["enter_normal"]),
    (CoreAction::Submit, &["submit"]),
    (CoreAction::NewLine, &["new_line", "newline"]),
    (CoreAction::Cancel, &["cancel"]),
    (CoreAction::Quit, &["quit"]),
    (CoreAction::MoveLeft, &["move_left"]),
    (CoreAction::MoveRight, &["move_right"]),
    (CoreAction::MoveHome, &["move_home"]),
    (CoreAction::MoveEnd, &["move_end"]),
    (CoreAction::DeleteBack, &["delete_back"]),
    (CoreAction::DeleteForward, &["delete_forward"]),
    (CoreAction::DeleteWord, &["delete_word"]),
    (CoreAction::ClearLine, &["clear_line"]),
    (CoreAction::HistoryUp, &["history_up"]),
    (CoreAction::HistoryDown, &["history_down"]),
    (CoreAction::ScrollUp, &["scroll_up"]),
    (CoreAction::ScrollDown, &["scroll_down"]),
    (CoreAction::ScrollPageUp, &["scroll_page_up", "page_up"]),
    (CoreAction::ScrollPageDown, &["scroll_page_down", "page_down"]),
    (CoreAction::ScrollToTop, &["scroll_to_top"]),
    (CoreAction::ScrollToBottom, &["scroll_to_bottom"]),
    (CoreAction::FocusPrevBlock, &["focus_prev_block", "prev_block"]),
    (CoreAction::FocusNextBlock, &["focus_next_block", "next_block"]),
    (CoreAction::Unfocus, &["unfocus"]),
    (CoreAction::MenuUp, &["menu_up"]),
    (CoreAction::MenuDown, &["menu_down"]),
    (CoreAction::MenuAccept, &["menu_accept"]),
    (CoreAction::MenuClose, &["menu_close"]),
];

fn parse_action(s: &str) -> Option<Action> {
    let normalized = s.to_lowercase().replace('-', "_");
    
    // Try core actions first
    CORE_ACTION_NAMES
        .iter()
        .find(|(_, names)| names.contains(&normalized.as_str()))
        .map(|(action, _)| Action::Core(*action))
        .or_else(|| ExtendedAction::from_name(&normalized).map(Action::Extended))
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

fn format_key_combo(k: &KeyCombo) -> String {
    let mut parts = Vec::new();
    if k.ctrl {
        parts.push("Ctrl".to_string());
    }
    if k.alt {
        parts.push("Alt".to_string());
    }
    if k.shift {
        parts.push("Shift".to_string());
    }
    parts.push(match k.code {
        KeyCode::Char(' ') => "Space".to_string(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Tab => "Tab".to_string(),
        KeyCode::Backspace => "Backspace".to_string(),
        KeyCode::Delete => "Delete".to_string(),
        KeyCode::Up => "Up".to_string(),
        KeyCode::Down => "Down".to_string(),
        KeyCode::Left => "Left".to_string(),
        KeyCode::Right => "Right".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::PageUp => "PageUp".to_string(),
        KeyCode::PageDown => "PageDown".to_string(),
        other => format!("{:?}", other),
    });
    parts.join("+")
}

// ---------------------------------------------------------------------------
// Serialisable config (loaded from settings file)
// ---------------------------------------------------------------------------

/// User-facing keymap configuration (stored in settings.json).
///
/// ```json
/// {
///   "keymap": {
///     "preset": "helix",
///     "normal": { "x": "quit" },
///     "insert": { "Ctrl+K": "delete_word" }
///   }
/// }
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeymapConfig {
    /// Which preset to start from: "helix" (default) or "vim"
    #[serde(default)]
    pub preset: KeymapPreset,

    /// Per-key overrides for normal mode
    #[serde(default)]
    pub normal: HashMap<String, String>,

    /// Per-key overrides for insert mode
    #[serde(default)]
    pub insert: HashMap<String, String>,
}

impl KeymapConfig {
    pub fn load(path: &Path) -> Self {
        std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn into_keymap(self) -> Keymap {
        Keymap::build(self.preset, &self.normal, &self.insert)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn helix() -> Keymap {
        Keymap::build(KeymapPreset::Helix, &HashMap::new(), &HashMap::new())
    }

    fn vim() -> Keymap {
        Keymap::build(KeymapPreset::Vim, &HashMap::new(), &HashMap::new())
    }

    // ── Normal mode ──────────────────────────────────

    #[test]
    fn normal_i_enters_insert() {
        let km = helix();
        let event = KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &event), Some(Action::Core(CoreAction::EnterInsert)));
    }

    #[test]
    fn normal_slash_enters_command() {
        let km = helix();
        let event = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &event), Some(Action::Core(CoreAction::EnterCommand)));
    }

    #[test]
    fn normal_q_quits() {
        let km = helix();
        let event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &event), Some(Action::Core(CoreAction::Quit)));
    }

    #[test]
    fn normal_e_edits_block() {
        let km = helix();
        let event = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &event), Some(Action::Extended(ExtendedAction::EditBlock)));
    }

    #[test]
    fn helix_normal_arrows_navigate_blocks() {
        let km = helix();
        let up = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
        let down = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &up), Some(Action::Core(CoreAction::FocusPrevBlock)));
        assert_eq!(km.resolve(InputMode::Normal, &down), Some(Action::Core(CoreAction::FocusNextBlock)));
    }

    #[test]
    fn helix_normal_jk_navigate_blocks() {
        let km = helix();
        let k = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
        let j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &k), Some(Action::Core(CoreAction::FocusPrevBlock)));
        assert_eq!(km.resolve(InputMode::Normal, &j), Some(Action::Core(CoreAction::FocusNextBlock)));
    }

    #[test]
    fn helix_normal_hl_navigate_branches() {
        let km = helix();
        let h = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE);
        let l = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &h), Some(Action::Extended(ExtendedAction::BranchPrev)));
        assert_eq!(km.resolve(InputMode::Normal, &l), Some(Action::Extended(ExtendedAction::BranchNext)));
    }

    #[test]
    fn helix_normal_left_right_navigate_branches() {
        let km = helix();
        let left = KeyEvent::new(KeyCode::Left, KeyModifiers::NONE);
        let right = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &left), Some(Action::Extended(ExtendedAction::BranchPrev)));
        assert_eq!(km.resolve(InputMode::Normal, &right), Some(Action::Extended(ExtendedAction::BranchNext)));
    }

    #[test]
    fn vim_normal_jk_navigate_blocks() {
        let km = vim();
        let k = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
        let j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &k), Some(Action::Core(CoreAction::FocusPrevBlock)));
        assert_eq!(km.resolve(InputMode::Normal, &j), Some(Action::Core(CoreAction::FocusNextBlock)));
    }

    #[test]
    fn vim_normal_hl_navigate_branches() {
        let km = vim();
        let h = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE);
        let l = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &h), Some(Action::Extended(ExtendedAction::BranchPrev)));
        assert_eq!(km.resolve(InputMode::Normal, &l), Some(Action::Extended(ExtendedAction::BranchNext)));
    }

    #[test]
    fn normal_g_scrolls_to_top() {
        let km = helix();
        let event = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &event), Some(Action::Core(CoreAction::ScrollToTop)));
    }

    #[test]
    fn normal_shift_g_scrolls_to_bottom() {
        let km = helix();
        let event = KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT);
        assert_eq!(km.resolve(InputMode::Normal, &event), Some(Action::Core(CoreAction::ScrollToBottom)));
    }

    // ── Insert mode ──────────────────────────────────

    #[test]
    fn insert_esc_enters_normal() {
        let km = helix();
        let event = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Insert, &event), Some(Action::Core(CoreAction::EnterNormal)));
    }

    #[test]
    fn insert_enter_submits() {
        let km = helix();
        let event = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Insert, &event), Some(Action::Core(CoreAction::Submit)));
    }

    #[test]
    fn insert_alt_enter_newline() {
        let km = helix();
        let event = KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT);
        assert_eq!(km.resolve(InputMode::Insert, &event), Some(Action::Core(CoreAction::NewLine)));
    }

    #[test]
    fn insert_bare_key_unmapped() {
        let km = helix();
        let event = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        // bare 'x' is not mapped in insert — it will be handled as character insertion
        assert_eq!(km.resolve(InputMode::Insert, &event), None);
    }

    #[test]
    fn insert_ctrl_w_deletes_word() {
        let km = helix();
        let event = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL);
        assert_eq!(km.resolve(InputMode::Insert, &event), Some(Action::Core(CoreAction::DeleteWord)));
    }

    // ── Overrides ────────────────────────────────────

    #[test]
    fn user_override_normal_mode() {
        let mut normal = HashMap::new();
        normal.insert("x".to_string(), "quit".to_string());
        let km = Keymap::build(KeymapPreset::Helix, &normal, &HashMap::new());
        let event = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &event), Some(Action::Core(CoreAction::Quit)));
    }

    #[test]
    fn user_override_insert_mode() {
        let mut insert = HashMap::new();
        insert.insert("Ctrl+k".to_string(), "delete_word".to_string());
        let km = Keymap::build(KeymapPreset::Helix, &HashMap::new(), &insert);
        let event = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL);
        assert_eq!(km.resolve(InputMode::Insert, &event), Some(Action::Core(CoreAction::DeleteWord)));
    }

    // ── Config ───────────────────────────────────────

    #[test]
    fn default_preset_is_helix() {
        let config = KeymapConfig::default();
        assert_eq!(config.preset, KeymapPreset::Helix);
    }

    #[test]
    fn describe_normal_not_empty() {
        let km = Keymap::default();
        assert!(!km.describe(InputMode::Normal).is_empty());
    }

    #[test]
    fn describe_insert_not_empty() {
        let km = Keymap::default();
        assert!(!km.describe(InputMode::Insert).is_empty());
    }

    // ── Parse key string ─────────────────────────────

    #[test]
    fn parse_simple_char() {
        let c = parse_key_string("e").unwrap();
        assert_eq!(c.code, KeyCode::Char('e'));
        assert!(!c.ctrl && !c.alt && !c.shift);
    }

    #[test]
    fn parse_ctrl_combo() {
        let c = parse_key_string("Ctrl+k").unwrap();
        assert_eq!(c.code, KeyCode::Char('k'));
        assert!(c.ctrl);
    }

    #[test]
    fn parse_alt_enter() {
        let c = parse_key_string("Alt+Enter").unwrap();
        assert_eq!(c.code, KeyCode::Enter);
        assert!(c.alt);
    }

    // ── Panel navigation ─────────────────────────────

    #[test]
    fn backtick_toggles_panel_focus() {
        let km = helix();
        let event = KeyEvent::new(KeyCode::Char('`'), KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &event), Some(Action::Extended(ExtendedAction::TogglePanelFocus)));
    }

    #[test]
    fn h_l_resolve_to_branch_nav_in_normal() {
        let km = helix();
        let h = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE);
        let l = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE);
        // h/l resolve to BranchPrev/BranchNext, which are remapped to
        // tab switching when panel is focused (in handle_action)
        assert_eq!(km.resolve(InputMode::Normal, &h), Some(Action::Extended(ExtendedAction::BranchPrev)));
        assert_eq!(km.resolve(InputMode::Normal, &l), Some(Action::Extended(ExtendedAction::BranchNext)));
    }

    // ── External editor ──────────────────────────────

    #[test]
    fn normal_o_opens_editor() {
        let km = helix();
        let event = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &event), Some(Action::Extended(ExtendedAction::OpenEditor)));
    }

    #[test]
    fn insert_ctrl_o_opens_editor() {
        let km = helix();
        let event = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL);
        assert_eq!(km.resolve(InputMode::Insert, &event), Some(Action::Extended(ExtendedAction::OpenEditor)));
    }

    #[test]
    fn vim_normal_o_opens_editor() {
        let km = vim();
        let event = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &event), Some(Action::Extended(ExtendedAction::OpenEditor)));
    }

    #[test]
    fn vim_insert_ctrl_o_opens_editor() {
        let km = vim();
        let event = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL);
        assert_eq!(km.resolve(InputMode::Insert, &event), Some(Action::Extended(ExtendedAction::OpenEditor)));
    }

    #[test]
    fn parse_action_open_editor() {
        assert_eq!(parse_action("open_editor"), Some(Action::Extended(ExtendedAction::OpenEditor)));
        assert_eq!(parse_action("editor"), Some(Action::Extended(ExtendedAction::OpenEditor)));
    }

    // ── Leader key ───────────────────────────────────

    #[test]
    fn normal_space_opens_leader_menu() {
        let km = helix();
        let event = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &event), Some(Action::Extended(ExtendedAction::OpenLeaderMenu)));
    }

    #[test]
    fn vim_normal_space_opens_leader_menu() {
        let km = vim();
        let event = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &event), Some(Action::Extended(ExtendedAction::OpenLeaderMenu)));
    }

    #[test]
    fn insert_space_is_unmapped() {
        let km = helix();
        let event = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
        // Space in insert mode should not be mapped — it inserts a space character
        assert_eq!(km.resolve(InputMode::Insert, &event), None);
    }

    #[test]
    fn parse_action_leader_menu() {
        assert_eq!(parse_action("open_leader_menu"), Some(Action::Extended(ExtendedAction::OpenLeaderMenu)));
        assert_eq!(parse_action("leader_menu"), Some(Action::Extended(ExtendedAction::OpenLeaderMenu)));
        assert_eq!(parse_action("leader"), Some(Action::Extended(ExtendedAction::OpenLeaderMenu)));
    }
}
