//! Visual regression tests — snapshot the TUI screen at key states.
//!
//! These tests capture both plain text and styled (color-aware) snapshots
//! of the TUI at specific moments. Regressions in layout, content, or
//! color scheme will cause snapshot mismatches.
//!
//! Run with: `cargo test --test tui_tests tui::visual`
//! Review:   `cargo insta review`
//! Update:   `cargo insta test --accept`
//!
//! Screenshots (PNG files) are saved to `tests/tui/captures/` for
//! human review alongside the text snapshots.

use std::time::Duration;

use super::harness::Key;
use super::harness::TuiTestHarness;
use super::snapshot::assert_structure_snapshot;

const SETTLE: Duration = Duration::from_millis(300);
const TIMEOUT: Duration = Duration::from_secs(3);

// ── Startup state ────────────────────────────────────────────

#[test]
fn snapshot_startup_text() {
    let h = TuiTestHarness::spawn(24, 80);
    h.save_screenshot("startup_24x80");
    assert_structure_snapshot!("startup_24x80_structure", &h);
}

#[test]
fn snapshot_startup_styled() {
    // Styled snapshot captures colors — useful for catching theme regressions.
    // Uses structure extraction to avoid volatile content.
    let h = TuiTestHarness::spawn(24, 80);
    h.save_screenshot("startup_24x80_styled");
    assert_structure_snapshot!("startup_24x80_styled_structure", &h);
}

#[test]
fn snapshot_startup_wide() {
    let h = TuiTestHarness::spawn(30, 160);
    h.save_screenshot("startup_30x160");
    assert_structure_snapshot!("startup_30x160_structure", &h);
}

// ── Insert mode ──────────────────────────────────────────────

#[test]
fn snapshot_insert_mode() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.type_str("i");
    h.wait_for_text("INSERT", TIMEOUT);
    h.settle(SETTLE);

    h.save_screenshot("insert_mode");
    assert_structure_snapshot!("insert_mode_structure", &h);
}

#[test]
fn snapshot_insert_with_text() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.type_str("ihello-world-testing");
    h.wait_for_text("hello-world-testing", TIMEOUT);
    h.settle(SETTLE);

    h.save_screenshot("insert_with_text");
    assert_structure_snapshot!("insert_with_text_structure", &h);
}

// ── Leader menu ──────────────────────────────────────────────

#[test]
fn snapshot_leader_menu() {
    let mut h = TuiTestHarness::spawn(30, 100);
    h.type_str(" ");
    h.wait_for_text("Space", TIMEOUT);
    h.settle(SETTLE);

    h.save_screenshot("leader_menu");
    assert_structure_snapshot!("leader_menu_structure", &h);
}

#[test]
fn snapshot_leader_session_submenu() {
    let mut h = TuiTestHarness::spawn(30, 100);
    h.type_str(" ");
    h.wait_for_text("Space", TIMEOUT);
    h.type_str("s");
    h.settle(SETTLE);

    h.save_screenshot("leader_session_submenu");
    assert_structure_snapshot!("leader_session_submenu_structure", &h);
}

// ── Slash menu ───────────────────────────────────────────────

#[test]
fn snapshot_slash_menu() {
    let mut h = TuiTestHarness::spawn(30, 100);
    h.type_str("i/");
    h.settle(Duration::from_millis(500));

    h.save_screenshot("slash_menu");
    assert_structure_snapshot!("slash_menu_structure", &h);
}

#[test]
fn snapshot_slash_menu_filtered() {
    let mut h = TuiTestHarness::spawn(30, 100);
    h.type_str("i/ver");
    h.settle(Duration::from_millis(500));

    h.save_screenshot("slash_menu_filtered");
    assert_structure_snapshot!("slash_menu_filtered_structure", &h);
}

// ── After slash command output ───────────────────────────────
//
// These tests generate screenshots for visual review but do NOT assert
// on snapshot content, because slash command output contains session-
// dependent data (worktree IDs, git status, timing). The screenshot
// PNGs are the primary artifact here.

#[test]
fn screenshot_after_version() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.type_str("i/version");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_text("clankers 0.1.0", TIMEOUT);
    h.settle(SETTLE);
    h.save_screenshot("after_version");
}

#[test]
fn screenshot_after_status() {
    let mut h = TuiTestHarness::spawn(24, 100);
    h.type_str("i/status");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_text("Model:", TIMEOUT);
    h.settle(SETTLE);
    h.save_screenshot("after_status");
}

#[test]
fn screenshot_after_help() {
    let mut h = TuiTestHarness::spawn(40, 120);
    h.type_str("i/help");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_text("/help", TIMEOUT);
    h.settle(SETTLE);
    h.save_screenshot("after_help");
}

// ── Panel focus ──────────────────────────────────────────────

#[test]
fn snapshot_panel_focused() {
    let mut h = TuiTestHarness::spawn(24, 120);
    h.send_key(Key::Escape);
    h.settle(SETTLE);
    h.type_str("`");
    h.wait_for_text("j/k", TIMEOUT);
    h.settle(SETTLE);

    h.save_screenshot("panel_focused");
    assert_structure_snapshot!("panel_focused_structure", &h);
}

// ── Terminal sizes ───────────────────────────────────────────

#[test]
fn snapshot_small_terminal() {
    let h = TuiTestHarness::spawn(12, 50);
    h.save_screenshot("small_12x50");
    assert_structure_snapshot!("small_12x50_structure", &h);
}

#[test]
fn snapshot_tall_terminal() {
    let h = TuiTestHarness::spawn(60, 80);
    h.save_screenshot("tall_60x80");
    assert_structure_snapshot!("tall_60x80_structure", &h);
}

// ── Styled snapshots (with color info) ───────────────────────
//
// Full-screen styled snapshots are screenshot-only (no assertion)
// because the message area has volatile content. Structure snapshots
// above catch layout/structural regressions; PNG screenshots catch
// visual/color regressions via human review.

#[test]
fn screenshot_startup_colors() {
    let h = TuiTestHarness::spawn(24, 80);
    h.save_screenshot("startup_colors");
}

#[test]
fn screenshot_insert_mode_colors() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.type_str("i");
    h.wait_for_text("INSERT", TIMEOUT);
    h.settle(SETTLE);
    h.save_screenshot("insert_mode_colors");
}

// ── Screenshot-only tests (PNG output, no snapshot assertion) ─

#[test]
fn screenshot_all_states() {
    // Generate a series of screenshots for visual review.
    // No assertions — these are reference images only.
    let mut h = TuiTestHarness::spawn(30, 120);

    // State 1: Normal mode startup
    h.save_screenshot("states_01_normal");

    // State 2: Insert mode
    h.type_str("i");
    h.wait_for_text("INSERT", TIMEOUT);
    h.save_screenshot("states_02_insert");

    // State 3: With typed text
    h.type_str("hello world, testing screenshots");
    h.settle(SETTLE);
    h.save_screenshot("states_03_typing");

    // State 4: After /version command
    h.type_str("\x15"); // Ctrl+U to clear
    h.settle(Duration::from_millis(100));
    h.type_str("/version");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_text("clankers 0.1.0", TIMEOUT);
    h.settle(SETTLE);
    h.save_screenshot("states_04_after_version");

    // State 5: Leader menu
    h.send_key(Key::Escape);
    h.settle(Duration::from_millis(100));
    h.type_str(" ");
    h.wait_for_text("Space", TIMEOUT);
    h.save_screenshot("states_05_leader_menu");

    // State 6: Back to normal
    h.send_key(Key::Escape);
    h.settle(SETTLE);
    h.save_screenshot("states_06_back_to_normal");
}
