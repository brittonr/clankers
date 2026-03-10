//! Smoke tests using the tmux-based test harness.
//!
//! These tests run clankers inside a real tmux session and capture both
//! plain text and ANSI-styled output. Skipped when tmux is not available.
//!
//! Run with: `cargo test --test tui_tests tui::tmux_smoke`

use std::time::Duration;

use super::snapshot;
use super::snapshot::{assert_tmux_normalized_snapshot, assert_tmux_snapshot, assert_tmux_styled_snapshot};
use super::tmux_harness::{require_tmux, TmuxKey, TmuxTestHarness};

const SETTLE: Duration = Duration::from_millis(300);
const TIMEOUT: Duration = Duration::from_secs(5);

// ── Basic launch ─────────────────────────────────────────────

#[test]
fn tmux_launches_and_shows_normal_mode() {
    require_tmux!();
    let h = TmuxTestHarness::spawn(24, 80).unwrap();
    assert!(h.screen_contains("NORMAL"));
    assert!(h.screen_contains("Messages"));
    h.quit();
}

#[test]
fn tmux_captures_ansi_output() {
    require_tmux!();
    let h = TmuxTestHarness::spawn(24, 80).unwrap();
    let ansi = h.capture_ansi();

    // ANSI output should contain escape sequences (color codes)
    assert!(
        ansi.contains('\x1b') || ansi.contains("\u{1b}"),
        "ANSI capture should contain escape sequences"
    );

    // Save for manual inspection
    snapshot::save_ansi_capture("tmux_startup", &ansi);
    h.quit();
}

// ── Mode switching ───────────────────────────────────────────

#[test]
fn tmux_insert_mode() {
    require_tmux!();
    let h = TmuxTestHarness::spawn(24, 80).unwrap();
    h.type_str("i");
    h.wait_for_text("INSERT", TIMEOUT);
    assert!(h.status_bar().contains("INSERT"));
    h.quit();
}

#[test]
fn tmux_escape_returns_to_normal() {
    require_tmux!();
    let h = TmuxTestHarness::spawn(24, 80).unwrap();
    h.type_str("i");
    h.wait_for_text("INSERT", TIMEOUT);
    h.send_key(TmuxKey::Escape);
    h.wait_for_text("NORMAL", TIMEOUT);
    assert!(h.status_bar().contains("NORMAL"));
    h.quit();
}

// ── Slash commands ───────────────────────────────────────────

#[test]
fn tmux_slash_version() {
    require_tmux!();
    let h = TmuxTestHarness::spawn(24, 80).unwrap();
    h.type_str("i/version");
    h.settle(SETTLE);
    h.send_key(TmuxKey::Enter);
    h.wait_for_text("clankers 0.1.0", TIMEOUT);
    h.quit();
}

// ── Snapshot tests (tmux + insta) ────────────────────────────

#[test]
fn tmux_snapshot_startup() {
    require_tmux!();
    let h = TmuxTestHarness::spawn(24, 80).unwrap();

    // Save ANSI capture file
    let ansi = h.capture_ansi();
    snapshot::save_ansi_capture("tmux_snapshot_startup", &ansi);

    // Text snapshot via insta (normalized for git counters etc.)
    assert_tmux_normalized_snapshot!("tmux_startup_24x80", &h);
    h.quit();
}

#[test]
fn tmux_snapshot_after_version() {
    require_tmux!();
    let h = TmuxTestHarness::spawn(24, 80).unwrap();
    h.type_str("i/version");
    h.settle(SETTLE);
    h.send_key(TmuxKey::Enter);
    h.wait_for_text("clankers 0.1.0", TIMEOUT);
    h.settle(SETTLE);

    snapshot::save_ansi_capture("tmux_after_version", &h.capture_ansi());
    assert_tmux_normalized_snapshot!("tmux_after_version_24x80", &h);
    h.quit();
}

#[test]
fn tmux_snapshot_leader_menu() {
    require_tmux!();
    let h = TmuxTestHarness::spawn(30, 100).unwrap();
    h.type_str(" ");
    h.wait_for_text("Space", TIMEOUT);
    h.settle(SETTLE);

    snapshot::save_ansi_capture("tmux_leader_menu", &h.capture_ansi());
    assert_tmux_normalized_snapshot!("tmux_leader_menu_30x100", &h);
    h.quit();
}

// ── ANSI-to-PNG screenshot from tmux capture ─────────────────

#[test]
fn tmux_screenshot_from_ansi() {
    require_tmux!();
    let h = TmuxTestHarness::spawn(24, 80).unwrap();

    let ansi = h.capture_ansi();
    let (rows, cols) = h.size();
    let capture = snapshot::ScreenCapture::from_ansi(&ansi, rows, cols);
    super::screenshot::capture_and_save_screenshot("tmux_startup_screenshot", &capture);

    h.quit();
}

// ── Styled snapshot from tmux ────────────────────────────────

#[test]
fn tmux_styled_snapshot() {
    require_tmux!();
    let h = TmuxTestHarness::spawn(24, 80).unwrap();

    // Capture styled output via tmux ANSI capture
    assert_tmux_styled_snapshot!("tmux_startup_styled", &h);
    h.quit();
}

// ── Quit ─────────────────────────────────────────────────────

#[test]
fn tmux_quit_with_q() {
    require_tmux!();
    let h = TmuxTestHarness::spawn(24, 80).unwrap();
    h.type_str("q");
    h.settle(Duration::from_millis(500));
    // Session may already be dead — that's fine (Drop handles cleanup)
}
