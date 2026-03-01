//! Tests for cancel behaviour (Ctrl+C) in various modes

use std::time::Duration;

use super::harness::Key;
use super::harness::TuiTestHarness;

const SETTLE: Duration = Duration::from_millis(300);

// ── Ctrl+C in insert mode ───────────────────────────────────
// Note: Ctrl+C is mapped to Cancel in insert mode and Quit in normal mode.
// In insert mode it should cancel/clear; the app may or may not exit.
// These tests just verify no crash occurs.

#[test]
fn ctrl_c_in_insert_mode_does_not_crash() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.type_str("isome text");
    h.settle(SETTLE);
    h.send_key(Key::CtrlC);
    h.settle(Duration::from_millis(500));
    // App may have exited or cancelled — either is fine
}

#[test]
fn ctrl_c_with_empty_input_does_not_crash() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.type_str("i");
    h.settle(SETTLE);
    h.send_key(Key::CtrlC);
    h.settle(Duration::from_millis(500));
}
