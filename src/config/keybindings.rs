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

/// Semantic actions that keybindings can trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    // ── Mode switching ───────────────────────────────
    /// Switch to insert mode (focus the editor)
    EnterInsert,
    /// Switch to insert mode with `/` pre-filled (quick slash command)
    EnterCommand,
    /// Switch to normal mode
    EnterNormal,

    // ── Core ──────────────────────────────────────────
    /// Submit the current input (send prompt / execute command)
    Submit,
    /// Insert a newline in the editor (multi-line input)
    NewLine,
    /// Cancel current operation (abort streaming) or clear input
    Cancel,
    /// Quit the application
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
    ToggleBlockCollapse,
    CollapseAllBlocks,
    ExpandAllBlocks,
    CopyBlock,
    RerunBlock,
    /// Edit the focused block's prompt (starts a branch)
    EditBlock,
    /// Unfocus the current block / dismiss
    Unfocus,

    // ── Branch navigation ────────────────────────────
    BranchPrev,
    BranchNext,
    /// Toggle block ID display in conversation view
    ToggleBlockIds,

    // ── Toggles ──────────────────────────────────────
    ToggleThinking,
    ToggleShowThinking,

    // ── Subagent panel ────────────────────────────────
    // ── Search ────────────────────────────────────
    /// Open the output search overlay
    SearchOutput,
    /// Jump to the next search match
    SearchNext,
    /// Jump to the previous search match
    SearchPrev,

    /// Toggle focus between main TUI and subagent panel
    TogglePanelFocus,
    /// Next subagent tab in the panel
    PanelNextTab,
    /// Previous subagent tab in the panel
    PanelPrevTab,
    /// Scroll up in the panel
    PanelScrollUp,
    /// Scroll down in the panel
    PanelScrollDown,
    /// Clear completed subagents from the panel
    PanelClearDone,
    /// Kill the selected running subagent
    PanelKill,
    /// Remove/dismiss the selected subagent entry from the panel
    PanelRemove,

    // ── Menu navigation (slash command autocomplete) ─
    /// Move selection up in the autocomplete menu
    MenuUp,
    /// Move selection down in the autocomplete menu
    MenuDown,
    /// Accept the selected menu item
    MenuAccept,
    /// Dismiss the autocomplete menu
    MenuClose,

    // ── Clipboard paste ──────────────────────────────
    /// Paste text or image from the system clipboard
    PasteImage,

    // ── Session popup ────────────────────────────────
    /// Toggle the session/branch popup
    ToggleSessionPopup,

    // ── External editor ──────────────────────────────
    /// Open $EDITOR to compose input
    OpenEditor,

    // ── Selectors ────────────────────────────────────
    /// Open the model selector popup
    OpenModelSelector,
    /// Open the account selector popup
    OpenAccountSelector,

    // ── Leader key ───────────────────────────────────
    /// Open the leader key (Space) popup menu
    OpenLeaderMenu,
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
            InputMode::Normal => self.normal.get(&combo).copied(),
            InputMode::Insert => self.insert.get(&combo).copied(),
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
        let mut out: Vec<(String, Action)> = table.iter().map(|(k, a)| (format_key_combo(k), *a)).collect();
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
    let mut m = HashMap::new();

    // ── Mode switching ───────────────────────────────
    m.insert(kc(KeyCode::Char('i'), false, false, false), Action::EnterInsert);
    m.insert(kc(KeyCode::Char('/'), false, false, false), Action::EnterCommand);

    // ── Cancel / quit ────────────────────────────────
    m.insert(kc(KeyCode::Char('c'), true, false, false), Action::Cancel);
    m.insert(kc(KeyCode::Char('q'), false, false, false), Action::Quit);

    // ── Scrolling ────────────────────────────────────
    m.insert(kc(KeyCode::PageUp, false, false, false), Action::ScrollPageUp);
    m.insert(kc(KeyCode::PageDown, false, false, false), Action::ScrollPageDown);

    // ── Block operations (universal) ─────────────────
    m.insert(kc(KeyCode::Tab, false, false, false), Action::ToggleBlockCollapse);
    m.insert(kc(KeyCode::Char('y'), false, false, false), Action::CopyBlock);
    m.insert(kc(KeyCode::Char('e'), false, false, false), Action::EditBlock);
    m.insert(kc(KeyCode::Char('r'), false, false, false), Action::RerunBlock);
    m.insert(kc(KeyCode::Esc, false, false, false), Action::Unfocus);

    // ── Toggles ──────────────────────────────────────
    m.insert(kc(KeyCode::Char('t'), true, false, false), Action::ToggleThinking);
    m.insert(kc(KeyCode::Char('T'), false, false, true), Action::ToggleShowThinking);

    // ── Session popup ─────────────────────────────────
    m.insert(kc(KeyCode::Char('s'), false, false, false), Action::ToggleSessionPopup);

    // ── Block IDs ─────────────────────────────────────
    m.insert(kc(KeyCode::Char('I'), false, false, true), Action::ToggleBlockIds);

    // ── Selectors ─────────────────────────────────────
    m.insert(kc(KeyCode::Char('m'), false, false, false), Action::OpenModelSelector);
    m.insert(kc(KeyCode::Char('a'), false, false, false), Action::OpenAccountSelector);

    // ── Leader key (Space) ──────────────────────────
    m.insert(kc(KeyCode::Char(' '), false, false, false), Action::OpenLeaderMenu);

    // ── External editor ──────────────────────────────
    m.insert(kc(KeyCode::Char('o'), false, false, false), Action::OpenEditor);

    // ── Search ────────────────────────────────────────
    m.insert(kc(KeyCode::Char('f'), false, false, false), Action::SearchOutput);
    m.insert(kc(KeyCode::Char('f'), true, false, false), Action::SearchOutput);
    m.insert(kc(KeyCode::Char('n'), false, false, false), Action::SearchNext);
    m.insert(kc(KeyCode::Char('N'), false, false, true), Action::SearchPrev);

    // ── Subagent / Todo panel ────────────────────────
    m.insert(kc(KeyCode::Char('`'), false, false, false), Action::TogglePanelFocus);
    m.insert(kc(KeyCode::Char('`'), true, false, false), Action::TogglePanelFocus);
    // Tab switching (h/l) is handled contextually when panel is focused —
    // BranchPrev/BranchNext (h/l) are remapped to PanelPrevTab/PanelNextTab.
    m.insert(kc(KeyCode::Char('x'), true, false, false), Action::PanelClearDone);
    // Panel-only keys (x=kill, X=remove) are handled via raw key intercept
    // in the event loop when panel is focused — not in the keymap.

    // ── Clipboard paste (text or image) ─────────────
    m.insert(kc(KeyCode::Char('v'), true, false, false), Action::PasteImage);
    m.insert(kc(KeyCode::Char('v'), true, false, true), Action::PasteImage);

    m
}

/// Helix normal mode.
///
/// Both arrow keys and hjkl for navigation — bare keys are free in normal mode.
fn helix_normal() -> HashMap<KeyCombo, Action> {
    let mut m = common_normal();

    // ── Block navigation (arrows + jk) ───────────────
    m.insert(kc(KeyCode::Up, false, false, false), Action::FocusPrevBlock);
    m.insert(kc(KeyCode::Down, false, false, false), Action::FocusNextBlock);
    m.insert(kc(KeyCode::Char('k'), false, false, false), Action::FocusPrevBlock);
    m.insert(kc(KeyCode::Char('j'), false, false, false), Action::FocusNextBlock);

    // ── Branch navigation (arrows + hl) ──────────────
    m.insert(kc(KeyCode::Left, false, false, false), Action::BranchPrev);
    m.insert(kc(KeyCode::Right, false, false, false), Action::BranchNext);
    m.insert(kc(KeyCode::Char('h'), false, false, false), Action::BranchPrev);
    m.insert(kc(KeyCode::Char('l'), false, false, false), Action::BranchNext);

    // ── Scrolling ────────────────────────────────────
    m.insert(kc(KeyCode::Char('u'), true, false, false), Action::ScrollPageUp);
    m.insert(kc(KeyCode::Char('d'), true, false, false), Action::ScrollPageDown);

    // ── Collapse / expand all ────────────────────────
    m.insert(kc(KeyCode::Char('K'), false, false, true), Action::CollapseAllBlocks);
    m.insert(kc(KeyCode::Char('L'), false, false, true), Action::ExpandAllBlocks);

    // ── Scroll extremes ──────────────────────────────
    m.insert(kc(KeyCode::Char('g'), false, false, false), Action::ScrollToTop);
    m.insert(kc(KeyCode::Char('G'), false, false, true), Action::ScrollToBottom);

    m
}

/// Vim normal mode.
///
/// hjkl for navigation.
fn vim_normal() -> HashMap<KeyCombo, Action> {
    let mut m = common_normal();

    // ── Block navigation (jk + arrows) ───────────────
    m.insert(kc(KeyCode::Char('k'), false, false, false), Action::FocusPrevBlock);
    m.insert(kc(KeyCode::Char('j'), false, false, false), Action::FocusNextBlock);
    m.insert(kc(KeyCode::Up, false, false, false), Action::FocusPrevBlock);
    m.insert(kc(KeyCode::Down, false, false, false), Action::FocusNextBlock);

    // ── Branch navigation (hl + arrows) ──────────────
    m.insert(kc(KeyCode::Char('h'), false, false, false), Action::BranchPrev);
    m.insert(kc(KeyCode::Char('l'), false, false, false), Action::BranchNext);
    m.insert(kc(KeyCode::Left, false, false, false), Action::BranchPrev);
    m.insert(kc(KeyCode::Right, false, false, false), Action::BranchNext);

    // ── Scrolling ────────────────────────────────────
    m.insert(kc(KeyCode::Char('u'), true, false, false), Action::ScrollPageUp);
    m.insert(kc(KeyCode::Char('d'), true, false, false), Action::ScrollPageDown);

    // ── Collapse / expand all ────────────────────────
    m.insert(kc(KeyCode::Char('K'), false, false, true), Action::CollapseAllBlocks);
    m.insert(kc(KeyCode::Char('L'), false, false, true), Action::ExpandAllBlocks);

    // ── Scroll extremes ──────────────────────────────
    m.insert(kc(KeyCode::Char('g'), false, false, false), Action::ScrollToTop);
    m.insert(kc(KeyCode::Char('G'), false, false, true), Action::ScrollToBottom);

    m
}

// ===========================================================================
// Insert mode presets
// ===========================================================================

/// Bindings shared by all presets in insert mode.
fn common_insert() -> HashMap<KeyCombo, Action> {
    let mut m = HashMap::new();

    // ── Mode switching ───────────────────────────────
    m.insert(kc(KeyCode::Esc, false, false, false), Action::EnterNormal);

    // ── Submit / newline ─────────────────────────────
    m.insert(kc(KeyCode::Enter, false, false, false), Action::Submit);
    m.insert(kc(KeyCode::Enter, false, true, false), Action::NewLine);

    // ── Cancel / quit ────────────────────────────────
    m.insert(kc(KeyCode::Char('c'), true, false, false), Action::Cancel);
    m.insert(kc(KeyCode::Char('d'), true, false, false), Action::Quit);

    // ── Basic editing ────────────────────────────────
    m.insert(kc(KeyCode::Backspace, false, false, false), Action::DeleteBack);
    m.insert(kc(KeyCode::Delete, false, false, false), Action::DeleteForward);

    // ── Arrow movement ───────────────────────────────
    m.insert(kc(KeyCode::Left, false, false, false), Action::MoveLeft);
    m.insert(kc(KeyCode::Right, false, false, false), Action::MoveRight);
    m.insert(kc(KeyCode::Home, false, false, false), Action::MoveHome);
    m.insert(kc(KeyCode::End, false, false, false), Action::MoveEnd);

    // ── History ──────────────────────────────────────
    m.insert(kc(KeyCode::Up, false, false, false), Action::HistoryUp);
    m.insert(kc(KeyCode::Down, false, false, false), Action::HistoryDown);

    // ── Scrolling (Ctrl+arrows) ──────────────────────
    m.insert(kc(KeyCode::Up, true, false, false), Action::ScrollUp);
    m.insert(kc(KeyCode::Down, true, false, false), Action::ScrollDown);
    m.insert(kc(KeyCode::PageUp, false, false, false), Action::ScrollPageUp);
    m.insert(kc(KeyCode::PageDown, false, false, false), Action::ScrollPageDown);
    m.insert(kc(KeyCode::Home, true, false, false), Action::ScrollToTop);
    m.insert(kc(KeyCode::End, true, false, false), Action::ScrollToBottom);

    // ── Menu navigation (Ctrl+j/k, Ctrl+n/p, Tab) ───
    m.insert(kc(KeyCode::Char('k'), true, false, false), Action::MenuUp);
    m.insert(kc(KeyCode::Char('j'), true, false, false), Action::MenuDown);
    m.insert(kc(KeyCode::Char('p'), true, false, false), Action::MenuUp);
    m.insert(kc(KeyCode::Char('n'), true, false, false), Action::MenuDown);
    m.insert(kc(KeyCode::Tab, false, false, false), Action::MenuAccept);

    // ── Search ────────────────────────────────────────
    m.insert(kc(KeyCode::Char('f'), true, false, false), Action::SearchOutput);

    // ── Panel focus ────────────────────────────────────
    m.insert(kc(KeyCode::Char('`'), true, false, false), Action::TogglePanelFocus);

    // ── Session popup ────────────────────────────────
    m.insert(kc(KeyCode::Char('s'), true, false, false), Action::ToggleSessionPopup);

    // ── Block IDs ─────────────────────────────────────
    m.insert(kc(KeyCode::Char('i'), true, false, false), Action::ToggleBlockIds);

    // ── Selectors (Ctrl+M model, Ctrl+A account) ────
    m.insert(kc(KeyCode::Char('m'), true, false, false), Action::OpenModelSelector);
    m.insert(kc(KeyCode::Char('a'), true, false, false), Action::OpenAccountSelector);

    // ── Clipboard paste (text or image) ─────────────
    m.insert(kc(KeyCode::Char('v'), true, false, false), Action::PasteImage);
    m.insert(kc(KeyCode::Char('v'), true, false, true), Action::PasteImage);

    // ── External editor ──────────────────────────────
    m.insert(kc(KeyCode::Char('o'), true, false, false), Action::OpenEditor);

    m
}

/// Helix insert mode — emacs-readline shortcuts.
fn helix_insert() -> HashMap<KeyCombo, Action> {
    let mut m = common_insert();

    m.insert(kc(KeyCode::Char('w'), true, false, false), Action::DeleteWord);
    m.insert(kc(KeyCode::Char('u'), true, false, false), Action::ClearLine);
    m.insert(kc(KeyCode::Char('a'), true, false, false), Action::MoveHome);
    m.insert(kc(KeyCode::Char('e'), true, false, false), Action::MoveEnd);

    m
}

/// Vim insert mode — same readline shortcuts.
fn vim_insert() -> HashMap<KeyCombo, Action> {
    let mut m = common_insert();

    m.insert(kc(KeyCode::Char('w'), true, false, false), Action::DeleteWord);
    m.insert(kc(KeyCode::Char('u'), true, false, false), Action::ClearLine);
    m.insert(kc(KeyCode::Char('a'), true, false, false), Action::MoveHome);
    m.insert(kc(KeyCode::Char('e'), true, false, false), Action::MoveEnd);

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
    match s.to_lowercase().replace('-', "_").as_str() {
        "enter_insert" => Some(Action::EnterInsert),
        "enter_command" => Some(Action::EnterCommand),
        "enter_normal" => Some(Action::EnterNormal),
        "submit" => Some(Action::Submit),
        "new_line" | "newline" => Some(Action::NewLine),
        "cancel" => Some(Action::Cancel),
        "quit" => Some(Action::Quit),
        "move_left" => Some(Action::MoveLeft),
        "move_right" => Some(Action::MoveRight),
        "move_home" => Some(Action::MoveHome),
        "move_end" => Some(Action::MoveEnd),
        "delete_back" => Some(Action::DeleteBack),
        "delete_forward" => Some(Action::DeleteForward),
        "delete_word" => Some(Action::DeleteWord),
        "clear_line" => Some(Action::ClearLine),
        "history_up" => Some(Action::HistoryUp),
        "history_down" => Some(Action::HistoryDown),
        "scroll_up" => Some(Action::ScrollUp),
        "scroll_down" => Some(Action::ScrollDown),
        "scroll_page_up" | "page_up" => Some(Action::ScrollPageUp),
        "scroll_page_down" | "page_down" => Some(Action::ScrollPageDown),
        "scroll_to_top" => Some(Action::ScrollToTop),
        "scroll_to_bottom" => Some(Action::ScrollToBottom),
        "focus_prev_block" | "prev_block" => Some(Action::FocusPrevBlock),
        "focus_next_block" | "next_block" => Some(Action::FocusNextBlock),
        "toggle_block_collapse" | "toggle_collapse" => Some(Action::ToggleBlockCollapse),
        "collapse_all_blocks" | "collapse_all" => Some(Action::CollapseAllBlocks),
        "expand_all_blocks" | "expand_all" => Some(Action::ExpandAllBlocks),
        "copy_block" => Some(Action::CopyBlock),
        "rerun_block" => Some(Action::RerunBlock),
        "edit_block" => Some(Action::EditBlock),
        "unfocus" => Some(Action::Unfocus),
        "branch_prev" => Some(Action::BranchPrev),
        "branch_next" => Some(Action::BranchNext),
        "toggle_block_ids" | "toggle_ids" => Some(Action::ToggleBlockIds),
        "toggle_thinking" => Some(Action::ToggleThinking),
        "toggle_show_thinking" => Some(Action::ToggleShowThinking),
        "toggle_panel_focus" | "panel_focus" => Some(Action::TogglePanelFocus),
        "panel_next_tab" | "panel_next" => Some(Action::PanelNextTab),
        "panel_prev_tab" | "panel_prev" => Some(Action::PanelPrevTab),
        "panel_scroll_up" => Some(Action::PanelScrollUp),
        "panel_scroll_down" => Some(Action::PanelScrollDown),
        "panel_clear_done" | "panel_clear" => Some(Action::PanelClearDone),
        "panel_kill" => Some(Action::PanelKill),
        "panel_remove" => Some(Action::PanelRemove),
        "menu_up" => Some(Action::MenuUp),
        "menu_down" => Some(Action::MenuDown),
        "menu_accept" => Some(Action::MenuAccept),
        "menu_close" => Some(Action::MenuClose),
        "toggle_session_popup" | "session_popup" => Some(Action::ToggleSessionPopup),
        "open_editor" | "editor" => Some(Action::OpenEditor),
        "search_output" | "search" | "find" => Some(Action::SearchOutput),
        "search_next" | "next_match" => Some(Action::SearchNext),
        "search_prev" | "prev_match" => Some(Action::SearchPrev),
        "open_leader_menu" | "leader_menu" | "leader" => Some(Action::OpenLeaderMenu),
        _ => None,
    }
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
        assert_eq!(km.resolve(InputMode::Normal, &event), Some(Action::EnterInsert));
    }

    #[test]
    fn normal_slash_enters_command() {
        let km = helix();
        let event = KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &event), Some(Action::EnterCommand));
    }

    #[test]
    fn normal_q_quits() {
        let km = helix();
        let event = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &event), Some(Action::Quit));
    }

    #[test]
    fn normal_e_edits_block() {
        let km = helix();
        let event = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &event), Some(Action::EditBlock));
    }

    #[test]
    fn helix_normal_arrows_navigate_blocks() {
        let km = helix();
        let up = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
        let down = KeyEvent::new(KeyCode::Down, KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &up), Some(Action::FocusPrevBlock));
        assert_eq!(km.resolve(InputMode::Normal, &down), Some(Action::FocusNextBlock));
    }

    #[test]
    fn helix_normal_jk_navigate_blocks() {
        let km = helix();
        let k = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
        let j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &k), Some(Action::FocusPrevBlock));
        assert_eq!(km.resolve(InputMode::Normal, &j), Some(Action::FocusNextBlock));
    }

    #[test]
    fn helix_normal_hl_navigate_branches() {
        let km = helix();
        let h = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE);
        let l = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &h), Some(Action::BranchPrev));
        assert_eq!(km.resolve(InputMode::Normal, &l), Some(Action::BranchNext));
    }

    #[test]
    fn helix_normal_left_right_navigate_branches() {
        let km = helix();
        let left = KeyEvent::new(KeyCode::Left, KeyModifiers::NONE);
        let right = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &left), Some(Action::BranchPrev));
        assert_eq!(km.resolve(InputMode::Normal, &right), Some(Action::BranchNext));
    }

    #[test]
    fn vim_normal_jk_navigate_blocks() {
        let km = vim();
        let k = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
        let j = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &k), Some(Action::FocusPrevBlock));
        assert_eq!(km.resolve(InputMode::Normal, &j), Some(Action::FocusNextBlock));
    }

    #[test]
    fn vim_normal_hl_navigate_branches() {
        let km = vim();
        let h = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE);
        let l = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &h), Some(Action::BranchPrev));
        assert_eq!(km.resolve(InputMode::Normal, &l), Some(Action::BranchNext));
    }

    #[test]
    fn normal_g_scrolls_to_top() {
        let km = helix();
        let event = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &event), Some(Action::ScrollToTop));
    }

    #[test]
    fn normal_shift_g_scrolls_to_bottom() {
        let km = helix();
        let event = KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT);
        assert_eq!(km.resolve(InputMode::Normal, &event), Some(Action::ScrollToBottom));
    }

    // ── Insert mode ──────────────────────────────────

    #[test]
    fn insert_esc_enters_normal() {
        let km = helix();
        let event = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Insert, &event), Some(Action::EnterNormal));
    }

    #[test]
    fn insert_enter_submits() {
        let km = helix();
        let event = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Insert, &event), Some(Action::Submit));
    }

    #[test]
    fn insert_alt_enter_newline() {
        let km = helix();
        let event = KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT);
        assert_eq!(km.resolve(InputMode::Insert, &event), Some(Action::NewLine));
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
        assert_eq!(km.resolve(InputMode::Insert, &event), Some(Action::DeleteWord));
    }

    // ── Overrides ────────────────────────────────────

    #[test]
    fn user_override_normal_mode() {
        let mut normal = HashMap::new();
        normal.insert("x".to_string(), "quit".to_string());
        let km = Keymap::build(KeymapPreset::Helix, &normal, &HashMap::new());
        let event = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &event), Some(Action::Quit));
    }

    #[test]
    fn user_override_insert_mode() {
        let mut insert = HashMap::new();
        insert.insert("Ctrl+k".to_string(), "delete_word".to_string());
        let km = Keymap::build(KeymapPreset::Helix, &HashMap::new(), &insert);
        let event = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL);
        assert_eq!(km.resolve(InputMode::Insert, &event), Some(Action::DeleteWord));
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
        assert_eq!(km.resolve(InputMode::Normal, &event), Some(Action::TogglePanelFocus));
    }

    #[test]
    fn h_l_resolve_to_branch_nav_in_normal() {
        let km = helix();
        let h = KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE);
        let l = KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE);
        // h/l resolve to BranchPrev/BranchNext, which are remapped to
        // tab switching when panel is focused (in handle_action)
        assert_eq!(km.resolve(InputMode::Normal, &h), Some(Action::BranchPrev));
        assert_eq!(km.resolve(InputMode::Normal, &l), Some(Action::BranchNext));
    }

    // ── External editor ──────────────────────────────

    #[test]
    fn normal_o_opens_editor() {
        let km = helix();
        let event = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &event), Some(Action::OpenEditor));
    }

    #[test]
    fn insert_ctrl_o_opens_editor() {
        let km = helix();
        let event = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL);
        assert_eq!(km.resolve(InputMode::Insert, &event), Some(Action::OpenEditor));
    }

    #[test]
    fn vim_normal_o_opens_editor() {
        let km = vim();
        let event = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &event), Some(Action::OpenEditor));
    }

    #[test]
    fn vim_insert_ctrl_o_opens_editor() {
        let km = vim();
        let event = KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL);
        assert_eq!(km.resolve(InputMode::Insert, &event), Some(Action::OpenEditor));
    }

    #[test]
    fn parse_action_open_editor() {
        assert_eq!(parse_action("open_editor"), Some(Action::OpenEditor));
        assert_eq!(parse_action("editor"), Some(Action::OpenEditor));
    }

    // ── Leader key ───────────────────────────────────

    #[test]
    fn normal_space_opens_leader_menu() {
        let km = helix();
        let event = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &event), Some(Action::OpenLeaderMenu));
    }

    #[test]
    fn vim_normal_space_opens_leader_menu() {
        let km = vim();
        let event = KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE);
        assert_eq!(km.resolve(InputMode::Normal, &event), Some(Action::OpenLeaderMenu));
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
        assert_eq!(parse_action("open_leader_menu"), Some(Action::OpenLeaderMenu));
        assert_eq!(parse_action("leader_menu"), Some(Action::OpenLeaderMenu));
        assert_eq!(parse_action("leader"), Some(Action::OpenLeaderMenu));
    }
}
