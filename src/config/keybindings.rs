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

impl ExtendedAction {
    /// Parse from a string name (for keymap config and leader menu).
    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "search_output" | "search" | "find" => Some(Self::SearchOutput),
            "search_next" | "next_match" => Some(Self::SearchNext),
            "search_prev" | "prev_match" => Some(Self::SearchPrev),
            "toggle_block_collapse" | "toggle_collapse" => Some(Self::ToggleBlockCollapse),
            "collapse_all_blocks" | "collapse_all" => Some(Self::CollapseAllBlocks),
            "expand_all_blocks" | "expand_all" => Some(Self::ExpandAllBlocks),
            "copy_block" => Some(Self::CopyBlock),
            "rerun_block" => Some(Self::RerunBlock),
            "edit_block" => Some(Self::EditBlock),
            "branch_prev" => Some(Self::BranchPrev),
            "branch_next" => Some(Self::BranchNext),
            "toggle_block_ids" | "toggle_ids" => Some(Self::ToggleBlockIds),
            "toggle_thinking" => Some(Self::ToggleThinking),
            "toggle_show_thinking" => Some(Self::ToggleShowThinking),
            "toggle_panel_focus" | "panel_focus" => Some(Self::TogglePanelFocus),
            "toggle_cost_overlay" | "cost_overlay" => Some(Self::ToggleCostOverlay),
            "toggle_session_popup" | "session_popup" => Some(Self::ToggleSessionPopup),
            "toggle_branch_panel" | "branch_panel" => Some(Self::ToggleBranchPanel),
            "panel_next_tab" | "panel_next" => Some(Self::PanelNextTab),
            "panel_prev_tab" | "panel_prev" => Some(Self::PanelPrevTab),
            "panel_scroll_up" => Some(Self::PanelScrollUp),
            "panel_scroll_down" => Some(Self::PanelScrollDown),
            "panel_clear_done" | "panel_clear" => Some(Self::PanelClearDone),
            "panel_kill" => Some(Self::PanelKill),
            "panel_remove" => Some(Self::PanelRemove),
            "open_leader_menu" | "leader_menu" | "leader" => Some(Self::OpenLeaderMenu),
            "open_model_selector" | "model_selector" => Some(Self::OpenModelSelector),
            "open_account_selector" | "account_selector" => Some(Self::OpenAccountSelector),
            "open_branch_switcher" | "branch_switcher" => Some(Self::OpenBranchSwitcher),
            "open_editor" | "editor" => Some(Self::OpenEditor),
            "paste_image" => Some(Self::PasteImage),
            "pane_zoom" | "zoom" | "zoom_toggle" => Some(Self::PaneZoom),
            "pane_split_horizontal" => Some(Self::PaneSplitHorizontal),
            "pane_split_vertical" => Some(Self::PaneSplitVertical),
            "pane_close" => Some(Self::PaneClose),
            "pane_equalize" => Some(Self::PaneEqualize),
            "pane_grow" => Some(Self::PaneGrow),
            "pane_shrink" => Some(Self::PaneShrink),
            "pane_move_left" => Some(Self::PaneMoveLeft),
            "pane_move_right" => Some(Self::PaneMoveRight),
            "pane_move_up" => Some(Self::PaneMoveUp),
            "pane_move_down" => Some(Self::PaneMoveDown),
            _ => None,
        }
    }

    /// Canonical string name (for serialization and display).
    pub fn name(self) -> &'static str {
        match self {
            Self::SearchOutput => "search_output",
            Self::SearchNext => "search_next",
            Self::SearchPrev => "search_prev",
            Self::ToggleBlockCollapse => "toggle_block_collapse",
            Self::CollapseAllBlocks => "collapse_all_blocks",
            Self::ExpandAllBlocks => "expand_all_blocks",
            Self::CopyBlock => "copy_block",
            Self::RerunBlock => "rerun_block",
            Self::EditBlock => "edit_block",
            Self::BranchPrev => "branch_prev",
            Self::BranchNext => "branch_next",
            Self::ToggleBlockIds => "toggle_block_ids",
            Self::ToggleThinking => "toggle_thinking",
            Self::ToggleShowThinking => "toggle_show_thinking",
            Self::TogglePanelFocus => "toggle_panel_focus",
            Self::ToggleCostOverlay => "toggle_cost_overlay",
            Self::ToggleSessionPopup => "toggle_session_popup",
            Self::ToggleBranchPanel => "toggle_branch_panel",
            Self::PanelNextTab => "panel_next_tab",
            Self::PanelPrevTab => "panel_prev_tab",
            Self::PanelScrollUp => "panel_scroll_up",
            Self::PanelScrollDown => "panel_scroll_down",
            Self::PanelClearDone => "panel_clear_done",
            Self::PanelKill => "panel_kill",
            Self::PanelRemove => "panel_remove",
            Self::OpenLeaderMenu => "open_leader_menu",
            Self::OpenModelSelector => "open_model_selector",
            Self::OpenAccountSelector => "open_account_selector",
            Self::OpenBranchSwitcher => "open_branch_switcher",
            Self::OpenEditor => "open_editor",
            Self::PasteImage => "paste_image",
            Self::PaneZoom => "pane_zoom",
            Self::PaneSplitHorizontal => "pane_split_horizontal",
            Self::PaneSplitVertical => "pane_split_vertical",
            Self::PaneClose => "pane_close",
            Self::PaneEqualize => "pane_equalize",
            Self::PaneGrow => "pane_grow",
            Self::PaneShrink => "pane_shrink",
            Self::PaneMoveLeft => "pane_move_left",
            Self::PaneMoveRight => "pane_move_right",
            Self::PaneMoveUp => "pane_move_up",
            Self::PaneMoveDown => "pane_move_down",
        }
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

    /// Check if an action is registered.
    pub fn is_registered(&self, name: &str) -> bool {
        self.actions.contains_key(name)
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

    let code = match key_str.to_lowercase().as_str() {
        "enter" | "return" | "cr" => KeyCode::Enter,
        "esc" | "escape" => KeyCode::Esc,
        "tab" => KeyCode::Tab,
        "backspace" | "bs" => KeyCode::Backspace,
        "delete" | "del" => KeyCode::Delete,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" | "pgup" => KeyCode::PageUp,
        "pagedown" | "pgdn" => KeyCode::PageDown,
        "space" | "spc" => KeyCode::Char(' '),
        "/" => KeyCode::Char('/'),
        s if s.len() == 1 => KeyCode::Char(s.chars().next().unwrap()),
        _ => return None,
    };

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
// Normal mode presets
// ===========================================================================

/// Bindings shared by all presets in normal mode.
fn common_normal() -> HashMap<KeyCombo, Action> {
    use CoreAction::*;
    let mut m = HashMap::new();

    // ── Mode switching ───────────────────────────────
    m.insert(kc(KeyCode::Char('i'), false, false, false), Action::Core(EnterInsert));
    m.insert(kc(KeyCode::Char('/'), false, false, false), Action::Core(EnterCommand));

    // ── Cancel / quit ────────────────────────────────
    m.insert(kc(KeyCode::Char('c'), true, false, false), Action::Core(Cancel));
    m.insert(kc(KeyCode::Char('q'), false, false, false), Action::Core(Quit));

    // ── Scrolling ────────────────────────────────────
    m.insert(kc(KeyCode::PageUp, false, false, false), Action::Core(ScrollPageUp));
    m.insert(kc(KeyCode::PageDown, false, false, false), Action::Core(ScrollPageDown));

    // ── Block operations (universal) ─────────────────
    m.insert(kc(KeyCode::Tab, false, false, false), Action::Extended(ExtendedAction::ToggleBlockCollapse));
    m.insert(kc(KeyCode::Char('y'), false, false, false), Action::Extended(ExtendedAction::CopyBlock));
    m.insert(kc(KeyCode::Char('e'), false, false, false), Action::Extended(ExtendedAction::EditBlock));
    m.insert(kc(KeyCode::Char('r'), false, false, false), Action::Extended(ExtendedAction::RerunBlock));
    m.insert(kc(KeyCode::Esc, false, false, false), Action::Core(Unfocus));

    // ── Toggles ──────────────────────────────────────
    m.insert(kc(KeyCode::Char('t'), true, false, false), Action::Extended(ExtendedAction::ToggleThinking));
    m.insert(kc(KeyCode::Char('T'), false, false, true), Action::Extended(ExtendedAction::ToggleShowThinking));

    // ── Cost overlay ──────────────────────────────────
    m.insert(kc(KeyCode::Char('C'), false, false, true), Action::Extended(ExtendedAction::ToggleCostOverlay));

    // ── Session popup ─────────────────────────────────
    m.insert(kc(KeyCode::Char('s'), false, false, false), Action::Extended(ExtendedAction::ToggleSessionPopup));

    // ── Branch panel / switcher ────────────────────────
    m.insert(kc(KeyCode::Char('b'), false, false, false), Action::Extended(ExtendedAction::ToggleBranchPanel));
    m.insert(kc(KeyCode::Char('B'), false, false, true), Action::Extended(ExtendedAction::OpenBranchSwitcher));

    // ── Block IDs ─────────────────────────────────────
    m.insert(kc(KeyCode::Char('I'), false, false, true), Action::Extended(ExtendedAction::ToggleBlockIds));

    // ── Selectors ─────────────────────────────────────
    m.insert(kc(KeyCode::Char('m'), false, false, false), Action::Extended(ExtendedAction::OpenModelSelector));
    m.insert(kc(KeyCode::Char('a'), false, false, false), Action::Extended(ExtendedAction::OpenAccountSelector));

    // ── Leader key (Space) ──────────────────────────
    m.insert(kc(KeyCode::Char(' '), false, false, false), Action::Extended(ExtendedAction::OpenLeaderMenu));

    // ── External editor ──────────────────────────────
    m.insert(kc(KeyCode::Char('o'), false, false, false), Action::Extended(ExtendedAction::OpenEditor));

    // ── Search ────────────────────────────────────────
    m.insert(kc(KeyCode::Char('f'), false, false, false), Action::Extended(ExtendedAction::SearchOutput));
    m.insert(kc(KeyCode::Char('f'), true, false, false), Action::Extended(ExtendedAction::SearchOutput));
    m.insert(kc(KeyCode::Char('n'), false, false, false), Action::Extended(ExtendedAction::SearchNext));
    m.insert(kc(KeyCode::Char('N'), false, false, true), Action::Extended(ExtendedAction::SearchPrev));

    // ── Subagent / Todo panel ────────────────────────
    m.insert(kc(KeyCode::Char('`'), false, false, false), Action::Extended(ExtendedAction::TogglePanelFocus));
    m.insert(kc(KeyCode::Char('`'), true, false, false), Action::Extended(ExtendedAction::TogglePanelFocus));
    // Tab switching (h/l) is handled contextually when panel is focused —
    // BranchPrev/BranchNext (h/l) are remapped to PanelPrevTab/PanelNextTab.
    m.insert(kc(KeyCode::Char('x'), true, false, false), Action::Extended(ExtendedAction::PanelClearDone));
    // Panel-only keys (x=kill, X=remove) are handled via raw key intercept
    // in the event loop when panel is focused — not in the keymap.

    // ── Clipboard paste (text or image) ─────────────
    m.insert(kc(KeyCode::Char('v'), true, false, false), Action::Core(PasteImage));
    m.insert(kc(KeyCode::Char('v'), true, false, true), Action::Core(PasteImage));

    m
}

/// Helix normal mode.
///
/// Both arrow keys and hjkl for navigation — bare keys are free in normal mode.
fn helix_normal() -> HashMap<KeyCombo, Action> {
    use CoreAction::*;
    let mut m = common_normal();

    // ── Block navigation (arrows + jk) ───────────────
    m.insert(kc(KeyCode::Up, false, false, false), Action::Core(FocusPrevBlock));
    m.insert(kc(KeyCode::Down, false, false, false), Action::Core(FocusNextBlock));
    m.insert(kc(KeyCode::Char('k'), false, false, false), Action::Core(FocusPrevBlock));
    m.insert(kc(KeyCode::Char('j'), false, false, false), Action::Core(FocusNextBlock));

    // ── Branch navigation (arrows + hl) ──────────────
    m.insert(kc(KeyCode::Left, false, false, false), Action::Extended(ExtendedAction::BranchPrev));
    m.insert(kc(KeyCode::Right, false, false, false), Action::Extended(ExtendedAction::BranchNext));
    m.insert(kc(KeyCode::Char('h'), false, false, false), Action::Extended(ExtendedAction::BranchPrev));
    m.insert(kc(KeyCode::Char('l'), false, false, false), Action::Extended(ExtendedAction::BranchNext));

    // ── Scrolling ────────────────────────────────────
    m.insert(kc(KeyCode::Char('u'), true, false, false), Action::Core(ScrollPageUp));
    m.insert(kc(KeyCode::Char('d'), true, false, false), Action::Core(ScrollPageDown));

    // ── Collapse / expand all ────────────────────────
    m.insert(kc(KeyCode::Char('K'), false, false, true), Action::Extended(ExtendedAction::CollapseAllBlocks));
    m.insert(kc(KeyCode::Char('L'), false, false, true), Action::Extended(ExtendedAction::ExpandAllBlocks));

    // ── Scroll extremes ──────────────────────────────
    m.insert(kc(KeyCode::Char('g'), false, false, false), Action::Core(ScrollToTop));
    m.insert(kc(KeyCode::Char('G'), false, false, true), Action::Core(ScrollToBottom));

    m
}

/// Vim normal mode.
///
/// hjkl for navigation.
fn vim_normal() -> HashMap<KeyCombo, Action> {
    use CoreAction::*;
    let mut m = common_normal();

    // ── Block navigation (jk + arrows) ───────────────
    m.insert(kc(KeyCode::Char('k'), false, false, false), Action::Core(FocusPrevBlock));
    m.insert(kc(KeyCode::Char('j'), false, false, false), Action::Core(FocusNextBlock));
    m.insert(kc(KeyCode::Up, false, false, false), Action::Core(FocusPrevBlock));
    m.insert(kc(KeyCode::Down, false, false, false), Action::Core(FocusNextBlock));

    // ── Branch navigation (hl + arrows) ──────────────
    m.insert(kc(KeyCode::Char('h'), false, false, false), Action::Extended(ExtendedAction::BranchPrev));
    m.insert(kc(KeyCode::Char('l'), false, false, false), Action::Extended(ExtendedAction::BranchNext));
    m.insert(kc(KeyCode::Left, false, false, false), Action::Extended(ExtendedAction::BranchPrev));
    m.insert(kc(KeyCode::Right, false, false, false), Action::Extended(ExtendedAction::BranchNext));

    // ── Scrolling ────────────────────────────────────
    m.insert(kc(KeyCode::Char('u'), true, false, false), Action::Core(ScrollPageUp));
    m.insert(kc(KeyCode::Char('d'), true, false, false), Action::Core(ScrollPageDown));

    // ── Collapse / expand all ────────────────────────
    m.insert(kc(KeyCode::Char('K'), false, false, true), Action::Extended(ExtendedAction::CollapseAllBlocks));
    m.insert(kc(KeyCode::Char('L'), false, false, true), Action::Extended(ExtendedAction::ExpandAllBlocks));

    // ── Scroll extremes ──────────────────────────────
    m.insert(kc(KeyCode::Char('g'), false, false, false), Action::Core(ScrollToTop));
    m.insert(kc(KeyCode::Char('G'), false, false, true), Action::Core(ScrollToBottom));

    m
}

// ===========================================================================
// Insert mode presets
// ===========================================================================

/// Bindings shared by all presets in insert mode.
fn common_insert() -> HashMap<KeyCombo, Action> {
    use CoreAction::*;
    let mut m = HashMap::new();

    // ── Mode switching ───────────────────────────────
    m.insert(kc(KeyCode::Esc, false, false, false), Action::Core(EnterNormal));

    // ── Submit / newline ─────────────────────────────
    m.insert(kc(KeyCode::Enter, false, false, false), Action::Core(Submit));
    m.insert(kc(KeyCode::Enter, false, true, false), Action::Core(NewLine));

    // ── Cancel / quit ────────────────────────────────
    m.insert(kc(KeyCode::Char('c'), true, false, false), Action::Core(Cancel));
    m.insert(kc(KeyCode::Char('d'), true, false, false), Action::Core(Quit));

    // ── Basic editing ────────────────────────────────
    m.insert(kc(KeyCode::Backspace, false, false, false), Action::Core(DeleteBack));
    m.insert(kc(KeyCode::Delete, false, false, false), Action::Core(DeleteForward));

    // ── Arrow movement ───────────────────────────────
    m.insert(kc(KeyCode::Left, false, false, false), Action::Core(MoveLeft));
    m.insert(kc(KeyCode::Right, false, false, false), Action::Core(MoveRight));
    m.insert(kc(KeyCode::Home, false, false, false), Action::Core(MoveHome));
    m.insert(kc(KeyCode::End, false, false, false), Action::Core(MoveEnd));

    // ── History ──────────────────────────────────────
    m.insert(kc(KeyCode::Up, false, false, false), Action::Core(HistoryUp));
    m.insert(kc(KeyCode::Down, false, false, false), Action::Core(HistoryDown));

    // ── Scrolling (Ctrl+arrows) ──────────────────────
    m.insert(kc(KeyCode::Up, true, false, false), Action::Core(ScrollUp));
    m.insert(kc(KeyCode::Down, true, false, false), Action::Core(ScrollDown));
    m.insert(kc(KeyCode::PageUp, false, false, false), Action::Core(ScrollPageUp));
    m.insert(kc(KeyCode::PageDown, false, false, false), Action::Core(ScrollPageDown));
    m.insert(kc(KeyCode::Home, true, false, false), Action::Core(ScrollToTop));
    m.insert(kc(KeyCode::End, true, false, false), Action::Core(ScrollToBottom));

    // ── Menu navigation (Ctrl+j/k, Ctrl+n/p, Tab) ───
    m.insert(kc(KeyCode::Char('k'), true, false, false), Action::Core(MenuUp));
    m.insert(kc(KeyCode::Char('j'), true, false, false), Action::Core(MenuDown));
    m.insert(kc(KeyCode::Char('p'), true, false, false), Action::Core(MenuUp));
    m.insert(kc(KeyCode::Char('n'), true, false, false), Action::Core(MenuDown));
    m.insert(kc(KeyCode::Tab, false, false, false), Action::Core(MenuAccept));

    // ── Search ────────────────────────────────────────
    m.insert(kc(KeyCode::Char('f'), true, false, false), Action::Extended(ExtendedAction::SearchOutput));

    // ── Panel focus ────────────────────────────────────
    m.insert(kc(KeyCode::Char('`'), true, false, false), Action::Extended(ExtendedAction::TogglePanelFocus));

    // ── Cost overlay ──────────────────────────────────
    m.insert(kc(KeyCode::Char('C'), true, false, true), Action::Extended(ExtendedAction::ToggleCostOverlay));

    // ── Session popup ────────────────────────────────
    m.insert(kc(KeyCode::Char('s'), true, false, false), Action::Extended(ExtendedAction::ToggleSessionPopup));

    // ── Branch panel ──────────────────────────────────
    m.insert(kc(KeyCode::Char('b'), true, false, false), Action::Extended(ExtendedAction::ToggleBranchPanel));

    // ── Block IDs ─────────────────────────────────────
    m.insert(kc(KeyCode::Char('i'), true, false, false), Action::Extended(ExtendedAction::ToggleBlockIds));

    // ── Selectors (Ctrl+M model, Ctrl+A account) ────
    m.insert(kc(KeyCode::Char('m'), true, false, false), Action::Extended(ExtendedAction::OpenModelSelector));
    m.insert(kc(KeyCode::Char('a'), true, false, false), Action::Extended(ExtendedAction::OpenAccountSelector));

    // ── Clipboard paste (text or image) ─────────────
    m.insert(kc(KeyCode::Char('v'), true, false, false), Action::Core(PasteImage));
    m.insert(kc(KeyCode::Char('v'), true, false, true), Action::Core(PasteImage));

    // ── External editor ──────────────────────────────
    m.insert(kc(KeyCode::Char('o'), true, false, false), Action::Extended(ExtendedAction::OpenEditor));

    m
}

/// Helix insert mode — emacs-readline shortcuts.
fn helix_insert() -> HashMap<KeyCombo, Action> {
    use CoreAction::*;
    let mut m = common_insert();

    m.insert(kc(KeyCode::Char('w'), true, false, false), Action::Core(DeleteWord));
    m.insert(kc(KeyCode::Char('u'), true, false, false), Action::Core(ClearLine));
    m.insert(kc(KeyCode::Char('a'), true, false, false), Action::Core(MoveHome));
    m.insert(kc(KeyCode::Char('e'), true, false, false), Action::Core(MoveEnd));

    m
}

/// Vim insert mode — same readline shortcuts.
fn vim_insert() -> HashMap<KeyCombo, Action> {
    use CoreAction::*;
    let mut m = common_insert();

    m.insert(kc(KeyCode::Char('w'), true, false, false), Action::Core(DeleteWord));
    m.insert(kc(KeyCode::Char('u'), true, false, false), Action::Core(ClearLine));
    m.insert(kc(KeyCode::Char('a'), true, false, false), Action::Core(MoveHome));
    m.insert(kc(KeyCode::Char('e'), true, false, false), Action::Core(MoveEnd));

    m
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn kc(code: KeyCode, ctrl: bool, alt: bool, shift: bool) -> KeyCombo {
    KeyCombo { code, ctrl, alt, shift }
}

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

fn parse_action(s: &str) -> Option<Action> {
    use CoreAction::*;
    
    let normalized = s.to_lowercase().replace('-', "_");
    
    // Core actions
    let core = match normalized.as_str() {
        "enter_insert" => Some(EnterInsert),
        "enter_command" => Some(EnterCommand),
        "enter_normal" => Some(EnterNormal),
        "submit" => Some(Submit),
        "new_line" | "newline" => Some(NewLine),
        "cancel" => Some(Cancel),
        "quit" => Some(Quit),
        "move_left" => Some(MoveLeft),
        "move_right" => Some(MoveRight),
        "move_home" => Some(MoveHome),
        "move_end" => Some(MoveEnd),
        "delete_back" => Some(DeleteBack),
        "delete_forward" => Some(DeleteForward),
        "delete_word" => Some(DeleteWord),
        "clear_line" => Some(ClearLine),
        "history_up" => Some(HistoryUp),
        "history_down" => Some(HistoryDown),
        "scroll_up" => Some(ScrollUp),
        "scroll_down" => Some(ScrollDown),
        "scroll_page_up" | "page_up" => Some(ScrollPageUp),
        "scroll_page_down" | "page_down" => Some(ScrollPageDown),
        "scroll_to_top" => Some(ScrollToTop),
        "scroll_to_bottom" => Some(ScrollToBottom),
        "focus_prev_block" | "prev_block" => Some(FocusPrevBlock),
        "focus_next_block" | "next_block" => Some(FocusNextBlock),
        "unfocus" => Some(Unfocus),
        "menu_up" => Some(MenuUp),
        "menu_down" => Some(MenuDown),
        "menu_accept" => Some(MenuAccept),
        "menu_close" => Some(MenuClose),
        _ => None,
    };
    
    if let Some(core_action) = core {
        return Some(Action::Core(core_action));
    }
    
    // Extended actions — parsed through the ExtendedAction enum
    ExtendedAction::from_name(&normalized).map(Action::Extended)
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
