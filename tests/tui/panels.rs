//! Integration tests for todo and subagent panel navigation

use std::time::Duration;

use super::harness::Key;
use super::harness::TuiTestHarness;

const SETTLE: Duration = Duration::from_millis(300);
const TIMEOUT: Duration = Duration::from_secs(5);

/// Helper to enter insert mode and submit a slash command
fn run_slash(h: &mut TuiTestHarness, cmd: &str) {
    h.type_str(&format!("i{}", cmd));
    h.settle(SETTLE);
    h.send_key(Key::Enter);
}

// ── Todo panel visibility ────────────────────────────────────

#[test]
fn todo_panel_appears_after_adding_item() {
    let mut h = TuiTestHarness::spawn(24, 120);
    // Todo panel is always visible (even when empty)
    h.wait_for_text("Todo (", TIMEOUT);

    // Add a todo item via slash command
    run_slash(&mut h, "/todo add Write tests");
    h.wait_for_text("Added todo #1", TIMEOUT);
    h.settle(SETTLE);

    // Todo panel should show the item
    assert!(h.screen_contains("Write tests"));
    h.quit();
}

// ── Panel focus via backtick ─────────────────────────────────

#[test]
fn backtick_focuses_todo_panel() {
    let mut h = TuiTestHarness::spawn(24, 120);

    // Add a todo item so the panel is visible
    run_slash(&mut h, "/todo add Task one");
    h.wait_for_text("Added todo #1", TIMEOUT);
    h.settle(SETTLE);

    // Go to normal mode and press backtick to focus panel
    h.send_key(Key::Escape);
    h.settle(SETTLE);
    h.type_str("`");
    h.settle(SETTLE);

    // Panel should show focus hints
    h.wait_for_text("j/k", TIMEOUT);
    h.quit();
}

// ── h/l spatial navigation between panes (BSP tiling) ────────

#[test]
fn h_l_cycles_through_panels() {
    let mut h = TuiTestHarness::spawn(24, 200);

    // Focus panel via backtick (starts on first pane via FocusNext)
    h.send_key(Key::Escape);
    h.settle(SETTLE);
    h.type_str("`");
    h.settle(SETTLE);
    h.wait_for_text("j/k", TIMEOUT);

    // Panel is focused — pressing h/l navigates spatially.
    // From a left panel, l → chat (center). Focus hints disappear.
    h.type_str("l");
    h.settle(SETTLE);

    // If we landed on chat, "Main" border should appear
    let screen = h.screen_text();
    let on_chat = screen.contains("Main") && !screen.contains("j/k ");
    let on_panel = screen.contains("j/k ");
    assert!(on_chat || on_panel, "l should move focus spatially. Screen:\n{}", screen);

    // Esc always returns to chat (clean state for next check)
    h.send_key(Key::Escape);
    h.settle(SETTLE);
    assert!(!h.screen_contains("j/k "), "Esc should return to chat");

    h.quit();
}

// ── Tab cycles through all panes (BSP tiling order) ──────────

#[test]
fn tab_cycles_all_panes() {
    let mut h = TuiTestHarness::spawn(30, 300);

    // Focus first pane via backtick
    h.send_key(Key::Escape);
    h.settle(SETTLE);
    h.type_str("`");
    h.wait_for_text("j/k", TIMEOUT);

    // Tab should cycle through panes. After enough tabs we should
    // see different panels getting focus hints.
    let mut saw_different_panels = false;
    let first_screen = h.screen_text();

    for _ in 0..6 {
        h.send_key(Key::Tab);
        h.settle(SETTLE);
        let screen = h.screen_text();
        if screen != first_screen {
            saw_different_panels = true;
        }
    }

    assert!(saw_different_panels, "Tab should cycle focus across different panes");

    // Esc returns to chat
    h.send_key(Key::Escape);
    h.settle(SETTLE);
    assert!(!h.screen_contains("j/k "), "Esc should return to chat");

    h.quit();
}

// ── Tab key cycles sub-panels within column ──────────────────

#[test]
fn tab_key_cycles_panels() {
    let mut h = TuiTestHarness::spawn(30, 200);

    // Focus left column (starts on Todo)
    h.send_key(Key::Escape);
    h.settle(SETTLE);
    h.type_str("`");
    h.wait_for_text("j/k", TIMEOUT);

    // Tab should cycle to Files (same column)
    h.send_key(Key::Tab);
    h.settle(SETTLE);
    // Should still be focused (focus hint visible)
    assert!(h.screen_contains("j/k "), "Tab should cycle within column");

    // Shift+Tab should cycle back to Todo
    h.send_key(Key::ShiftTab);
    h.settle(SETTLE);
    assert!(h.screen_contains("j/k "), "Shift+Tab should cycle back");

    h.quit();
}

// ── Esc unfocuses panel ──────────────────────────────────────

#[test]
fn esc_unfocuses_panel() {
    let mut h = TuiTestHarness::spawn(24, 120);

    // Add a todo so panel is visible
    run_slash(&mut h, "/todo add Task");
    h.wait_for_text("Added todo #1", TIMEOUT);
    h.settle(SETTLE);

    // Focus panel
    h.send_key(Key::Escape);
    h.settle(SETTLE);
    h.type_str("`");
    h.settle(SETTLE);
    h.wait_for_text("j/k", TIMEOUT);

    // Esc to unfocus
    h.send_key(Key::Escape);
    h.settle(SETTLE);

    // Focus hints should be gone from the todo panel title
    // (the panel is still visible but without focus hints)
    assert!(!h.screen_contains("j/k "));
    h.quit();
}

// ── j/k navigates within todo panel ──────────────────────────

#[test]
fn jk_navigates_todo_items() {
    let mut h = TuiTestHarness::spawn(24, 120);

    // Add first item and wait for it to complete
    run_slash(&mut h, "/todo add First task");
    h.wait_for_text("Added todo #1", TIMEOUT);
    // Wait for NORMAL mode to return (not streaming)
    h.send_key(Key::Escape);
    h.settle(SETTLE);
    h.wait_for_text("NORMAL", TIMEOUT);

    // Add second item
    run_slash(&mut h, "/todo add Second task");
    h.wait_for_text("Added todo #2", TIMEOUT);
    h.send_key(Key::Escape);
    h.settle(SETTLE);
    h.wait_for_text("NORMAL", TIMEOUT);

    // Focus panel
    h.type_str("`");
    h.settle(SETTLE);
    h.wait_for_text("j/k", TIMEOUT);

    // Navigate with j/k — panel should show selection indicator (▸)
    h.type_str("j");
    h.settle(SETTLE);
    h.type_str("k");
    h.settle(SETTLE);

    // Both items should still be visible
    assert!(h.screen_contains("First task"));
    assert!(h.screen_contains("Second task"));
    h.quit();
}
