//! Thinking mode toggle integration tests

use std::time::Duration;

use super::harness::Key;
use super::harness::TuiTestHarness;

const SETTLE: Duration = Duration::from_millis(300);
const TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn toggle_thinking_with_ctrl_t() {
    let mut h = TuiTestHarness::spawn(24, 100);

    // Initially no thinking badge in status bar
    assert!(!h.status_bar().contains("💭"), "Thinking should be off initially");

    // Ctrl+t cycles to "low"
    h.send_key(Key::CtrlT);
    h.wait_for_text("💭", TIMEOUT);
    assert!(h.status_bar().contains("💭"), "Status bar should show 💭 after Ctrl+t");
    assert!(h.screen_contains("Thinking: low"), "Should show low level:\n{}", h.screen_text());

    // Ctrl+t cycles to "medium"
    h.send_key(Key::CtrlT);
    h.wait_for_text("Thinking: medium", TIMEOUT);
    assert!(h.status_bar().contains("💭 medium"), "Status bar should show medium level");

    // Ctrl+t cycles to "high"
    h.send_key(Key::CtrlT);
    h.wait_for_text("Thinking: high", TIMEOUT);

    // Ctrl+t cycles to "max"
    h.send_key(Key::CtrlT);
    h.wait_for_text("Thinking: max", TIMEOUT);

    // Ctrl+t cycles back to "off"
    h.send_key(Key::CtrlT);
    h.wait_for_text("Thinking: off", TIMEOUT);
    assert!(!h.status_bar().contains("💭"), "Status bar should not show 💭 when off");

    h.quit();
}

#[test]
fn toggle_thinking_with_slash_think() {
    let mut h = TuiTestHarness::spawn(24, 100);

    // /think cycles to "low"
    h.type_str("i/think");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_text("💭", TIMEOUT);
    assert!(h.status_bar().contains("💭"));
    assert!(h.screen_contains("Thinking: low"));

    // /think off disables
    h.send_key(Key::Escape);
    h.settle(SETTLE);
    h.type_str("i/think off");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_text("Thinking: off", TIMEOUT);
    assert!(!h.status_bar().contains("💭"));

    // /think high sets directly
    h.send_key(Key::Escape);
    h.settle(SETTLE);
    h.type_str("i/think high");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_text("Thinking: high", TIMEOUT);
    assert!(h.status_bar().contains("💭 high"));

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

    // Enable thinking
    h.send_key(Key::CtrlT);
    h.wait_for_text("💭", TIMEOUT);

    // Run a slash command — badge should persist
    h.type_str("i/version");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_text("clankers 0.1.0", TIMEOUT);

    assert!(h.status_bar().contains("💭"), "Thinking badge should persist after /version");

    h.quit();
}
