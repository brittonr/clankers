//! Advanced slash command tests covering argument handling, error paths,
//! and edge cases not in the basic slash_commands module.

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

// ── /model with argument ────────────────────────────────────

#[test]
fn slash_model_switches_model() {
    let mut h = TuiTestHarness::spawn(24, 120);

    run_slash(&mut h, "/model my-custom-model");
    h.wait_for_text("Model switched", TIMEOUT);
    assert!(h.screen_contains("my-custom-model"));

    // Verify /status reflects the change
    h.send_key(Key::Escape);
    h.settle(SETTLE);
    run_slash(&mut h, "/status");
    h.wait_for_text("my-custom-model", TIMEOUT);
    h.quit();
}

// ── /cd to a file (not directory) ───────────────────────────

#[test]
fn slash_cd_to_file_shows_error() {
    let mut h = TuiTestHarness::spawn(24, 120);
    run_slash(&mut h, "/cd /etc/hosts");
    h.wait_for_text("Not a directory", TIMEOUT);
    h.quit();
}

// ── /shell with failing command ─────────────────────────────

#[test]
fn slash_shell_failing_command() {
    let mut h = TuiTestHarness::spawn(24, 120);
    run_slash(&mut h, "/shell false");
    h.wait_for_text("exit code", TIMEOUT);
    h.quit();
}

#[test]
fn slash_shell_stderr_output() {
    let mut h = TuiTestHarness::spawn(24, 120);
    run_slash(&mut h, "/shell echo STDERR_TEST >&2");
    h.wait_for_text("STDERR_TEST", TIMEOUT);
    h.quit();
}

// ── /export default filename ────────────────────────────────

#[test]
fn slash_export_default_filename() {
    let mut h = TuiTestHarness::spawn(24, 120);

    // Create content first
    run_slash(&mut h, "/version");
    h.wait_for_text("clankers", TIMEOUT);
    h.send_key(Key::Escape);
    h.settle(SETTLE);

    // Export with no filename → generates timestamped file
    run_slash(&mut h, "/export");
    h.wait_for_text("Exported to:", TIMEOUT);
    assert!(h.screen_contains("clankers-export-"));

    // Clean up the generated file
    let screen = h.screen_text();
    if let Some(start) = screen.find("Exported to: ") {
        let path_start = start + "Exported to: ".len();
        if let Some(path) = screen[path_start..].lines().next() {
            let path = path.trim();
            std::fs::remove_file(path).ok();
        }
    }

    h.quit();
}

// ── /think with custom budget ───────────────────────────────

#[test]
fn slash_think_with_budget() {
    let mut h = TuiTestHarness::spawn(24, 100);

    // /think 20000 maps to nearest level (high = 32k)
    run_slash(&mut h, "/think 20000");
    h.wait_for_text("💭", TIMEOUT);
    assert!(h.screen_contains("Thinking: high"), "20000 should map to high level");

    h.quit();
}

// ── /login without prior auth URL ───────────────────────────

#[test]
fn slash_login_code_without_url_shows_error() {
    let mut h = TuiTestHarness::spawn(24, 120);
    run_slash(&mut h, "/login somecode#somestate");
    h.wait_for_text("No login in progress", TIMEOUT);
    h.quit();
}

// ── /undo after adding content ──────────────────────────────

// Note: /undo removes conversation blocks, not system messages.
// Slash commands create system messages, so /undo won't remove them.
// This test verifies that /undo correctly says "nothing to undo"
// when only system messages exist.
#[test]
fn slash_undo_only_system_messages() {
    let mut h = TuiTestHarness::spawn(24, 100);

    run_slash(&mut h, "/version");
    h.wait_for_text("clankers", TIMEOUT);
    h.send_key(Key::Escape);
    h.settle(SETTLE);

    run_slash(&mut h, "/undo");
    h.wait_for_text("Nothing to undo", TIMEOUT);
    h.quit();
}

// ── /clear then verify clean slate ──────────────────────────

#[test]
fn slash_clear_removes_all_visible_messages() {
    let mut h = TuiTestHarness::spawn(24, 100);

    // Add multiple messages
    run_slash(&mut h, "/version");
    h.wait_for_text("clankers", TIMEOUT);
    h.send_key(Key::Escape);
    h.settle(SETTLE);

    run_slash(&mut h, "/status");
    h.wait_for_text("Model:", TIMEOUT);
    h.send_key(Key::Escape);
    h.settle(SETTLE);

    // Clear
    run_slash(&mut h, "/clear");
    h.wait_for_text("cleared", TIMEOUT);

    // The version string should no longer appear (only "cleared" message)
    // Note: "clankers" might still be in the status bar header, so check for
    // the specific version output
    assert!(!h.screen_contains("Tokens used:"), "Status output should be cleared.\nScreen:\n{}", h.screen_text());
    h.quit();
}

// ── /reset clears tokens ────────────────────────────────────

#[test]
fn slash_reset_clears_token_count() {
    let mut h = TuiTestHarness::spawn(24, 100);

    // Reset
    run_slash(&mut h, "/reset");
    h.wait_for_text("reset", TIMEOUT);
    h.send_key(Key::Escape);
    h.settle(SETTLE);

    // Check usage shows 0
    run_slash(&mut h, "/usage");
    h.wait_for_text("Total tokens: 0", TIMEOUT);
    h.quit();
}

// ── Chaining multiple slash commands rapidly ────────────────

#[test]
fn rapid_slash_commands() {
    let mut h = TuiTestHarness::spawn(30, 100);

    for _ in 0..5 {
        run_slash(&mut h, "/version");
        h.settle(Duration::from_millis(200));
        h.send_key(Key::Escape);
        h.settle(Duration::from_millis(100));
    }

    h.wait_for_text("clankers", TIMEOUT);
    h.quit();
}
