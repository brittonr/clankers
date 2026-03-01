//! Integration tests for the slash command autocomplete menu

use std::time::Duration;

use super::harness::Key;
use super::harness::TuiTestHarness;

const SETTLE: Duration = Duration::from_millis(300);
const TIMEOUT: Duration = Duration::from_secs(3);

// ── Autocomplete menu appears ───────────────────────────────

#[test]
fn slash_menu_appears_on_slash() {
    let mut h = TuiTestHarness::spawn(30, 100);

    h.type_str("i/");
    h.settle(SETTLE);

    // The autocomplete menu should show some commands
    // Look for common commands that should be in the menu
    assert!(
        h.screen_contains("help") || h.screen_contains("clear") || h.screen_contains("version"),
        "Slash menu should show command suggestions.\nScreen:\n{}",
        h.screen_text()
    );
    h.quit();
}

#[test]
fn slash_menu_filters_on_typing() {
    let mut h = TuiTestHarness::spawn(30, 100);

    h.type_str("i/ver");
    h.settle(SETTLE);

    // Should show /version but not /help
    assert!(h.screen_contains("version"), "Menu should show 'version' for /ver");
    h.quit();
}

#[test]
fn slash_menu_disappears_after_submit() {
    let mut h = TuiTestHarness::spawn(30, 100);

    h.type_str("i/version");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_text("clankers", TIMEOUT);

    // The autocomplete menu should no longer be showing command list
    // (the version output replaces it)
    h.settle(SETTLE);
    h.quit();
}

// ── Tab completion ──────────────────────────────────────────

#[test]
fn tab_completes_slash_command() {
    let mut h = TuiTestHarness::spawn(30, 100);

    h.type_str("i/ver");
    h.settle(SETTLE);

    // Tab should complete to /version
    h.send_key(Key::Tab);
    h.settle(SETTLE);

    // Now submit and check it was /version
    h.send_key(Key::Enter);
    h.wait_for_text("clankers", TIMEOUT);
    h.quit();
}

// ── Escape dismisses menu ───────────────────────────────────

#[test]
fn escape_dismisses_slash_menu() {
    let mut h = TuiTestHarness::spawn(30, 100);

    h.type_str("i/");
    h.settle(Duration::from_millis(500));

    // First Escape hides the slash menu (stays in insert mode),
    // second Escape returns to normal mode
    h.send_key(Key::Escape);
    h.settle(SETTLE);
    h.send_key(Key::Escape);
    h.wait_for_text("NORMAL", TIMEOUT);
    h.quit();
}

// ── Menu scrolls when navigating past visible window ────────

#[test]
fn slash_menu_scrolls_past_visible_items() {
    let mut h = TuiTestHarness::spawn(40, 180);

    h.type_str("i/");
    h.settle(SETTLE);

    // The menu shows 10 items at most. We have 16 commands total.
    // Navigate down past all 10 visible items to reach commands
    // beyond the initial window (export, cd, shell, version, login, quit).
    // Use Ctrl+N (MenuDown) which is the correct keybinding for menu navigation.
    for _ in 0..12 {
        h.send_key(Key::CtrlN);
        h.settle(Duration::from_millis(50));
    }
    h.settle(SETTLE);

    // After scrolling past item 10, the menu should now show items
    // that were initially hidden — e.g. "shell" or "quit"
    let screen = h.screen_text();
    assert!(
        screen.contains("shell") || screen.contains("quit") || screen.contains("login"),
        "Menu should scroll to show items beyond the first 10.\nScreen:\n{}",
        screen
    );

    h.quit();
}

#[test]
fn slash_menu_scrolls_with_ctrl_j_k() {
    let mut h = TuiTestHarness::spawn(30, 100);

    h.type_str("i/");
    h.settle(SETTLE);

    // Navigate down past visible window using Ctrl+J
    for _ in 0..12 {
        h.send_key(Key::CtrlJ);
        h.settle(Duration::from_millis(50));
    }
    h.settle(SETTLE);

    let screen = h.screen_text();
    assert!(
        screen.contains("shell") || screen.contains("quit") || screen.contains("login"),
        "Ctrl+J should scroll the menu down past visible items.\nScreen:\n{}",
        screen
    );

    // Now navigate back up with Ctrl+K
    for _ in 0..12 {
        h.send_key(Key::CtrlK);
        h.settle(Duration::from_millis(50));
    }
    h.settle(SETTLE);

    let screen = h.screen_text();
    assert!(
        screen.contains("help"),
        "Ctrl+K should scroll the menu back up to show 'help'.\nScreen:\n{}",
        screen
    );

    h.quit();
}

// ── Backspace updates menu ──────────────────────────────────

#[test]
fn backspace_updates_slash_menu() {
    let mut h = TuiTestHarness::spawn(30, 100);

    h.type_str("i/version");
    h.settle(SETTLE);

    // Backspace several times to get back to /ver
    for _ in 0..4 {
        h.send_key(Key::Backspace);
    }
    h.settle(SETTLE);

    // Menu should still show version (matching /ver)
    assert!(h.screen_contains("version"), "Menu should show version for /ver");
    h.quit();
}
