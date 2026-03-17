//! Integration tests for slash commands executed in the TUI
//!
//! These tests verify that various slash commands produce the expected
//! on-screen output when run in the real PTY-based TUI.

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

// ── /status ──────────────────────────────────────────────────

#[test]
fn slash_status_shows_model_and_cwd() {
    let mut h = TuiTestHarness::spawn(24, 100);
    run_slash(&mut h, "/status");
    h.wait_for_text("Model:", TIMEOUT);
    assert!(h.screen_contains("CWD:"));
    assert!(h.screen_contains("Tokens used:"));
    h.quit();
}

// ── /usage ───────────────────────────────────────────────────

#[test]
fn slash_usage_shows_token_info() {
    let mut h = TuiTestHarness::spawn(24, 100);
    run_slash(&mut h, "/usage");
    h.wait_for_text("Token usage:", TIMEOUT);
    assert!(h.screen_contains("Total tokens:"));
    assert!(h.screen_contains("Estimated cost:"));
    h.quit();
}

// ── /session ─────────────────────────────────────────────────

#[test]
fn slash_session_shows_info() {
    let mut h = TuiTestHarness::spawn(24, 100);
    run_slash(&mut h, "/session");
    // Either shows session ID or "No active session"
    h.wait_for_text("ession", TIMEOUT); // matches "Session" or "session"
    h.quit();
}

// ── /model (no args) ────────────────────────────────────────

#[test]
fn slash_model_no_args_shows_current() {
    let mut h = TuiTestHarness::spawn(24, 100);
    run_slash(&mut h, "/model");
    h.wait_for_text("Current model:", TIMEOUT);
    assert!(h.screen_contains("Usage: /model"));
    h.quit();
}

// ── /cd (no args) ───────────────────────────────────────────

#[test]
fn slash_cd_no_args_shows_cwd() {
    let mut h = TuiTestHarness::spawn(24, 100);
    run_slash(&mut h, "/cd");
    h.wait_for_text("Current directory:", TIMEOUT);
    h.quit();
}

// ── /cd with valid path ─────────────────────────────────────

#[test]
fn slash_cd_changes_directory() {
    let mut h = TuiTestHarness::spawn(24, 120);
    run_slash(&mut h, "/cd /tmp");
    h.wait_for_text("Changed directory to:", TIMEOUT);
    assert!(h.screen_contains("/tmp"));
    h.quit();
}

// ── /cd with invalid path ───────────────────────────────────

#[test]
fn slash_cd_invalid_path_shows_error() {
    let mut h = TuiTestHarness::spawn(24, 120);
    run_slash(&mut h, "/cd /nonexistent_path_xyz_12345");
    h.wait_for_text("Invalid path", TIMEOUT);
    h.quit();
}

// ── /shell ──────────────────────────────────────────────────

#[test]
fn slash_shell_runs_command() {
    let mut h = TuiTestHarness::spawn(24, 100);
    run_slash(&mut h, "/shell echo CLANKERS_SHELL_TEST");
    h.wait_for_text("CLANKERS_SHELL_TEST", TIMEOUT);
    h.quit();
}

#[test]
fn slash_shell_no_args_shows_usage() {
    let mut h = TuiTestHarness::spawn(24, 100);
    run_slash(&mut h, "/shell");
    h.wait_for_text("Usage: /shell", TIMEOUT);
    h.quit();
}

// ── /export ─────────────────────────────────────────────────

#[test]
fn slash_export_creates_file() {
    let mut h = TuiTestHarness::spawn(24, 120);
    // First create some content to export
    run_slash(&mut h, "/version");
    h.wait_for_text("clankers", TIMEOUT);
    h.send_key(Key::Escape);
    h.settle(SETTLE);

    // Export to a known file
    let export_path = "/tmp/clankers-tui-test-export.md";
    run_slash(&mut h, &format!("/export {}", export_path));
    h.wait_for_text("Exported to:", TIMEOUT);

    // Verify the file was created
    assert!(std::path::Path::new(export_path).exists(), "Export file should exist");
    let content = std::fs::read_to_string(export_path).unwrap();
    assert!(content.contains("clankers"), "Export should contain version output");
    std::fs::remove_file(export_path).ok();
    h.quit();
}

// ── /undo ───────────────────────────────────────────────────

#[test]
fn slash_undo_with_nothing_says_nothing() {
    let mut h = TuiTestHarness::spawn(24, 100);
    run_slash(&mut h, "/undo");
    h.wait_for_text("Nothing to undo", TIMEOUT);
    h.quit();
}

// ── /compact ────────────────────────────────────────────────

#[test]
fn slash_compact_shows_not_implemented() {
    let mut h = TuiTestHarness::spawn(24, 100);
    run_slash(&mut h, "/compact");
    h.wait_for_text("not yet implemented", TIMEOUT);
    h.quit();
}

// ── /quit ───────────────────────────────────────────────────

#[test]
fn slash_quit_exits_app() {
    let mut h = TuiTestHarness::spawn(24, 80);
    run_slash(&mut h, "/quit");
    // App should exit — give it a moment
    h.settle(Duration::from_millis(500));
}

// ── /help ───────────────────────────────────────────────────

#[test]
fn slash_help_lists_all_commands() {
    let mut h = TuiTestHarness::spawn(60, 200);
    run_slash(&mut h, "/help");
    // With 37+ commands + welcome message, the header may scroll off in smaller terminals.
    // Wait for a command that's always visible (near the bottom of the list).
    h.wait_for_text("/quit", TIMEOUT);
    // Verify a sampling of commands appear
    assert!(h.screen_contains("/clear"));
    assert!(h.screen_contains("/reset"));
    assert!(h.screen_contains("/model"));
    h.quit();
}

// ── unknown command ─────────────────────────────────────────

#[test]
fn unknown_slash_command_is_sent_as_prompt_or_ignored() {
    let mut h = TuiTestHarness::spawn(24, 100);
    // An unknown command like /xyzzy should not crash
    run_slash(&mut h, "/xyzzy");
    h.settle(Duration::from_millis(500));
    // The app should still be alive and responsive
    h.send_key(Key::Escape);
    h.wait_for_text("NORMAL", TIMEOUT);
    h.quit();
}
