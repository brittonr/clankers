//! Tests for all quit/exit paths

use std::time::Duration;

use super::harness::Key;
use super::harness::TuiTestHarness;

const TIMEOUT: Duration = Duration::from_secs(5);
const SETTLE: Duration = Duration::from_millis(200);
const EXIT_TIMEOUT: Duration = Duration::from_secs(10);
const ALT_SCREEN_EXIT_SEQUENCE: &str = "\u{1b}[?1049l";
const PREVIEW_PROMPT: &str = "(markdown preview)";
const PREVIEW_MARKDOWN: &str = "scrollback **bold** evidence";
const PREVIEW_RENDERED_TEXT: &str = "scrollback bold evidence";

fn run_slash(h: &mut TuiTestHarness, command: &str) {
    h.type_str(&format!("i{command}"));
    h.settle(SETTLE);
    h.send_key(Key::Enter);
}

fn prepare_scrollback_block(h: &mut TuiTestHarness) {
    h.wait_for_text("NORMAL", TIMEOUT);
    run_slash(h, &format!("/preview {PREVIEW_MARKDOWN}"));
    h.wait_for_text("scrollback", TIMEOUT);
    h.wait_for_text("bold", TIMEOUT);
    h.send_key(Key::Escape);
    h.wait_for_text("NORMAL", TIMEOUT);
}

fn scrollback_tail_after_exit(h: &mut TuiTestHarness) -> String {
    h.wait_for_rendered_text_after_last(ALT_SCREEN_EXIT_SEQUENCE, PREVIEW_PROMPT, EXIT_TIMEOUT);
    h.wait_for_rendered_text_after_last(ALT_SCREEN_EXIT_SEQUENCE, PREVIEW_RENDERED_TEXT, EXIT_TIMEOUT);
    h.rendered_text_after_last(ALT_SCREEN_EXIT_SEQUENCE)
        .expect("alternate-screen exit sequence should be present in raw PTY output")
}

fn assert_scrollback_dump_contains_preview(tail: &str) {
    assert!(tail.contains(PREVIEW_PROMPT), "missing prompt in scrollback tail: {tail:?}");
    assert!(
        tail.contains(PREVIEW_RENDERED_TEXT),
        "missing rendered assistant markdown in scrollback tail: {tail:?}"
    );
    assert!(
        !tail.contains(PREVIEW_MARKDOWN),
        "scrollback tail should render markdown instead of raw markers: {tail:?}"
    );
}

// ── q in normal mode ────────────────────────────────────────

#[test]
fn quit_with_q() {
    let mut h = TuiTestHarness::spawn(24, 80);
    prepare_scrollback_block(&mut h);

    h.type_str("q");

    let tail = scrollback_tail_after_exit(&mut h);
    assert_scrollback_dump_contains_preview(&tail);
}

// ── Ctrl+D ──────────────────────────────────────────────────

#[test]
fn quit_with_ctrl_d() {
    let mut h = TuiTestHarness::spawn(24, 80);
    prepare_scrollback_block(&mut h);

    h.type_str("i");
    h.wait_for_text("INSERT", TIMEOUT);
    h.send_key(Key::CtrlD);

    let tail = scrollback_tail_after_exit(&mut h);
    assert_scrollback_dump_contains_preview(&tail);
}

// ── /quit slash command ─────────────────────────────────────

#[test]
fn quit_with_slash_quit() {
    let mut h = TuiTestHarness::spawn(24, 80);
    prepare_scrollback_block(&mut h);

    run_slash(&mut h, "/quit");

    let tail = scrollback_tail_after_exit(&mut h);
    assert_scrollback_dump_contains_preview(&tail);
}

// ── Ctrl+C in normal mode ───────────────────────────────────

#[test]
fn quit_with_ctrl_c_normal() {
    let mut h = TuiTestHarness::spawn(24, 80);
    prepare_scrollback_block(&mut h);

    h.send_key(Key::CtrlC);

    let tail = scrollback_tail_after_exit(&mut h);
    assert_scrollback_dump_contains_preview(&tail);
}
