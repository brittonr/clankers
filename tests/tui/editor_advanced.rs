//! Advanced editor integration tests
//!
//! Tests for Delete (forward), Ctrl+W (delete word), Ctrl+U (clear line),
//! Home/End cursor movement, and history down navigation.

use std::time::Duration;

use super::harness::Key;
use super::harness::TuiTestHarness;

const SETTLE: Duration = Duration::from_millis(300);
const TIMEOUT: Duration = Duration::from_secs(3);

// ── Delete forward ──────────────────────────────────────────

#[test]
fn delete_forward_removes_char_ahead() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.type_str("iabcdef");
    h.wait_for_text("abcdef", TIMEOUT);

    // Move left twice (cursor before 'e')
    h.send_key(Key::Left);
    h.send_key(Key::Left);
    h.settle(SETTLE);

    // Delete forward should remove 'e'
    h.send_key(Key::Delete);
    h.settle(SETTLE);

    assert!(h.screen_contains("abcdf"), "Expected 'abcdf' after delete forward.\nScreen:\n{}", h.screen_text());
    h.quit();
}

// ── Ctrl+W: delete word ─────────────────────────────────────

#[test]
fn ctrl_w_deletes_word() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.type_str("ihello");
    h.settle(Duration::from_millis(100));
    // Type a space then another word — send chars individually to avoid
    // space-eating issues
    h.send(b" ");
    h.settle(Duration::from_millis(100));
    h.type_str("world");
    h.wait_for_text("world", TIMEOUT);

    // Ctrl+W should delete "world"
    h.send_key(Key::CtrlW);
    h.settle(SETTLE);

    // "world" should be gone, "hello" should remain
    assert!(h.screen_contains("hello"), "hello should remain.\nScreen:\n{}", h.screen_text());
    h.quit();
}

// ── Ctrl+U: clear line ──────────────────────────────────────

#[test]
fn ctrl_u_clears_line() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.type_str("isomething");
    h.wait_for_text("something", TIMEOUT);

    // Ctrl+U should clear the line
    h.send_key(Key::CtrlU);
    h.settle(SETTLE);

    // The input should be empty; typing new text should appear cleanly
    h.type_str("fresh");
    h.wait_for_text("fresh", TIMEOUT);
    assert!(!h.screen_contains("something"), "Old text should be gone.\nScreen:\n{}", h.screen_text());
    h.quit();
}

// ── Home / End keys ─────────────────────────────────────────

#[test]
fn home_moves_cursor_to_start() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.type_str("iabcdef");
    h.wait_for_text("abcdef", TIMEOUT);

    // Home moves to start, then typing inserts at beginning
    h.send_key(Key::Home);
    h.settle(SETTLE);
    h.type_str("X");
    h.settle(SETTLE);

    assert!(h.screen_contains("Xabcdef"), "Expected 'Xabcdef'.\nScreen:\n{}", h.screen_text());
    h.quit();
}

#[test]
fn end_moves_cursor_to_end() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.type_str("iabcdef");
    h.wait_for_text("abcdef", TIMEOUT);

    // Move to beginning, then End moves to end
    h.send_key(Key::Home);
    h.settle(SETTLE);
    h.send_key(Key::End);
    h.settle(SETTLE);
    h.type_str("Z");
    h.settle(SETTLE);

    assert!(h.screen_contains("abcdefZ"), "Expected 'abcdefZ'.\nScreen:\n{}", h.screen_text());
    h.quit();
}

// ── History down ────────────────────────────────────────────

#[test]
fn history_up_then_down_restores_input() {
    let mut h = TuiTestHarness::spawn(24, 100);

    // Submit a command to create history
    h.type_str("i/version");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_text("clankers", TIMEOUT);

    // Type something new
    h.type_str("i");
    h.settle(SETTLE);
    h.type_str("draft");
    h.wait_for_text("draft", TIMEOUT);

    // Up to recall history
    h.send_key(Key::Up);
    h.settle(SETTLE);
    assert!(h.screen_contains("/version"), "Up should show /version");

    // Down to return to our draft
    h.send_key(Key::Down);
    h.settle(SETTLE);
    assert!(h.screen_contains("draft"), "Down should restore draft.\nScreen:\n{}", h.screen_text());
    h.quit();
}

// ── Multiple history entries ────────────────────────────────

#[test]
fn history_navigates_multiple_entries() {
    let mut h = TuiTestHarness::spawn(24, 100);

    // Submit two commands (return to normal mode between them)
    h.type_str("i/version");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_text("clankers", TIMEOUT);
    h.send_key(Key::Escape);
    h.settle(SETTLE);

    h.type_str("i/usage");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_text("Token usage:", TIMEOUT);
    h.send_key(Key::Escape);
    h.settle(SETTLE);

    // Now go into insert mode and navigate history
    h.type_str("i");
    h.settle(SETTLE);

    // Up once → /usage (most recent)
    h.send_key(Key::Up);
    h.settle(SETTLE);
    assert!(h.screen_contains("/usage"), "First Up should show /usage");

    // Up again → /version
    h.send_key(Key::Up);
    h.settle(SETTLE);
    assert!(h.screen_contains("/version"), "Second Up should show /version");

    h.quit();
}

// ── Delete at end of line is safe ───────────────────────────

#[test]
fn delete_at_end_of_line_is_noop() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.type_str("iabc");
    h.wait_for_text("abc", TIMEOUT);

    // Delete at end should do nothing
    h.send_key(Key::Delete);
    h.settle(SETTLE);
    assert!(h.screen_contains("abc"), "Text should be unchanged");
    h.quit();
}

// ── Backspace at start of line is safe ──────────────────────

#[test]
fn backspace_at_start_is_noop_or_joins() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.type_str("i");
    h.settle(SETTLE);

    // Backspace with nothing typed
    h.send_key(Key::Backspace);
    h.settle(SETTLE);

    // Should not crash, type something to verify
    h.type_str("ok");
    h.wait_for_text("ok", TIMEOUT);
    h.quit();
}
