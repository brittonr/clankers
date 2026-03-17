//! Integration tests for normal-mode navigation, block focus, and collapse

use std::time::Duration;

use super::harness::Key;
use super::harness::TuiTestHarness;

const SETTLE: Duration = Duration::from_millis(300);
const TIMEOUT: Duration = Duration::from_secs(5);

/// Helper: submit a slash command and return to normal mode
fn run_and_escape(h: &mut TuiTestHarness, cmd: &str) {
    h.type_str(&format!("i{}", cmd));
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.settle(SETTLE);
    h.send_key(Key::Escape);
    h.settle(SETTLE);
}

// ── Mode switching ──────────────────────────────────────────

#[test]
fn slash_key_enters_command_mode() {
    let mut h = TuiTestHarness::spawn(24, 80);
    // In normal mode, pressing '/' should enter command mode (insert with '/')
    h.type_str("/");
    h.settle(SETTLE);
    // The '/' should appear in the input and we should be in insert mode
    assert!(h.status_bar().contains("INSERT") || h.screen_contains("/"), "/ should enter insert/command mode");
    h.quit();
}

#[test]
fn ctrl_c_from_normal_mode() {
    let mut h = TuiTestHarness::spawn(24, 80);
    // Ctrl+C should quit or cancel
    h.send_key(Key::CtrlC);
    h.settle(Duration::from_millis(500));
    // Process may have exited — that's fine
}

// ── j/k block navigation ────────────────────────────────────

#[test]
fn jk_navigation_between_system_messages() {
    // Create several messages, then navigate with j and k
    let mut h = TuiTestHarness::spawn(30, 100);

    // Create some system messages
    run_and_escape(&mut h, "/version");
    h.wait_for_text("clankers", TIMEOUT);
    run_and_escape(&mut h, "/status");
    h.wait_for_text("Model:", TIMEOUT);
    run_and_escape(&mut h, "/usage");
    h.wait_for_text("Token usage:", TIMEOUT);

    // Now in normal mode, j/k should work (at minimum not crash)
    h.type_str("k"); // focus prev
    h.settle(SETTLE);
    h.type_str("k"); // focus prev again
    h.settle(SETTLE);
    h.type_str("j"); // focus next
    h.settle(SETTLE);

    // App should still be alive
    assert!(h.screen_contains("Messages"));
    h.quit();
}

// ── g and G: scroll to top/bottom ───────────────────────────

#[test]
fn g_scrolls_to_top() {
    let mut h = TuiTestHarness::spawn(24, 100);

    // Fill screen with content
    for _ in 0..6 {
        run_and_escape(&mut h, "/status");
        h.settle(Duration::from_millis(100));
    }

    // g should scroll to top
    h.type_str("g");
    h.settle(SETTLE);
    assert!(h.screen_contains("Messages"));

    // G should scroll to bottom
    h.type_str("G");
    h.settle(SETTLE);
    assert!(h.screen_contains("Messages"));

    h.quit();
}

// ── Multiple mode transitions ───────────────────────────────

#[test]
fn repeated_mode_transitions() {
    let mut h = TuiTestHarness::spawn(24, 80);

    for _ in 0..5 {
        h.type_str("i");
        h.wait_for_text("INSERT", TIMEOUT);
        h.send_key(Key::Escape);
        h.wait_for_text("NORMAL", TIMEOUT);
    }

    // Should still be responsive
    assert!(h.status_bar().contains("NORMAL"));
    h.quit();
}

// ── PageUp / PageDown ───────────────────────────────────────

#[test]
fn page_up_down_does_not_crash() {
    let mut h = TuiTestHarness::spawn(24, 80);

    // Fill some content
    run_and_escape(&mut h, "/version");
    h.wait_for_text("clankers", TIMEOUT);

    h.send_key(Key::PageUp);
    h.settle(SETTLE);
    h.send_key(Key::PageDown);
    h.settle(SETTLE);

    assert!(h.screen_contains("Messages"));
    h.quit();
}

// ── Resize resilience ───────────────────────────────────────

#[test]
fn app_handles_content_after_commands() {
    // Verify the app doesn't crash after a series of different commands
    let mut h = TuiTestHarness::spawn(30, 100);

    run_and_escape(&mut h, "/version");
    h.wait_for_text("clankers", TIMEOUT);

    run_and_escape(&mut h, "/status");
    h.wait_for_text("Model:", TIMEOUT);

    run_and_escape(&mut h, "/usage");
    h.wait_for_text("Token usage:", TIMEOUT);

    run_and_escape(&mut h, "/clear");
    h.wait_for_text("cleared", TIMEOUT);

    // After clear, run another command to verify the app is still responsive
    run_and_escape(&mut h, "/version");
    h.wait_for_text("clankers", TIMEOUT);

    h.quit();
}
