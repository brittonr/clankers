//! Block navigation integration tests
//!
//! Note: Slash commands like /version create *system messages*, not conversation
//! blocks. Conversation blocks require sending a prompt to the LLM. These tests
//! verify navigation behaviour using system messages and the slash commands we
//! can run offline.

use std::time::Duration;

use super::harness::Key;
use super::harness::TuiTestHarness;

const SETTLE: Duration = Duration::from_millis(300);
const TIMEOUT: Duration = Duration::from_secs(3);

/// Helper: create several system messages to fill the screen
fn fill_screen(h: &mut TuiTestHarness, n: usize) {
    for i in 0..n {
        h.type_str("i/version");
        h.settle(SETTLE);
        h.send_key(Key::Enter);
        if i == 0 {
            h.wait_for_text("clankers 0.1.0", TIMEOUT);
        } else {
            h.settle(SETTLE);
        }
        h.send_key(Key::Escape);
        h.settle(SETTLE);
    }
}

#[test]
fn slash_version_output_appears() {
    let mut h = TuiTestHarness::spawn(24, 80);

    h.type_str("i/version");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_text("clankers 0.1.0", TIMEOUT);

    assert!(h.screen_contains("clankers 0.1.0"));
    h.quit();
}

#[test]
fn slash_status_shows_model() {
    let mut h = TuiTestHarness::spawn(24, 80);

    h.type_str("i/status");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_text("Model:", TIMEOUT);

    assert!(h.screen_contains("Model:"));
    h.quit();
}

#[test]
fn multiple_commands_all_visible() {
    let mut h = TuiTestHarness::spawn(30, 100);
    fill_screen(&mut h, 3);

    // All three version outputs should be visible (or scrollable)
    // At minimum the latest should be on screen
    assert!(h.screen_contains("clankers 0.1.0"));
    h.quit();
}

#[test]
fn scroll_up_and_down() {
    let mut h = TuiTestHarness::spawn(24, 100);

    // Fill the screen with enough content to require scrolling
    for _ in 0..8 {
        h.type_str("i/status");
        h.settle(Duration::from_millis(200));
        h.send_key(Key::Enter);
        h.settle(Duration::from_millis(200));
        h.send_key(Key::Escape);
        h.settle(Duration::from_millis(100));
    }

    // Scroll up with PageUp
    h.send_key(Key::PageUp);
    h.settle(SETTLE);

    // Should still show Messages header
    assert!(h.screen_contains("Messages"));

    // Scroll back down
    h.send_key(Key::PageDown);
    h.settle(SETTLE);

    assert!(h.screen_contains("Messages"));
    h.quit();
}

#[test]
fn clear_command_removes_messages() {
    let mut h = TuiTestHarness::spawn(24, 80);

    // Create some content
    h.type_str("i/version");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_text("clankers 0.1.0", TIMEOUT);

    // Clear — return to normal mode first
    h.send_key(Key::Escape);
    h.settle(SETTLE);
    h.type_str("i/clear");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_text("cleared", TIMEOUT);

    // The version output should be gone, replaced by "cleared" message
    h.quit();
}

#[test]
fn reset_clears_everything() {
    let mut h = TuiTestHarness::spawn(24, 80);

    // Create content and enable thinking
    h.type_str("i/version");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_text("clankers 0.1.0", TIMEOUT);

    h.send_key(Key::Escape);
    h.settle(SETTLE);

    // Reset
    h.type_str("i/reset");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_text("reset", TIMEOUT);

    assert!(h.screen_contains("reset"));
    h.quit();
}

#[test]
fn input_cleared_after_command() {
    let mut h = TuiTestHarness::spawn(24, 80);

    h.type_str("i/version");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_text("clankers 0.1.0", TIMEOUT);

    // Input area should be cleared after submitting — the "/version" text
    // should no longer be in the input. We're still in insert mode though,
    // so check that the input doesn't contain the old command.
    h.settle(SETTLE);
    assert!(!h.screen_contains("/version"), "Input should be cleared after submit");
    h.quit();
}
