//! Leader key (Space) menu integration tests

use std::time::Duration;

use super::harness::Key;
use super::harness::TuiTestHarness;

const SETTLE: Duration = Duration::from_millis(300);
const TIMEOUT: Duration = Duration::from_secs(3);

// ── Open / close ────────────────────────────────────────────

#[test]
fn space_opens_leader_menu() {
    let mut h = TuiTestHarness::spawn(24, 80);

    // Press Space in normal mode
    h.type_str(" ");
    h.wait_for_text("Space", TIMEOUT);

    // Should show root menu items
    assert!(h.screen_contains("session"), "Leader menu should show 'session' item");
    assert!(h.screen_contains("model"), "Leader menu should show 'model' item");
    assert!(h.screen_contains("account"), "Leader menu should show 'account' item");

    h.quit();
}

#[test]
fn escape_closes_leader_menu() {
    let mut h = TuiTestHarness::spawn(24, 80);

    // Open leader menu
    h.type_str(" ");
    h.wait_for_text("Space", TIMEOUT);

    // Escape should close it
    h.send_key(Key::Escape);
    h.settle(SETTLE);

    // Menu should be gone, still in normal mode
    assert!(h.status_bar().contains("NORMAL"), "Should be back in normal mode");

    h.quit();
}

#[test]
fn unknown_key_dismisses_leader_menu() {
    let mut h = TuiTestHarness::spawn(24, 80);

    // Open leader menu
    h.type_str(" ");
    h.wait_for_text("Space", TIMEOUT);

    // Press an unmapped key — should dismiss
    h.type_str("z");
    h.settle(SETTLE);

    // Should be back to normal mode, no menu visible
    assert!(h.status_bar().contains("NORMAL"), "Should be in normal mode after dismiss");

    h.quit();
}

// ── Direct actions ──────────────────────────────────────────

#[test]
fn leader_t_toggles_thinking() {
    let mut h = TuiTestHarness::spawn(24, 100);

    // Space → t should toggle thinking
    h.type_str(" ");
    h.wait_for_text("Space", TIMEOUT);
    h.type_str("t");
    h.wait_for_text("Thinking: low", TIMEOUT);
    assert!(h.status_bar().contains("💭"), "Thinking badge should appear");

    h.quit();
}

#[test]
fn leader_shift_t_toggles_show_thinking() {
    let mut h = TuiTestHarness::spawn(24, 100);

    // Space → T should toggle show/hide thinking
    h.type_str(" ");
    h.wait_for_text("Space", TIMEOUT);
    h.type_str("T");
    h.wait_for_text("Thinking content now hidden", TIMEOUT);

    h.quit();
}

#[test]
fn leader_f_activates_search() {
    let mut h = TuiTestHarness::spawn(24, 100);

    // Space → f should open output search
    h.type_str(" ");
    h.wait_for_text("Space", TIMEOUT);
    h.type_str("f");
    h.settle(SETTLE);

    // Search bar should be active (look for the search prompt)
    assert!(
        h.screen_contains("Search") || h.screen_contains("/") || h.screen_contains("🔍"),
        "Search overlay should be visible after Space f\nScreen:\n{}",
        h.screen_text()
    );

    // Clean up — Escape to close search
    h.send_key(Key::Escape);
    h.settle(SETTLE);

    h.quit();
}

#[test]
fn leader_question_mark_shows_help() {
    let mut h = TuiTestHarness::spawn(30, 100);

    // Space → ? should show help
    h.type_str(" ");
    h.wait_for_text("Space", TIMEOUT);
    h.type_str("?");
    h.wait_for_text("help", TIMEOUT);

    h.quit();
}

// ── Submenu navigation ──────────────────────────────────────

#[test]
fn leader_s_opens_session_submenu() {
    let mut h = TuiTestHarness::spawn(24, 80);

    // Space → s should open session submenu
    h.type_str(" ");
    h.wait_for_text("Space", TIMEOUT);
    h.type_str("s");
    h.settle(SETTLE);

    // Should show session submenu items with breadcrumb
    assert!(h.screen_contains("session"), "Should show session breadcrumb");
    assert!(h.screen_contains("new"), "Session submenu should show 'new'");
    assert!(h.screen_contains("compact"), "Session submenu should show 'compact'");

    h.quit();
}

#[test]
fn submenu_escape_goes_back_to_root() {
    let mut h = TuiTestHarness::spawn(24, 80);

    // Space → s → session submenu
    h.type_str(" ");
    h.wait_for_text("Space", TIMEOUT);
    h.type_str("s");
    h.settle(SETTLE);
    assert!(h.screen_contains("new"), "Should be in session submenu");

    // Escape → back to root (not closed)
    h.send_key(Key::Escape);
    h.settle(SETTLE);

    // Should be back at root menu showing model/account/etc
    assert!(h.screen_contains("model"), "Should be back at root menu");
    assert!(h.screen_contains("account"), "Should be back at root menu");

    // Escape again → closes entirely
    h.send_key(Key::Escape);
    h.settle(SETTLE);
    assert!(h.status_bar().contains("NORMAL"), "Should be in normal mode");

    h.quit();
}

#[test]
fn submenu_action_executes_and_closes() {
    let mut h = TuiTestHarness::spawn(24, 100);

    // Space → s → c should execute /compact
    h.type_str(" ");
    h.wait_for_text("Space", TIMEOUT);
    h.type_str("s");
    h.settle(SETTLE);
    assert!(h.screen_contains("compact"), "Session submenu should show 'compact'");
    h.type_str("c");
    h.settle(SETTLE);

    // Menu should be dismissed, compact should have executed
    h.wait_for_text("Compact", TIMEOUT);

    h.quit();
}

// ── Space does not fire in insert mode ──────────────────────

#[test]
fn space_in_insert_mode_types_space() {
    let mut h = TuiTestHarness::spawn(24, 80);

    // Enter insert mode and type a space
    h.type_str("i");
    h.wait_for_text("INSERT", TIMEOUT);
    h.type_str("hello world");
    h.settle(SETTLE);

    // The space should be part of the input text, not open the leader menu
    assert!(!h.screen_contains("session"), "Leader menu should NOT open in insert mode");

    h.quit();
}

// ── Repeated open/close ─────────────────────────────────────

#[test]
fn repeated_leader_menu_open_close() {
    let mut h = TuiTestHarness::spawn(24, 80);

    for _ in 0..5 {
        h.type_str(" ");
        h.wait_for_text("Space", TIMEOUT);
        h.send_key(Key::Escape);
        h.settle(Duration::from_millis(150));
    }

    // App should still be responsive
    assert!(h.status_bar().contains("NORMAL"), "Should still be in normal mode");

    h.quit();
}
