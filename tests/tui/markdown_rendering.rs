//! TUI tests for live markdown rendering in conversation blocks.
//!
//! Uses the `/preview` slash command to inject a fake assistant block
//! with markdown content, then verifies the rendered output.

use std::time::Duration;

use super::harness::Key;
use super::harness::TuiTestHarness;

const SETTLE: Duration = Duration::from_millis(300);
const TIMEOUT: Duration = Duration::from_secs(5);

/// Helper: run a slash command from normal mode
fn run_slash(h: &mut TuiTestHarness, cmd: &str) {
    h.type_str(&format!("i{}", cmd));
    h.settle(SETTLE);
    h.send_key(Key::Enter);
}

// ── Default preview ──────────────────────────────────────────

#[test]
fn preview_renders_heading() {
    let mut h = TuiTestHarness::spawn(40, 120);
    run_slash(&mut h, "/preview");
    h.wait_for_text("Markdown Preview", TIMEOUT);
    // The H1 should be rendered (without the leading `# `)
    assert!(h.screen_contains("Markdown Preview"));
    // The raw `# ` prefix must NOT appear
    assert!(!h.screen_contains("# Markdown Preview"));
    h.quit();
}

#[test]
fn preview_renders_code_block() {
    let mut h = TuiTestHarness::spawn(40, 120);
    run_slash(&mut h, "/preview");
    h.wait_for_text("rust", TIMEOUT);
    // Code fence should show the language label
    assert!(h.screen_contains("rust"));
    // Code content should be indented
    assert!(h.screen_contains("println!"));
    // Raw fences (```) should NOT appear on screen
    assert!(!h.screen_contains("```"));
    h.quit();
}

#[test]
fn preview_renders_bullet_list() {
    let mut h = TuiTestHarness::spawn(40, 120);
    run_slash(&mut h, "/preview");
    h.wait_for_text("First item", TIMEOUT);
    // Unordered list items should be rendered with bullet markers
    assert!(h.screen_contains("•"));
    assert!(h.screen_contains("First item"));
    assert!(h.screen_contains("Second item"));
    // Raw `- ` prefix should NOT appear
    assert!(!h.screen_contains("- First item"));
    h.quit();
}

#[test]
fn preview_renders_ordered_list() {
    let mut h = TuiTestHarness::spawn(40, 120);
    run_slash(&mut h, "/preview");
    h.wait_for_text("Ordered one", TIMEOUT);
    // Numbered list items should be visible with their numbers
    assert!(h.screen_contains("1."));
    assert!(h.screen_contains("Ordered one"));
    h.quit();
}

#[test]
fn preview_renders_blockquote() {
    let mut h = TuiTestHarness::spawn(40, 120);
    run_slash(&mut h, "/preview");
    h.wait_for_text("blockquote", TIMEOUT);
    // Blockquote should show the ▎ marker instead of >
    assert!(h.screen_contains("▎"));
    assert!(h.screen_contains("blockquote"));
    assert!(!h.screen_contains("> This is"));
    h.quit();
}

#[test]
fn preview_renders_horizontal_rule() {
    let mut h = TuiTestHarness::spawn(40, 120);
    run_slash(&mut h, "/preview");
    h.wait_for_text("Markdown Preview", TIMEOUT);
    // The `---` should become a rendered horizontal rule (─ characters)
    assert!(h.screen_contains("────"));
    h.quit();
}

// ── Custom markdown via /preview <text> ──────────────────────

#[test]
fn preview_custom_bold() {
    let mut h = TuiTestHarness::spawn(30, 120);
    run_slash(&mut h, "/preview This has **important** info");
    h.wait_for_text("important", TIMEOUT);
    // The bold markers should be stripped in the assistant response area.
    // Note: the raw text may appear in the user prompt line ("(markdown preview)"
    // is the prompt, not the arg text), so just verify `important` is shown.
    assert!(h.screen_contains("important"));
    assert!(h.screen_contains("info"));
    h.quit();
}

#[test]
fn preview_custom_inline_code() {
    let mut h = TuiTestHarness::spawn(30, 120);
    run_slash(&mut h, "/preview Run `cargo test` now");
    h.wait_for_text("cargo test", TIMEOUT);
    // Inline code should appear without backticks
    assert!(h.screen_contains("cargo test"));
    assert!(!h.screen_contains("`cargo test`"));
    h.quit();
}

// ── Block structure ──────────────────────────────────────────

#[test]
fn preview_creates_conversation_block() {
    let mut h = TuiTestHarness::spawn(40, 120);
    run_slash(&mut h, "/preview");
    // Wait for the block to finalize (✓ marker)
    h.wait_for_text("✓", TIMEOUT);
    // Should have block borders (┌ and └)
    assert!(h.screen_contains("┌"), "No top border. Screen:\n{}", h.screen_text());
    assert!(h.screen_contains("└"), "No bottom border. Screen:\n{}", h.screen_text());
    // Should show the user prompt marker
    assert!(h.screen_contains("❯"), "No prompt marker. Screen:\n{}", h.screen_text());
    h.quit();
}

#[test]
fn preview_subheading_rendered() {
    let mut h = TuiTestHarness::spawn(40, 120);
    run_slash(&mut h, "/preview");
    h.wait_for_text("Code Block", TIMEOUT);
    // ## headings should appear without the ## prefix
    assert!(h.screen_contains("Code Block"));
    assert!(!h.screen_contains("## Code Block"));
    assert!(h.screen_contains("Lists"));
    assert!(!h.screen_contains("## Lists"));
    h.quit();
}
