//! Tests for block operations: collapse/expand, unfocus, and block-level actions
//!
//! Note: Block collapse, copy, edit, and rerun all operate on *conversation blocks*
//! which require LLM interaction. These tests verify the operations don't crash
//! when there are no conversation blocks, and test Tab/K/L/Escape keybindings.

use std::time::Duration;

use super::harness::Key;
use super::harness::TuiTestHarness;

const SETTLE: Duration = Duration::from_millis(300);
const TIMEOUT: Duration = Duration::from_secs(3);

/// Helper to submit a slash command and return to normal mode
fn run_and_escape(h: &mut TuiTestHarness, cmd: &str) {
    h.type_str(&format!("i{}", cmd));
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.settle(SETTLE);
    h.send_key(Key::Escape);
    h.settle(SETTLE);
}

// ── Tab toggle collapse on no focused block ─────────────────

#[test]
fn tab_toggle_with_no_focus_is_safe() {
    let mut h = TuiTestHarness::spawn(24, 80);
    // In normal mode, Tab should toggle collapse, but with no focused block
    // it should be a no-op
    h.send_key(Key::Tab);
    h.settle(SETTLE);
    // Should still be alive
    assert!(h.status_bar().contains("NORMAL"));
    h.quit();
}

// ── K (Shift+K) collapse all with no blocks ─────────────────

#[test]
fn shift_k_collapse_all_no_blocks() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.type_str("K");
    h.settle(SETTLE);
    assert!(h.status_bar().contains("NORMAL"));
    h.quit();
}

// ── L (Shift+L) expand all with no blocks ───────────────────

#[test]
fn shift_l_expand_all_no_blocks() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.type_str("L");
    h.settle(SETTLE);
    assert!(h.status_bar().contains("NORMAL"));
    h.quit();
}

// ── y (copy block) with no focus ────────────────────────────

#[test]
fn copy_block_no_focus_is_safe() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.type_str("y");
    h.settle(SETTLE);
    assert!(h.status_bar().contains("NORMAL"));
    h.quit();
}

// ── e (edit block) with no focus ────────────────────────────

#[test]
fn edit_block_no_focus_is_safe() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.type_str("e");
    h.settle(SETTLE);
    assert!(h.status_bar().contains("NORMAL"));
    h.quit();
}

// ── r (rerun block) with no focus ───────────────────────────

#[test]
fn rerun_block_no_focus_is_safe() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.type_str("r");
    h.settle(SETTLE);
    assert!(h.status_bar().contains("NORMAL"));
    h.quit();
}

// ── Escape unfocuses (no focused block) ─────────────────────

#[test]
fn escape_unfocus_no_block_is_safe() {
    let mut h = TuiTestHarness::spawn(24, 80);
    // Escape in normal mode with no focus should be a no-op
    h.send_key(Key::Escape);
    h.settle(SETTLE);
    assert!(h.status_bar().contains("NORMAL"));
    h.quit();
}

// ── h/l branch navigation with no blocks ────────────────────

#[test]
fn branch_prev_no_blocks_is_safe() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.type_str("h");
    h.settle(SETTLE);
    assert!(h.status_bar().contains("NORMAL"));
    h.quit();
}

#[test]
fn branch_next_no_blocks_is_safe() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.type_str("l");
    h.settle(SETTLE);
    assert!(h.status_bar().contains("NORMAL"));
    h.quit();
}

// ── All normal-mode keys with no content ────────────────────

#[test]
fn all_normal_mode_keys_are_safe_on_empty() {
    let mut h = TuiTestHarness::spawn(24, 80);

    // Press every normal-mode key to ensure nothing crashes
    let keys = "jkhlyreTKLgG";
    for c in keys.chars() {
        h.type_str(&c.to_string());
        h.settle(Duration::from_millis(100));
    }

    // Tab
    h.send_key(Key::Tab);
    h.settle(Duration::from_millis(100));

    // Ctrl+T
    h.send_key(Key::CtrlT);
    h.settle(Duration::from_millis(100));

    // PageUp/Down
    h.send_key(Key::PageUp);
    h.settle(Duration::from_millis(100));
    h.send_key(Key::PageDown);
    h.settle(Duration::from_millis(100));

    // Arrow keys
    h.send_key(Key::Up);
    h.settle(Duration::from_millis(100));
    h.send_key(Key::Down);
    h.settle(Duration::from_millis(100));
    h.send_key(Key::Left);
    h.settle(Duration::from_millis(100));
    h.send_key(Key::Right);
    h.settle(Duration::from_millis(100));

    // Should still be alive (may have toggled thinking, etc.)
    // Go back to normal mode (Escape) and check
    h.send_key(Key::Escape);
    h.settle(SETTLE);
    // quit might have been triggered by 'q' except we didn't send 'q'
    // The 'T' (shift+T) toggles show_thinking, 'K' collapses, 'L' expands, 'G' scrolls
    // All should be safe
    h.quit();
}

// ── jk with system messages present ─────────────────────────

#[test]
fn focus_navigation_with_system_messages() {
    let mut h = TuiTestHarness::spawn(30, 100);

    // Create system messages
    run_and_escape(&mut h, "/version");
    h.wait_for_text("clankers", TIMEOUT);
    run_and_escape(&mut h, "/status");
    h.wait_for_text("Model:", TIMEOUT);

    // Navigate — system messages aren't focusable conversation blocks
    // so j/k should be no-ops, but shouldn't crash
    h.type_str("k");
    h.settle(SETTLE);
    h.type_str("j");
    h.settle(SETTLE);
    h.type_str("k");
    h.settle(SETTLE);
    h.type_str("j");
    h.settle(SETTLE);

    assert!(h.screen_contains("Messages"));
    h.quit();
}
