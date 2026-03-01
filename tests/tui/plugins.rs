//! TUI integration tests for the plugin system
//!
//! These tests verify that plugin-related slash commands and status
//! information render correctly in the PTY-based TUI.
//!
//! clankers scans the project-root `plugins/` directory at startup, so the
//! test plugin at `plugins/clankers-test-plugin/` is loaded automatically
//! when the binary runs from the repo root.

use std::time::Duration;

use super::harness::Key;
use super::harness::TuiTestHarness;

const SETTLE: Duration = Duration::from_millis(300);
const TIMEOUT: Duration = Duration::from_secs(3);

/// Helper to enter insert mode and submit a slash command
fn run_slash(h: &mut TuiTestHarness, cmd: &str) {
    h.type_str(&format!("i{}", cmd));
    h.settle(SETTLE);
    h.send_key(Key::Enter);
}

// ── /plugin lists loaded plugins ────────────────────────────

#[test]
fn slash_plugin_lists_test_plugin() {
    let mut h = TuiTestHarness::spawn(40, 120);
    run_slash(&mut h, "/plugin");
    h.wait_for_text("clankers-test-plugin", TIMEOUT);
    assert!(h.screen_contains("v0.1.0"), "Should show plugin version.\nScreen:\n{}", h.screen_text());
    h.quit();
}

// ── /plugin shows active state ──────────────────────────────

#[test]
fn slash_plugin_shows_active_marker() {
    let mut h = TuiTestHarness::spawn(40, 120);
    run_slash(&mut h, "/plugin");
    h.wait_for_text("clankers-test-plugin", TIMEOUT);
    // Active plugins are marked with ✓
    assert!(h.screen_contains("✓"), "Active plugin should have ✓ marker.\nScreen:\n{}", h.screen_text());
    h.quit();
}

// ── /plugin shows tool names ────────────────────────────────

#[test]
fn slash_plugin_shows_tools() {
    let mut h = TuiTestHarness::spawn(40, 120);
    run_slash(&mut h, "/plugin");
    h.wait_for_text("test_echo", TIMEOUT);
    assert!(h.screen_contains("test_reverse"), "Should list test_reverse tool.\nScreen:\n{}", h.screen_text());
    h.quit();
}

// ── /plugin <name> shows details ────────────────────────────

#[test]
fn slash_plugin_show_specific_plugin() {
    let mut h = TuiTestHarness::spawn(40, 120);
    run_slash(&mut h, "/plugin clankers-test-plugin");
    h.wait_for_text("Plugin: clankers-test-plugin", TIMEOUT);
    assert!(h.screen_contains("State:"), "Should show State field.\nScreen:\n{}", h.screen_text());
    assert!(h.screen_contains("Permissions:"), "Should show Permissions field.\nScreen:\n{}", h.screen_text());
    h.quit();
}

// ── /plugin shows description ───────────────────────────────

#[test]
fn slash_plugin_detail_shows_description() {
    let mut h = TuiTestHarness::spawn(40, 120);
    run_slash(&mut h, "/plugin clankers-test-plugin");
    h.wait_for_text("Test plugin for exercising", TIMEOUT);
    h.quit();
}

// ── /plugin shows events ────────────────────────────────────

#[test]
fn slash_plugin_detail_shows_events() {
    let mut h = TuiTestHarness::spawn(40, 120);
    run_slash(&mut h, "/plugin clankers-test-plugin");
    h.wait_for_text("Events:", TIMEOUT);
    assert!(h.screen_contains("agent_start"), "Should show subscribed events.\nScreen:\n{}", h.screen_text());
    h.quit();
}

// ── /plugin with unknown name ───────────────────────────────

#[test]
fn slash_plugin_unknown_name_shows_not_found() {
    let mut h = TuiTestHarness::spawn(34, 100);
    run_slash(&mut h, "/plugin nonexistent-plugin-xyz");
    h.wait_for_text("not found", TIMEOUT);
    h.quit();
}

// ── /help includes /plugin ──────────────────────────────────

#[test]
fn slash_help_lists_plugin_command() {
    let mut h = TuiTestHarness::spawn(50, 200);
    run_slash(&mut h, "/help");
    h.wait_for_text("Available slash commands", TIMEOUT);
    assert!(h.screen_contains("/plugin"), "Help should list /plugin command.\nScreen:\n{}", h.screen_text());
    h.quit();
}

// ── slash menu shows /plugin completion ─────────────────────

#[test]
fn slash_menu_shows_plugin_completion() {
    let mut h = TuiTestHarness::spawn(40, 100);
    // Enter insert mode and start typing /pl
    h.type_str("i/pl");
    h.settle(SETTLE);
    // The slash menu should show "plugin" as a completion
    assert!(
        h.screen_contains("plugin"),
        "Slash menu should show 'plugin' completion.\nScreen:\n{}",
        h.screen_text()
    );
    h.send_key(Key::Escape);
    h.settle(SETTLE);
    h.quit();
}

// ── /plugin after /clear still works ────────────────────────

#[test]
fn slash_plugin_after_clear() {
    let mut h = TuiTestHarness::spawn(40, 120);
    run_slash(&mut h, "/clear");
    h.settle(SETTLE);
    h.send_key(Key::Escape);
    h.settle(SETTLE);
    run_slash(&mut h, "/plugin");
    h.wait_for_text("clankers-test-plugin", TIMEOUT);
    h.quit();
}

// ── /plugin then /status still works ────────────────────────

#[test]
fn slash_plugin_then_status() {
    let mut h = TuiTestHarness::spawn(40, 120);
    run_slash(&mut h, "/plugin");
    h.wait_for_text("clankers-test-plugin", TIMEOUT);
    h.send_key(Key::Escape);
    h.settle(SETTLE);
    run_slash(&mut h, "/status");
    h.wait_for_text("Model:", TIMEOUT);
    h.quit();
}

// ── /tools lists built-in tools ─────────────────────────────

#[test]
fn slash_tools_lists_builtin_tools() {
    let mut h = TuiTestHarness::spawn(50, 200);
    run_slash(&mut h, "/tools");
    h.wait_for_text("Available tools:", TIMEOUT);
    assert!(h.screen_contains("read"), "Should list read tool.\nScreen:\n{}", h.screen_text());
    assert!(h.screen_contains("bash"), "Should list bash tool.\nScreen:\n{}", h.screen_text());
    assert!(h.screen_contains("edit"), "Should list edit tool.\nScreen:\n{}", h.screen_text());
    assert!(h.screen_contains("built-in"), "Should show built-in source.\nScreen:\n{}", h.screen_text());
    h.quit();
}

// ── /tools lists plugin tools ───────────────────────────────

#[test]
fn slash_tools_lists_plugin_tools() {
    let mut h = TuiTestHarness::spawn(50, 200);
    run_slash(&mut h, "/tools");
    h.wait_for_text("Available tools:", TIMEOUT);
    assert!(h.screen_contains("test_echo"), "Should list test_echo plugin tool.\nScreen:\n{}", h.screen_text());
    assert!(
        h.screen_contains("test_reverse"),
        "Should list test_reverse plugin tool.\nScreen:\n{}",
        h.screen_text()
    );
    assert!(h.screen_contains("plugin"), "Should show plugin source.\nScreen:\n{}", h.screen_text());
    h.quit();
}

// ── /tools shows total count ────────────────────────────────

#[test]
fn slash_tools_shows_total_count() {
    let mut h = TuiTestHarness::spawn(40, 120);
    run_slash(&mut h, "/tools");
    h.wait_for_text("tool(s) total", TIMEOUT);
    h.quit();
}

// ── /tools in help ──────────────────────────────────────────

#[test]
fn slash_help_lists_tools_command() {
    let mut h = TuiTestHarness::spawn(45, 120);
    run_slash(&mut h, "/help");
    h.wait_for_text("Available slash commands", TIMEOUT);
    assert!(h.screen_contains("/tools"), "Help should list /tools command.\nScreen:\n{}", h.screen_text());
    h.quit();
}

// ── slash menu shows /tools completion ──────────────────────

#[test]
fn slash_menu_shows_tools_completion() {
    let mut h = TuiTestHarness::spawn(40, 120);
    h.type_str("i/too");
    h.settle(SETTLE);
    assert!(
        h.screen_contains("tools"),
        "Slash menu should show 'tools' completion.\nScreen:\n{}",
        h.screen_text()
    );
    h.send_key(Key::Escape);
    h.settle(SETTLE);
    h.quit();
}
