//! Modal keymap configuration with preset support (helix, vim)
//!
//! Two modes: **Normal** (navigation, block operations with bare keys) and
//! **Insert** (typing into the editor, only modified keys trigger actions).
//!
//! The interactive event loop resolves key events through the active keymap
//! with the current `InputMode`, instead of hardcoding key checks.

mod actions;
mod defaults;
mod parser;

use std::collections::HashMap;
use std::fmt;

// Re-export public types
pub use actions::{Action, ActionRegistry, CoreAction, ExtendedAction, ExtendedActionDef};
use crossterm::event::KeyEvent;
pub use parser::KeyCombo;
pub use parser::KeymapConfig;
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
            KeymapPreset::Helix => (defaults::helix_normal(), defaults::helix_insert()),
            KeymapPreset::Vim => (defaults::vim_normal(), defaults::vim_insert()),
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
        let mut out: Vec<(String, Action)> =
            table.iter().map(|(k, a)| (parser::format_key_combo(k), a.clone())).collect();
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
        if let (Some(combo), Some(action)) = (parser::parse_key_string(key_str), actions::parse_action(action_str)) {
            map.insert(combo, action);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crossterm::event::KeyCode;
    use crossterm::event::KeyEvent;
    use crossterm::event::KeyModifiers;

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
        let c = parser::parse_key_string("e").expect("should parse simple char key");
        assert_eq!(c.code, KeyCode::Char('e'));
        assert!(!c.ctrl && !c.alt && !c.shift);
    }

    #[test]
    fn parse_ctrl_combo() {
        let c = parser::parse_key_string("Ctrl+k").expect("should parse ctrl combo");
        assert_eq!(c.code, KeyCode::Char('k'));
        assert!(c.ctrl);
    }

    #[test]
    fn parse_alt_enter() {
        let c = parser::parse_key_string("Alt+Enter").expect("should parse alt+enter combo");
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
        assert_eq!(actions::parse_action("open_editor"), Some(Action::Extended(ExtendedAction::OpenEditor)));
        assert_eq!(actions::parse_action("editor"), Some(Action::Extended(ExtendedAction::OpenEditor)));
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
        assert_eq!(actions::parse_action("open_leader_menu"), Some(Action::Extended(ExtendedAction::OpenLeaderMenu)));
        assert_eq!(actions::parse_action("leader_menu"), Some(Action::Extended(ExtendedAction::OpenLeaderMenu)));
        assert_eq!(actions::parse_action("leader"), Some(Action::Extended(ExtendedAction::OpenLeaderMenu)));
    }
}
