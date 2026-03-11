//! Default keybinding presets (helix and vim).

use std::collections::HashMap;

use clankers_tui_types::Action;
use clankers_tui_types::CoreAction;
use clankers_tui_types::ExtendedAction;
use crossterm::event::KeyCode;

use super::parser::KeyCombo;

// ===========================================================================
// Keymap building helpers
// ===========================================================================

/// Helper to create a KeyCombo
pub(super) fn kc(code: KeyCode, ctrl: bool, alt: bool, shift: bool) -> KeyCombo {
    KeyCombo { code, ctrl, alt, shift }
}

/// Helper type for key binding entries
type KeyBinding = (KeyCode, bool, bool, bool, Action);

/// Build a hashmap from a slice of key bindings
pub(super) fn build_keymap(bindings: &[KeyBinding]) -> HashMap<KeyCombo, Action> {
    bindings
        .iter()
        .map(|(code, ctrl, alt, shift, action)| (kc(*code, *ctrl, *alt, *shift), action.clone()))
        .collect()
}

/// Merge multiple keymaps into one
pub(super) fn merge_keymaps(maps: &[HashMap<KeyCombo, Action>]) -> HashMap<KeyCombo, Action> {
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
    use CoreAction::Cancel;
    use CoreAction::EnterCommand;
    use CoreAction::EnterInsert;
    use CoreAction::PasteImage;
    use CoreAction::Quit;
    use CoreAction::ScrollPageDown;
    use CoreAction::ScrollPageUp;
    use CoreAction::Unfocus;
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
        (KeyCode::Char('P'), false, false, true, Action::Extended(EA::TogglePromptImprove)),
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

/// Normal mode navigation bindings (shared by helix and vim — identical maps).
fn common_normal_nav() -> HashMap<KeyCombo, Action> {
    use CoreAction::FocusNextBlock;
    use CoreAction::FocusPrevBlock;
    use CoreAction::ScrollPageDown;
    use CoreAction::ScrollPageUp;
    use CoreAction::ScrollToBottom;
    use CoreAction::ScrollToTop;
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
pub(super) fn helix_normal() -> HashMap<KeyCombo, Action> {
    merge_keymaps(&[common_normal(), common_normal_nav()])
}

/// Vim normal mode.
pub(super) fn vim_normal() -> HashMap<KeyCombo, Action> {
    merge_keymaps(&[common_normal(), common_normal_nav()])
}

// ===========================================================================
// Insert mode presets
// ===========================================================================

/// Bindings shared by all presets in insert mode.
fn common_insert() -> HashMap<KeyCombo, Action> {
    use CoreAction::Cancel;
    use CoreAction::DeleteBack;
    use CoreAction::DeleteForward;
    use CoreAction::EnterNormal;
    use CoreAction::HistoryDown;
    use CoreAction::HistoryUp;
    use CoreAction::MenuAccept;
    use CoreAction::MenuDown;
    use CoreAction::MenuUp;
    use CoreAction::MoveEnd;
    use CoreAction::MoveHome;
    use CoreAction::MoveLeft;
    use CoreAction::MoveRight;
    use CoreAction::NewLine;
    use CoreAction::PasteImage;
    use CoreAction::Quit;
    use CoreAction::ScrollDown;
    use CoreAction::ScrollPageDown;
    use CoreAction::ScrollPageUp;
    use CoreAction::ScrollToBottom;
    use CoreAction::ScrollToTop;
    use CoreAction::ScrollUp;
    use CoreAction::Submit;
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
        (KeyCode::Char('r'), true, false, false, Action::Extended(EA::TogglePromptImprove)),
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
    use CoreAction::ClearLine;
    use CoreAction::DeleteWord;
    use CoreAction::MoveEnd;
    use CoreAction::MoveHome;

    build_keymap(&[
        (KeyCode::Char('w'), true, false, false, Action::Core(DeleteWord)),
        (KeyCode::Char('u'), true, false, false, Action::Core(ClearLine)),
        (KeyCode::Char('a'), true, false, false, Action::Core(MoveHome)),
        (KeyCode::Char('e'), true, false, false, Action::Core(MoveEnd)),
    ])
}

/// Helix insert mode (identical to vim insert)
pub(super) fn helix_insert() -> HashMap<KeyCombo, Action> {
    merge_keymaps(&[common_insert(), readline_shortcuts()])
}

/// Vim insert mode (identical to helix insert)
pub(super) fn vim_insert() -> HashMap<KeyCombo, Action> {
    merge_keymaps(&[common_insert(), readline_shortcuts()])
}
