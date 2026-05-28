//! Thinking mode toggle integration tests

use std::time::Duration;

use super::harness::Key;
use super::harness::TuiTestHarness;

const SETTLE: Duration = Duration::from_millis(300);
const TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn toggle_thinking_with_ctrl_t() {
    let mut h = TuiTestHarness::spawn(24, 100);

    // Thinking defaults to max and Ctrl+t cycles from max back to off.
    h.wait_for_status_bar_contains("💭 max", TIMEOUT);

    h.send_key(Key::CtrlT);
    h.wait_for_text("Thinking: off", TIMEOUT);
    h.wait_for_status_bar_absent("💭", TIMEOUT);

    // Then continue through low → medium → high → max.
    h.send_key(Key::CtrlT);
    h.wait_for_text("Thinking: low", TIMEOUT);
    h.wait_for_status_bar_contains("💭 low", TIMEOUT);

    h.send_key(Key::CtrlT);
    h.wait_for_text("Thinking: medium", TIMEOUT);
    h.wait_for_status_bar_contains("💭 medium", TIMEOUT);

    h.send_key(Key::CtrlT);
    h.wait_for_text("Thinking: high", TIMEOUT);

    h.send_key(Key::CtrlT);
    h.wait_for_text("Thinking: max", TIMEOUT);
    h.wait_for_status_bar_contains("💭 max", TIMEOUT);

    h.quit();
}

#[test]
fn toggle_thinking_with_slash_think() {
    let mut h = TuiTestHarness::spawn(24, 100);

    // /think cycles the default max level to off.
    h.wait_for_status_bar_contains("💭 max", TIMEOUT);
    h.type_str("i/think");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_text("Thinking: off", TIMEOUT);
    h.wait_for_status_bar_absent("💭", TIMEOUT);

    // /think cycles off to low.
    h.send_key(Key::Escape);
    h.settle(SETTLE);
    h.type_str("i/think");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_text("Thinking: low", TIMEOUT);
    h.wait_for_status_bar_contains("💭 low", TIMEOUT);

    // /think off disables.
    h.send_key(Key::Escape);
    h.settle(SETTLE);
    h.type_str("i/think off");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_status_bar_absent("💭", TIMEOUT);

    // /think xhigh aliases max.
    h.send_key(Key::Escape);
    h.settle(SETTLE);
    h.type_str("i/think xhigh");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_text("Thinking: max", TIMEOUT);
    h.wait_for_status_bar_contains("💭 max", TIMEOUT);

    h.quit();
}

#[test]
fn toggle_show_thinking_with_shift_t() {
    let mut h = TuiTestHarness::spawn(24, 100);

    // T in normal mode toggles show_thinking
    h.type_str("T");
    h.wait_for_text("Thinking content now hidden", TIMEOUT);

    // T again shows it
    h.type_str("T");
    h.wait_for_text("Thinking content now visible", TIMEOUT);

    h.quit();
}

#[test]
fn thinking_badge_persists_across_commands() {
    let mut h = TuiTestHarness::spawn(24, 100);

    // Thinking is enabled by default.
    h.wait_for_status_bar_contains("💭 max", TIMEOUT);

    // Run a slash command — badge should persist
    h.type_str("i/version");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_text("clankers 0.1.0", TIMEOUT);

    assert!(h.status_bar().contains("💭 max"), "Thinking badge should persist after /version");

    h.quit();
}
