//! Integration tests for status bar content

use std::time::Duration;

use super::harness::Key;
use super::harness::TuiTestHarness;

const SETTLE: Duration = Duration::from_millis(300);
const TIMEOUT: Duration = Duration::from_secs(3);

// ── Status bar shows mode ───────────────────────────────────

#[test]
fn status_bar_shows_normal_mode() {
    let mut h = TuiTestHarness::spawn(24, 100);
    assert!(h.status_bar().contains("NORMAL"));
    h.quit();
}

#[test]
fn status_bar_shows_insert_mode() {
    let mut h = TuiTestHarness::spawn(24, 100);
    h.type_str("i");
    h.wait_for_text("INSERT", TIMEOUT);
    assert!(h.status_bar().contains("INSERT"));
    h.quit();
}

// ── Status bar shows model ──────────────────────────────────

#[test]
fn status_bar_shows_model_name() {
    let mut h = TuiTestHarness::spawn(24, 200);
    let bar = h.status_bar();
    // The status bar should show the model name (e.g. "claude-...")
    assert!(
        bar.contains("claude") || bar.contains("gpt") || bar.contains("model"),
        "Status bar should show model name.\nBar: {}",
        bar
    );
    h.quit();
}

// ── Status bar shows idle state ─────────────────────────────

#[test]
fn status_bar_shows_idle() {
    let mut h = TuiTestHarness::spawn(24, 200);
    assert!(h.status_bar().contains("idle"), "Status bar should show 'idle'.\nBar: {}", h.status_bar());
    h.quit();
}

// ── Status bar shows CWD ────────────────────────────────────

#[test]
fn status_bar_shows_cwd() {
    let mut h = TuiTestHarness::spawn(24, 120);
    let bar = h.status_bar();
    // Should contain part of the current working directory
    assert!(bar.contains("/") || bar.contains("~"), "Status bar should show CWD.\nBar: {}", bar);
    h.quit();
}

// ── Status bar updates after model change ───────────────────

#[test]
fn status_bar_updates_after_model_switch() {
    let mut h = TuiTestHarness::spawn(24, 200);

    h.type_str("i/model test-model-xyz");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_text("test-model-xyz", TIMEOUT);
    h.send_key(Key::Escape);
    h.settle(SETTLE);

    assert!(
        h.status_bar().contains("test-model-xyz"),
        "Status bar should show new model.\nBar: {}",
        h.status_bar()
    );
    h.quit();
}

// ── Thinking badge in status bar ────────────────────────────

#[test]
fn status_bar_thinking_badge_toggle() {
    let mut h = TuiTestHarness::spawn(24, 120);

    assert!(!h.status_bar().contains("💭"), "No thinking badge initially");

    // Ctrl+t cycles to low
    h.send_key(Key::CtrlT);
    h.wait_for_text("💭", TIMEOUT);
    assert!(h.status_bar().contains("💭"), "Badge should appear");
    assert!(h.status_bar().contains("💭 low"), "Badge should show 'low' level");

    // Cycle through all levels back to off
    h.send_key(Key::CtrlT); // medium
    h.wait_for_text("Thinking: medium", TIMEOUT);
    h.send_key(Key::CtrlT); // high
    h.wait_for_text("Thinking: high", TIMEOUT);
    h.send_key(Key::CtrlT); // max
    h.wait_for_text("Thinking: max", TIMEOUT);
    h.send_key(Key::CtrlT); // off
    h.wait_for_text("Thinking: off", TIMEOUT);
    assert!(!h.status_bar().contains("💭"), "Badge should disappear");

    h.quit();
}
