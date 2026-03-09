//! PTY-based TUI test harness
//!
//! Spawns clankers in a real pseudo-terminal, sends keystrokes, and reads back
//! the rendered screen via a vt100 parser.

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

/// A running clankers TUI instance inside a PTY
pub struct TuiTestHarness {
    parser: Arc<Mutex<Parser>>,
    writer: Box<dyn Write + Send>,
    _child: Box<dyn portable_pty::Child + Send + Sync>,
    _reader_thread: std::thread::JoinHandle<()>,
    rows: u16,
    cols: u16,
}

impl TuiTestHarness {
    /// Spawn clankers in a PTY with the given dimensions.
    /// Waits for the initial render before returning.
    pub fn spawn(rows: u16, cols: u16) -> Self {
        let pty_system = NativePtySystem::default();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("Failed to open PTY");

        let mut cmd = CommandBuilder::new(env!("CARGO_BIN_EXE_clankers"));
        cmd.args(["--no-zellij"]);
        cmd.env("RUST_LOG", "off");
        cmd.env("TERM", "xterm-256color");
        // Ensure CWD is the project root so plugins/ dir is discovered
        cmd.cwd(env!("CARGO_MANIFEST_DIR"));

        let child = pair.slave.spawn_command(cmd).expect("Failed to spawn clankers");
        drop(pair.slave); // close slave side so reads don't hang

        let mut reader = pair.master.try_clone_reader().expect("Failed to clone reader");
        let writer = pair.master.take_writer().expect("Failed to take writer");

        let parser = Arc::new(Mutex::new(Parser::new(rows, cols, 0)));

        // Spawn a background thread that continuously reads from the PTY
        // and feeds bytes into the vt100 parser.
        let parser_clone = Arc::clone(&parser);
        let reader_thread = std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break, // EOF — process exited
                    Ok(n) => {
                        parser_clone.lock().unwrap().process(&buf[..n]);
                    }
                    Err(_) => break,
                }
            }
        });

        let mut harness = Self {
            parser,
            writer,
            _child: child,
            _reader_thread: reader_thread,
            rows,
            cols,
        };

        // Wait for initial render
        harness.wait_for_text("NORMAL", Duration::from_secs(10));
        harness
    }

    /// Send raw bytes (keystrokes) to the PTY
    pub fn send(&mut self, data: &[u8]) {
        self.writer.write_all(data).expect("Failed to write to PTY");
        self.writer.flush().expect("Failed to flush PTY");
    }

    /// Send a string as keystrokes
    pub fn type_str(&mut self, s: &str) {
        self.send(s.as_bytes());
    }

    /// Send a key with escape sequence
    pub fn send_key(&mut self, key: Key) {
        self.send(key.as_bytes());
    }

    /// Wait a bit for the TUI to process input and re-render
    pub fn settle(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }

    /// Wait until the given text appears somewhere on screen, or panic after timeout
    pub fn wait_for_text(&mut self, needle: &str, timeout: Duration) {
        let start = Instant::now();
        loop {
            if self.screen_text().contains(needle) {
                return;
            }
            if start.elapsed() >= timeout {
                panic!("Timed out waiting for {:?} on screen.\nScreen contents:\n{}", needle, self.screen_text());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Get the full screen contents as a string (one line per row)
    pub fn screen_text(&self) -> String {
        let parser = self.parser.lock().unwrap();
        let screen = parser.screen();
        let mut lines = Vec::new();
        for row in 0..self.rows {
            let mut line = String::new();
            for col in 0..self.cols {
                let cell = screen.cell(row, col).unwrap();
                line.push_str(cell.contents());
            }
            lines.push(line.trim_end().to_string());
        }
        lines.join("\n")
    }

    /// Get the text content of a specific row (0-indexed)
    pub fn row_text(&self, row: u16) -> String {
        let parser = self.parser.lock().unwrap();
        let screen = parser.screen();
        let mut line = String::new();
        for col in 0..self.cols {
            let cell = screen.cell(row, col).unwrap();
            line.push_str(cell.contents());
        }
        line.trim_end().to_string()
    }

    /// Check if any row contains the given text
    pub fn screen_contains(&self, needle: &str) -> bool {
        self.screen_text().contains(needle)
    }

    /// Get the status bar text.
    ///
    /// Scans upward from the bottom to skip border rows added by the
    /// focused main-panel frame (`Borders::ALL`).
    pub fn status_bar(&self) -> String {
        for row in (0..self.rows).rev() {
            let text = self.row_text(row);
            let trimmed = text.trim();
            if trimmed.is_empty() {
                continue;
            }
            // Skip rows that are entirely box-drawing / border characters
            if trimmed
                .chars()
                .all(|c| matches!(c, '─' | '│' | '┌' | '┐' | '└' | '┘' | '├' | '┤' | '┬' | '┴' | '┼' | ' '))
            {
                continue;
            }
            return text;
        }
        // Fallback to the absolute last row
        self.row_text(self.rows - 1)
    }

    /// Quit cleanly: Esc to normal mode, then q
    pub fn quit(&mut self) {
        self.send_key(Key::Escape);
        self.settle(Duration::from_millis(100));
        self.type_str("q");
        self.settle(Duration::from_millis(300));
    }
}

use std::io::Read;

/// Named keys for sending to the terminal
pub enum Key {
    Enter,
    Escape,
    Up,
    Down,
    Left,
    Right,
    Tab,
    ShiftTab,
    Backspace,
    Delete,
    Home,
    End,
    CtrlC,
    CtrlD,
    CtrlT,
    CtrlU,
    CtrlW,
    AltEnter,
    PageUp,
    PageDown,
}

impl Key {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Key::Enter => b"\r",
            Key::Escape => b"\x1b",
            Key::Up => b"\x1b[A",
            Key::Down => b"\x1b[B",
            Key::Right => b"\x1b[C",
            Key::Left => b"\x1b[D",
            Key::Tab => b"\t",
            Key::ShiftTab => b"\x1b[Z",
            Key::Backspace => b"\x7f",
            Key::Delete => b"\x1b[3~",
            Key::Home => b"\x1b[H",
            Key::End => b"\x1b[F",
            Key::CtrlC => b"\x03",
            Key::CtrlD => b"\x04",
            Key::CtrlT => b"\x14",
            Key::CtrlU => b"\x15",
            Key::CtrlW => b"\x17",
            Key::AltEnter => b"\x1b\r",
            Key::PageUp => b"\x1b[5~",
            Key::PageDown => b"\x1b[6~",
        }
    }
}
