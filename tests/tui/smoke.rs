//! Smoke tests for the clankers TUI

use std::time::Duration;

use super::harness::Key;
use super::harness::TuiTestHarness;

const SETTLE: Duration = Duration::from_millis(300);

#[test]
fn tui_launches_and_shows_normal_mode() {
    let mut h = TuiTestHarness::spawn(24, 80);
    assert!(h.status_bar().contains("NORMAL"));
    assert!(h.screen_contains("Messages"));
    h.quit();
}

#[test]
fn tui_switch_to_insert_mode() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.type_str("i");
    h.wait_for_text("INSERT", Duration::from_secs(2));
    assert!(h.status_bar().contains("INSERT"));
    h.quit();
}

#[test]
fn tui_escape_returns_to_normal() {
    let mut h = TuiTestHarness::spawn(24, 80);
    h.type_str("i");
    h.wait_for_text("INSERT", Duration::from_secs(2));

    h.send_key(Key::Escape);
    h.wait_for_text("NORMAL", Duration::from_secs(2));
    assert!(h.status_bar().contains("NORMAL"));
    h.quit();
}

#[test]
fn tui_slash_version_shows_version() {
    let mut h = TuiTestHarness::spawn(24, 80);

    h.type_str("i/version");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_text("clankers 0.1.0", Duration::from_secs(2));
    h.quit();
}

#[test]
fn tui_slash_help_shows_commands() {
    let mut h = TuiTestHarness::spawn(24, 80);

    h.type_str("i/help");
    h.settle(SETTLE);
    h.send_key(Key::Enter);
    h.wait_for_text("/help", Duration::from_secs(2));
    h.quit();
}

#[test]
fn tui_typing_shows_in_input() {
    let mut h = TuiTestHarness::spawn(24, 80);

    h.type_str("ihello");
    h.wait_for_text("hello", Duration::from_secs(2));
    // Type more to confirm input is live
    h.type_str("123");
    h.wait_for_text("hello123", Duration::from_secs(2));
    h.quit();
}

#[test]
fn tui_quit_with_q_in_normal_mode() {
    let mut h = TuiTestHarness::spawn(24, 80);
    assert!(h.status_bar().contains("NORMAL"));
    h.type_str("q");
    // Process should exit — give it a moment
    h.settle(Duration::from_millis(500));
}
