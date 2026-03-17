//! Tests for all quit/exit paths

use std::time::Duration;

use super::harness::Key;
use super::harness::TuiTestHarness;

const TIMEOUT: Duration = Duration::from_secs(5);

// ── q in normal mode ────────────────────────────────────────

#[test]
fn quit_with_q() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.wait_for_text("NORMAL", TIMEOUT);
    h.type_str("q");
    h.settle(Duration::from_millis(500));
    // Process should have exited
}

// ── Ctrl+D ──────────────────────────────────────────────────

#[test]
fn quit_with_ctrl_d() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.wait_for_text("NORMAL", TIMEOUT);
    h.send_key(Key::CtrlD);
    h.settle(Duration::from_millis(500));
    // Process should have exited
}

// ── /quit slash command ─────────────────────────────────────

#[test]
fn quit_with_slash_quit() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.type_str("i/quit");
    h.settle(Duration::from_millis(200));
    h.send_key(Key::Enter);
    h.settle(Duration::from_millis(500));
    // Process should have exited
}

// ── Ctrl+C in normal mode ───────────────────────────────────

#[test]
fn quit_with_ctrl_c_normal() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.wait_for_text("NORMAL", TIMEOUT);
    h.send_key(Key::CtrlC);
    h.settle(Duration::from_millis(500));
    // Process should have exited or cancelled
}
