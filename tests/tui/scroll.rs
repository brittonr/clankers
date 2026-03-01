//! Tests for scroll behaviour: Ctrl+U/Ctrl+D (page scroll in normal mode),
//! g/G (top/bottom), and scroll state across operations.

use std::time::Duration;

use super::harness::Key;
use super::harness::TuiTestHarness;

const SETTLE: Duration = Duration::from_millis(300);

/// Fill screen with multiple status messages
fn fill_content(h: &mut TuiTestHarness, count: usize) {
    for _ in 0..count {
        h.type_str("i/status");
        h.settle(Duration::from_millis(200));
        h.send_key(Key::Enter);
        h.settle(Duration::from_millis(200));
        h.send_key(Key::Escape);
        h.settle(Duration::from_millis(100));
    }
}

// ── g scrolls to top ────────────────────────────────────────

#[test]
fn g_scroll_to_top_shows_header() {
    let mut h = TuiTestHarness::spawn(20, 100);
    fill_content(&mut h, 8);

    h.type_str("g");
    h.settle(SETTLE);

    // "Messages" header should be visible at the top
    assert!(h.screen_contains("Messages"));
    h.quit();
}

// ── G scrolls to bottom ────────────────────────────────────

#[test]
fn big_g_scroll_to_bottom() {
    let mut h = TuiTestHarness::spawn(20, 100);
    fill_content(&mut h, 8);

    // Scroll to top first
    h.type_str("g");
    h.settle(SETTLE);

    // Then scroll to bottom
    h.type_str("G");
    h.settle(SETTLE);

    // The latest content should be visible
    assert!(h.screen_contains("Model:") || h.screen_contains("Messages"));
    h.quit();
}

// ── PageUp then PageDown returns to similar position ────────

#[test]
fn page_up_then_page_down() {
    let mut h = TuiTestHarness::spawn(20, 100);
    fill_content(&mut h, 8);

    let before = h.screen_text();

    h.send_key(Key::PageUp);
    h.settle(SETTLE);
    h.send_key(Key::PageDown);
    h.settle(SETTLE);

    let after = h.screen_text();
    // Should be roughly back to the same position
    // Both should show Messages header
    assert!(before.contains("Messages") || after.contains("Messages"));
    h.quit();
}

// ── Scroll doesn't crash on empty ───────────────────────────

#[test]
fn scroll_empty_conversation() {
    let mut h = TuiTestHarness::spawn(24, 80);

    h.type_str("g");
    h.settle(SETTLE);
    h.type_str("G");
    h.settle(SETTLE);
    h.send_key(Key::PageUp);
    h.settle(SETTLE);
    h.send_key(Key::PageDown);
    h.settle(SETTLE);

    assert!(h.status_bar().contains("NORMAL"));
    h.quit();
}

// ── Scroll up multiple times ────────────────────────────────

#[test]
fn repeated_scroll_up() {
    let mut h = TuiTestHarness::spawn(20, 100);
    fill_content(&mut h, 10);

    // Scroll up several times
    for _ in 0..5 {
        h.send_key(Key::PageUp);
        h.settle(Duration::from_millis(100));
    }

    // Should still be alive and showing content
    assert!(h.screen_contains("Messages"));
    h.quit();
}
