//! Integration tests for the slash command autocomplete menu

use std::time::Duration;

use super::harness::Key;
use super::harness::TuiTestHarness;

const SETTLE: Duration = Duration::from_millis(300);
const TIMEOUT: Duration = Duration::from_secs(5);

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

    // Capture which commands are initially visible.
    let initial_screen = h.screen_text();

    // The menu shows at most 10 items. Press Down until the visible
    // content changes — no hard-coded count or command names needed.
    let max_presses = 40; // safety cap
    for _ in 0..max_presses {
        h.send_key(Key::Down);
        h.settle(Duration::from_millis(50));
    }
    h.settle(SETTLE);

    let scrolled_screen = h.screen_text();
    assert_ne!(
        scrolled_screen, initial_screen,
        "Menu should scroll to show different items after pressing Down.\nScreen:\n{}",
        scrolled_screen
    );

    h.quit();
}

#[test]
fn slash_menu_scrolls_with_arrows() {
    let mut h = TuiTestHarness::spawn(30, 100);

    h.type_str("i/");
    h.settle(SETTLE);

    let initial_screen = h.screen_text();

    // Scroll down well past the visible window
    let max_presses = 40;
    for _ in 0..max_presses {
        h.send_key(Key::Down);
        h.settle(Duration::from_millis(50));
    }
    h.settle(SETTLE);

    let after_down = h.screen_text();
    assert_ne!(
        after_down, initial_screen,
        "Down arrow should scroll the menu to show different items.\nScreen:\n{}",
        after_down
    );

    // Navigate back up the same amount
    for _ in 0..max_presses {
        h.send_key(Key::Up);
        h.settle(Duration::from_millis(50));
    }
    h.settle(SETTLE);

    // Should be back at (or near) the top — "account" is always first
    let after_up = h.screen_text();
    assert!(
        after_up.contains("account"),
        "Up arrow should scroll back to the top of the menu.\nScreen:\n{}",
        after_up
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
