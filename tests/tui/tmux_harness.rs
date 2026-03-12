//! tmux-based TUI test harness
//!
//! Spawns clankers inside a real tmux session, sends keystrokes via
//! `tmux send-keys`, and captures the rendered pane contents (both
//! plain text and ANSI-styled) for snapshot testing.
//!
//! Compared to the PTY harness (`harness.rs`), this approach:
//! - Uses the real tmux terminal emulator (tests the full rendering pipeline)
//! - Captures ANSI escape sequences (colors, bold, underline)
//! - Can save `.ansi` capture files viewable with `cat` in any terminal
//!
//! Tests using this harness are skipped when tmux is not installed.

use std::process::Command;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;

static SESSION_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Check whether tmux is available on PATH.
pub fn tmux_available() -> bool {
    Command::new("tmux").arg("-V").output().map(|o| o.status.success()).unwrap_or(false)
}

/// Skip the calling test if tmux is not installed.
macro_rules! require_tmux {
    () => {
        if !super::tmux_harness::tmux_available() {
            eprintln!("SKIP: tmux not available");
            return;
        }
    };
}
pub(crate) use require_tmux;

/// A running clankers instance inside a tmux session.
pub struct TmuxTestHarness {
    session: String,
    rows: u16,
    cols: u16,
}

impl TmuxTestHarness {
    /// Spawn clankers in a fresh tmux session with the given dimensions.
    ///
    /// Returns `None` if tmux is not available.
    /// Waits for the TUI to render before returning.
    pub fn spawn(rows: u16, cols: u16) -> Option<Self> {
        if !tmux_available() {
            return None;
        }

        let id = SESSION_COUNTER.fetch_add(1, Ordering::SeqCst);
        let session = format!("clankers-test-{}-{}", std::process::id(), id);

        let binary = env!("CARGO_BIN_EXE_clankers");
        let cwd = env!("CARGO_MANIFEST_DIR");

        // Create a detached tmux session running clankers
        let status = Command::new("tmux")
            .args([
                "new-session",
                "-d",
                "-s",
                &session,
                "-x",
                &cols.to_string(),
                "-y",
                &rows.to_string(),
                "-e",
                "TERM=xterm-256color",
                "-e",
                "RUST_LOG=off",
                binary,
                "--no-zellij",
            ])
            .current_dir(cwd)
            .status()
            .expect("failed to run tmux");

        assert!(status.success(), "tmux new-session failed: {status}");

        let harness = Self { session, rows, cols };

        // Wait for the TUI to start rendering
        harness.wait_for_text("NORMAL", Duration::from_secs(10));
        Some(harness)
    }

    // ── Key input ────────────────────────────────────────────

    /// Send a tmux key name (e.g. "Enter", "Escape", "C-c", "Up").
    pub fn send_key(&self, key: TmuxKey) {
        let status = Command::new("tmux")
            .args(["send-keys", "-t", &self.session, key.as_tmux_str()])
            .status()
            .expect("tmux send-keys failed");
        assert!(status.success());
    }

    /// Type a literal string (tmux won't interpret key names).
    pub fn type_str(&self, s: &str) {
        let status = Command::new("tmux")
            .args(["send-keys", "-t", &self.session, "-l", s])
            .status()
            .expect("tmux send-keys literal failed");
        assert!(status.success());
    }

    /// Send raw bytes as hex key codes.
    pub fn send_hex(&self, hex: &str) {
        let status = Command::new("tmux")
            .args(["send-keys", "-t", &self.session, "-H", hex])
            .status()
            .expect("tmux send-keys hex failed");
        assert!(status.success());
    }

    // ── Capture ──────────────────────────────────────────────

    /// Capture the pane contents as plain text (no ANSI escapes).
    pub fn capture_text(&self) -> String {
        let output = Command::new("tmux")
            .args([
                "capture-pane",
                "-t",
                &self.session,
                "-p", // print to stdout
            ])
            .output()
            .expect("tmux capture-pane failed");
        assert!(output.status.success());
        String::from_utf8_lossy(&output.stdout).to_string()
    }

    /// Capture the pane contents with ANSI escape sequences preserved.
    pub fn capture_ansi(&self) -> String {
        let output = Command::new("tmux")
            .args([
                "capture-pane",
                "-t",
                &self.session,
                "-e", // include escapes
                "-p", // print to stdout
            ])
            .output()
            .expect("tmux capture-pane -e failed");
        assert!(output.status.success());
        String::from_utf8_lossy(&output.stdout).to_string()
    }

    /// Capture with full scroll-back history (not just visible area).
    pub fn capture_full(&self) -> String {
        let output = Command::new("tmux")
            .args([
                "capture-pane",
                "-t",
                &self.session,
                "-p",
                "-S",
                "-", // start of history
                "-E",
                "-", // end of history
            ])
            .output()
            .expect("tmux capture-pane full failed");
        assert!(output.status.success());
        String::from_utf8_lossy(&output.stdout).to_string()
    }

    // ── Assertions ───────────────────────────────────────────

    /// Wait until `needle` appears in the captured text, or panic after timeout.
    pub fn wait_for_text(&self, needle: &str, timeout: Duration) {
        let start = Instant::now();
        loop {
            let text = self.capture_text();
            if text.contains(needle) {
                return;
            }
            assert!(start.elapsed() < timeout, "Timed out after {timeout:?} waiting for {needle:?}\nScreen:\n{text}");
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Check if the captured text contains the given string.
    pub fn screen_contains(&self, needle: &str) -> bool {
        self.capture_text().contains(needle)
    }

    /// Get a specific row of the captured text (0-indexed).
    pub fn row_text(&self, row: u16) -> String {
        let text = self.capture_text();
        text.lines().nth(row as usize).unwrap_or("").to_string()
    }

    /// Get the status bar text (last non-empty, non-border row).
    pub fn status_bar(&self) -> String {
        let text = self.capture_text();
        for line in text.lines().rev() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed
                .chars()
                .all(|c| matches!(c, '─' | '│' | '┌' | '┐' | '└' | '┘' | '├' | '┤' | '┬' | '┴' | '┼' | ' '))
            {
                continue;
            }
            return line.to_string();
        }
        String::new()
    }

    // ── Convenience ──────────────────────────────────────────

    /// Sleep briefly for the TUI to process input.
    pub fn settle(&self, duration: Duration) {
        std::thread::sleep(duration);
    }

    /// Quit the TUI cleanly.
    pub fn quit(&self) {
        self.send_key(TmuxKey::Escape);
        self.settle(Duration::from_millis(100));
        self.type_str("q");
        self.settle(Duration::from_millis(300));
    }

    /// Terminal dimensions.
    pub fn size(&self) -> (u16, u16) {
        (self.rows, self.cols)
    }

    /// Session name (for debugging).
    pub fn session_name(&self) -> &str {
        &self.session
    }
}

impl Drop for TmuxTestHarness {
    fn drop(&mut self) {
        // Kill the session on cleanup — ignore errors (may already be dead)
        let _ = Command::new("tmux").args(["kill-session", "-t", &self.session]).status();
    }
}

/// Named keys for tmux `send-keys`.
pub enum TmuxKey {
    Enter,
    Escape,
    Tab,
    ShiftTab,
    Backspace,
    Delete,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    CtrlC,
    CtrlD,
    CtrlT,
    CtrlU,
    CtrlW,
    Space,
}

impl TmuxKey {
    pub fn as_tmux_str(&self) -> &'static str {
        match self {
            TmuxKey::Enter => "Enter",
            TmuxKey::Escape => "Escape",
            TmuxKey::Tab => "Tab",
            TmuxKey::ShiftTab => "BTab",
            TmuxKey::Backspace => "BSpace",
            TmuxKey::Delete => "DC",
            TmuxKey::Up => "Up",
            TmuxKey::Down => "Down",
            TmuxKey::Left => "Left",
            TmuxKey::Right => "Right",
            TmuxKey::Home => "Home",
            TmuxKey::End => "End",
            TmuxKey::PageUp => "PPage",
            TmuxKey::PageDown => "NPage",
            TmuxKey::CtrlC => "C-c",
            TmuxKey::CtrlD => "C-d",
            TmuxKey::CtrlT => "C-t",
            TmuxKey::CtrlU => "C-u",
            TmuxKey::CtrlW => "C-w",
            TmuxKey::Space => "Space",
        }
    }
}
