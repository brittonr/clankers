//! PTY harness for spawning clankers in a virtual terminal.
//!
//! Used by the validate_tui tool and its tests to interact with
//! a live clankers instance via keystrokes and screen assertions.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use std::io::Read;
use std::io::Write;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

use portable_pty::CommandBuilder;
use portable_pty::NativePtySystem;
use portable_pty::PtySize;
use portable_pty::PtySystem;
use vt100::Parser;

// ── Key name mapping ────────────────────────────────────────────────

pub(super) fn key_bytes(name: &str) -> Option<&'static [u8]> {
    Some(match name.to_lowercase().as_str() {
        "enter" | "return" | "cr" => b"\r",
        "esc" | "escape" => b"\x1b",
        "tab" => b"\t",
        "backspace" | "bs" => b"\x7f",
        "delete" | "del" => b"\x1b[3~",
        "up" => b"\x1b[A",
        "down" => b"\x1b[B",
        "right" => b"\x1b[C",
        "left" => b"\x1b[D",
        "home" => b"\x1b[H",
        "end" => b"\x1b[F",
        "pageup" | "pgup" => b"\x1b[5~",
        "pagedown" | "pgdn" => b"\x1b[6~",
        "ctrl+c" => b"\x03",
        "ctrl+d" => b"\x04",
        "ctrl+j" => b"\x0a",
        "ctrl+k" => b"\x0b",
        "ctrl+n" => b"\x0e",
        "ctrl+p" => b"\x10",
        "ctrl+t" => b"\x14",
        "ctrl+u" => b"\x15",
        "ctrl+w" => b"\x17",
        "ctrl+x" => b"\x18",
        "alt+enter" => b"\x1b\r",
        "space" => b" ",
        "backtick" | "`" => b"`",
        _ => return None,
    })
}

// ── PTY harness ─────────────────────────────────────────────────────

pub(super) struct PtyHarness {
    parser: Arc<Mutex<Parser>>,
    writer: Box<dyn Write + Send>,
    _child: Box<dyn portable_pty::Child + Send + Sync>,
    _reader_thread: std::thread::JoinHandle<()>,
    rows: u16,
    cols: u16,
}

impl PtyHarness {
    #[cfg_attr(
        dylint_lib = "tigerstyle",
        allow(unbounded_loop, reason = "event loop; bounded by PTY close")
    )]
    pub(super) fn spawn(rows: u16, cols: u16, extra_args: &[String]) -> Result<Self, String> {
        let pty_system = NativePtySystem::default();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to open PTY: {}", e))?;

        // Find clankers binary: prefer CARGO_BIN_EXE_clankers (set by cargo test),
        // then look next to our executable, then fall back to PATH
        let clankers_bin = if let Ok(bin) = std::env::var("CARGO_BIN_EXE_clankers") {
            std::path::PathBuf::from(bin)
        } else {
            let exe = std::env::current_exe().map_err(|e| format!("Can't find binary: {}", e))?;
            let dir = exe.parent().unwrap_or_else(|| std::path::Path::new("."));
            let candidates = [
                dir.join("clankers"),
                dir.parent().map(|p| p.join("clankers")).unwrap_or_default(),
            ];
            candidates
                .iter()
                .find(|p| p.exists() && p.is_file())
                .cloned()
                .unwrap_or_else(|| std::path::PathBuf::from("clankers"))
        };
        let mut cmd = CommandBuilder::new(&clankers_bin);
        cmd.args(["--no-zellij", "--no-daemon"]);
        for arg in extra_args {
            cmd.arg(arg);
        }
        cmd.env("RUST_LOG", "off");
        cmd.env("TERM", "xterm-256color");

        let child = pair.slave.spawn_command(cmd).map_err(|e| format!("Failed to spawn clankers: {}", e))?;
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader().map_err(|e| format!("Failed to clone reader: {}", e))?;
        let writer = pair.master.take_writer().map_err(|e| format!("Failed to take writer: {}", e))?;

        let parser = Arc::new(Mutex::new(Parser::new(rows, cols, 0)));
        let parser_clone = Arc::clone(&parser);
        let reader_thread = std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        parser_clone.lock().unwrap_or_else(|e| e.into_inner()).process(&buf[..n]);
                    }
                    Err(_) => break,
                }
            }
        });

        let harness = Self {
            parser,
            writer,
            _child: child,
            _reader_thread: reader_thread,
            rows,
            cols,
        };

        // Wait for initial render
        harness
            .wait_for("NORMAL", Duration::from_secs(10))
            .map_err(|e| format!("TUI failed to start: {}", e))?;

        Ok(harness)
    }

    pub(super) fn send(&mut self, data: &[u8]) -> Result<(), String> {
        self.writer.write_all(data).map_err(|e| format!("Write failed: {}", e))?;
        self.writer.flush().map_err(|e| format!("Flush failed: {}", e))?;
        Ok(())
    }

    pub(super) fn type_str(&mut self, s: &str) -> Result<(), String> {
        self.send(s.as_bytes())
    }

    pub(super) fn screen_text(&self) -> String {
        let parser = self.parser.lock().unwrap_or_else(|e| e.into_inner());
        let screen = parser.screen();
        let mut lines = Vec::new();
        for row in 0..self.rows {
            let mut line = String::new();
            for col in 0..self.cols {
                let Some(cell) = screen.cell(row, col) else { continue };
                line.push_str(cell.contents());
            }
            lines.push(line.trim_end().to_string());
        }
        lines.join("\n")
    }

    pub(super) fn screen_contains(&self, needle: &str) -> bool {
        self.screen_text().contains(needle)
    }

    #[cfg_attr(
        dylint_lib = "tigerstyle",
        allow(unbounded_loop, reason = "event loop; bounded by PTY close")
    )]
    pub(super) fn wait_for(&self, needle: &str, timeout: Duration) -> Result<(), String> {
        let start = Instant::now();
        loop {
            if self.screen_contains(needle) {
                return Ok(());
            }
            if start.elapsed() >= timeout {
                return Err(format!(
                    "Timed out after {:?} waiting for {:?}.\nScreen:\n{}",
                    timeout,
                    needle,
                    self.screen_text()
                ));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    #[cfg_attr(
        dylint_lib = "tigerstyle",
        allow(unbounded_loop, reason = "event loop; bounded by PTY close")
    )]
    pub(super) fn wait_for_absent(&self, needle: &str, timeout: Duration) -> Result<(), String> {
        let start = Instant::now();
        loop {
            if !self.screen_contains(needle) {
                return Ok(());
            }
            if start.elapsed() >= timeout {
                return Err(format!(
                    "Timed out after {:?} waiting for {:?} to disappear.\nScreen:\n{}",
                    timeout,
                    needle,
                    self.screen_text()
                ));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    pub(super) fn quit(&mut self) {
        self.send(b"\x1b").ok(); // Esc
        std::thread::sleep(Duration::from_millis(100));
        self.type_str("q").ok();
        std::thread::sleep(Duration::from_millis(300));
    }
}
