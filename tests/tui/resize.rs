//! Tests for different terminal sizes and layout resilience

use std::time::Duration;

use super::harness::Key;
use super::harness::TuiTestHarness;

const SETTLE: Duration = Duration::from_millis(300);
const TIMEOUT: Duration = Duration::from_secs(3);

// ── Small terminal ──────────────────────────────────────────

#[test]
fn small_terminal_launches() {
    // Minimum viable size
    let mut h = TuiTestHarness::spawn(10, 40);
    assert!(h.screen_contains("NORMAL") || h.screen_contains("Messages"));
    h.quit();
}

#[test]
fn small_terminal_can_run_commands() {
    let mut h = TuiTestHarness::spawn(12, 50);
    h.type_str("i/version");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_text("clankers", TIMEOUT);
    h.quit();
}

// ── Wide terminal ───────────────────────────────────────────

#[test]
fn wide_terminal_launches() {
    let mut h = TuiTestHarness::spawn(24, 200);
    assert!(h.status_bar().contains("NORMAL"));
    h.quit();
}

#[test]
fn wide_terminal_status_bar_shows_all() {
    let mut h = TuiTestHarness::spawn(24, 200);
    let bar = h.status_bar();
    assert!(bar.contains("NORMAL"));
    assert!(bar.contains("idle"));
    h.quit();
}

// ── Tall terminal ───────────────────────────────────────────

#[test]
fn tall_terminal_launches() {
    let mut h = TuiTestHarness::spawn(60, 80);
    assert!(h.status_bar().contains("NORMAL"));
    h.quit();
}

#[test]
fn tall_terminal_shows_header_and_input() {
    let mut h = TuiTestHarness::spawn(60, 80);
    assert!(h.screen_contains("Messages"));
    // Input area should be visible at the bottom
    h.type_str("i");
    h.wait_for_text("INSERT", TIMEOUT);
    h.quit();
}
