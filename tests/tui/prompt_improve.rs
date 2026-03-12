//! Integration tests for the prompt improve toggle.

use std::time::Duration;

use super::harness::Key;
use super::harness::TuiTestHarness;

const SETTLE: Duration = Duration::from_millis(300);
const TIMEOUT: Duration = Duration::from_secs(3);

/// Helper to enter insert mode and submit a slash command.
fn run_slash(h: &mut TuiTestHarness, cmd: &str) {
    h.type_str(&format!("i{}", cmd));
    h.settle(SETTLE);
    h.send_key(Key::Enter);
}

// ── /improve slash command toggles on and off ───────────────

#[test]
fn slash_improve_toggles_state() {
    let mut h = TuiTestHarness::spawn(24, 120);

    run_slash(&mut h, "/improve");
    h.wait_for_text("Prompt improve: on", TIMEOUT);

    h.send_key(Key::Escape);
    h.settle(SETTLE);

    run_slash(&mut h, "/improve");
    h.wait_for_text("Prompt improve: off", TIMEOUT);

    h.quit();
}

#[test]
fn slash_improve_on_off_explicit() {
    let mut h = TuiTestHarness::spawn(24, 120);

    run_slash(&mut h, "/improve on");
    h.wait_for_text("Prompt improve: on", TIMEOUT);

    h.send_key(Key::Escape);
    h.settle(SETTLE);

    run_slash(&mut h, "/improve off");
    h.wait_for_text("Prompt improve: off", TIMEOUT);

    h.quit();
}

// ── Status bar badge appears when toggle is on ──────────────

#[test]
fn status_bar_shows_improve_badge() {
    let mut h = TuiTestHarness::spawn(24, 120);

    // Badge should not be present initially
    assert!(!h.status_bar().contains("improve"), "No improve badge initially.\nBar: {}", h.status_bar());

    // Enable via slash command
    run_slash(&mut h, "/improve on");
    h.wait_for_text("Prompt improve: on", TIMEOUT);
    h.send_key(Key::Escape);
    h.settle(SETTLE);

    assert!(
        h.status_bar().contains("improve"),
        "Improve badge should appear in status bar.\nBar: {}",
        h.status_bar()
    );

    // Disable and check badge disappears
    run_slash(&mut h, "/improve off");
    h.wait_for_text("Prompt improve: off", TIMEOUT);
    h.send_key(Key::Escape);
    h.settle(SETTLE);

    assert!(!h.status_bar().contains("improve"), "Improve badge should disappear.\nBar: {}", h.status_bar());

    h.quit();
}

// ── Editor title shows improve indicator ────────────────────

#[test]
fn editor_title_shows_improve_indicator() {
    let mut h = TuiTestHarness::spawn(24, 120);

    // Enable improve
    run_slash(&mut h, "/improve on");
    h.wait_for_text("Prompt improve: on", TIMEOUT);
    h.settle(SETTLE);

    // The editor title should contain "improve"
    let screen = h.screen_text();
    assert!(screen.contains("improve"), "Editor title should show improve indicator.\nScreen:\n{}", screen);

    h.quit();
}

// ── Shift+P toggles in normal mode ──────────────────────────

#[test]
fn shift_p_toggles_prompt_improve() {
    let mut h = TuiTestHarness::spawn(24, 120);

    // Should be in normal mode after spawn
    h.type_str("P"); // Shift+P
    h.wait_for_text("Prompt improve: on", TIMEOUT);
    assert!(h.status_bar().contains("improve"));

    h.type_str("P"); // toggle off
    h.wait_for_text("Prompt improve: off", TIMEOUT);
    assert!(!h.status_bar().contains("improve"));

    h.quit();
}

// ── Ctrl+R toggles in insert mode ───────────────────────────

#[test]
fn ctrl_r_toggles_prompt_improve_in_insert_mode() {
    let mut h = TuiTestHarness::spawn(24, 120);

    h.type_str("i"); // enter insert mode
    h.wait_for_text("INSERT", TIMEOUT);

    h.send_key(Key::CtrlR);
    h.wait_for_text("Prompt improve: on", TIMEOUT);

    h.send_key(Key::CtrlR);
    h.wait_for_text("Prompt improve: off", TIMEOUT);

    h.quit();
}
