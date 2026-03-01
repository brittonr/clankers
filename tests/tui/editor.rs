//! Integration tests for the input editor behaviour in the TUI
//!
//! Tests multi-line input, history navigation, backspace, and paste-like input.

use std::time::Duration;

use super::harness::Key;
use super::harness::TuiTestHarness;

const SETTLE: Duration = Duration::from_millis(300);
const TIMEOUT: Duration = Duration::from_secs(3);

// ── Basic typing and backspace ──────────────────────────────

#[test]
fn backspace_deletes_characters() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.type_str("ihello");
    h.wait_for_text("hello", TIMEOUT);

    // Delete the 'o'
    h.send_key(Key::Backspace);
    h.settle(SETTLE);

    // Should show "hell" and not "hello"
    assert!(h.screen_contains("hell"), "Should contain 'hell'");
    // Can't easily assert the 'o' is gone from the input area alone,
    // but we can type more and check
    h.type_str("p");
    h.wait_for_text("hellp", TIMEOUT);
    h.quit();
}

#[test]
fn multi_line_input_with_alt_enter() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.type_str("iline");
    h.settle(Duration::from_millis(100));
    h.type_str(" one");
    h.settle(SETTLE);
    h.send_key(Key::AltEnter);
    h.settle(Duration::from_millis(500));
    h.type_str("second");
    h.wait_for_text("second", TIMEOUT);

    // Both lines should be visible in the editor area
    assert!(h.screen_contains("line one"), "Should show first line.\nScreen:\n{}", h.screen_text());
    assert!(h.screen_contains("second"), "Should show second line.\nScreen:\n{}", h.screen_text());
    h.quit();
}

// ── History navigation ──────────────────────────────────────

#[test]
fn history_up_recalls_previous_command() {
    let mut h = TuiTestHarness::spawn(24, 100);

    // Submit a slash command (which goes through history)
    h.type_str("i/version");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_text("clankers", TIMEOUT);

    // Now press Up to recall the previous input
    // First ensure we're in insert mode
    h.type_str("i");
    h.settle(SETTLE);
    h.send_key(Key::Up);
    h.settle(SETTLE);

    // The editor should now contain /version
    assert!(h.screen_contains("/version"), "Up arrow should recall /version");
    h.quit();
}

// ── Cursor movement ─────────────────────────────────────────

#[test]
fn left_right_arrow_keys_move_cursor() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.type_str("iabcdef");
    h.settle(SETTLE);

    // Move left three times
    h.send_key(Key::Left);
    h.send_key(Key::Left);
    h.send_key(Key::Left);
    h.settle(SETTLE);

    // Type 'X' — should insert at cursor position (after 'c')
    h.type_str("X");
    h.settle(SETTLE);

    assert!(h.screen_contains("abcXdef"), "Insertion after cursor move: expected 'abcXdef'");
    h.quit();
}

// ── Empty submit does nothing ───────────────────────────────

#[test]
fn empty_submit_does_not_crash() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.type_str("i");
    h.settle(SETTLE);

    // Submit with empty input
    h.send_key(Key::Enter);
    h.settle(SETTLE);

    // App should still be responsive
    h.send_key(Key::Escape);
    h.wait_for_text("NORMAL", TIMEOUT);
    h.quit();
}

// ── Rapid typing ────────────────────────────────────────────

#[test]
fn rapid_typing_is_captured() {
    let mut h = TuiTestHarness::spawn(24, 80);
    // Type without spaces to avoid space-eating in PTY
    h.type_str("iabcdefghij");
    h.wait_for_text("abcdefghij", TIMEOUT);
    assert!(h.screen_contains("abcdefghij"), "Screen:\n{}", h.screen_text());
    h.quit();
}
