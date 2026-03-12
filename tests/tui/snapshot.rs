//! Snapshot testing utilities for the TUI
//!
//! Provides structured screen capture with color/style information,
//! and wrappers around `insta` for both text and styled snapshots.
//!
//! Snapshot workflow:
//!   1. Tests call `assert_text_snapshot()` or `assert_styled_snapshot()`
//!   2. First run creates snapshot files in `tests/tui/snapshots/`
//!   3. Subsequent runs compare against stored snapshots
//!   4. Review changes with `cargo insta review`

use std::fmt;

use vt100::Parser;

/// A single terminal cell with content and style attributes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledCell {
    pub text: String,
    pub fg: TermColor,
    pub bg: TermColor,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
}

/// Terminal color — either a named ANSI color, 256-palette index, or RGB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TermColor {
    Default,
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
    Palette(u8),
    Rgb(u8, u8, u8),
}

impl fmt::Display for TermColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TermColor::Default => write!(f, "-"),
            TermColor::Black => write!(f, "blk"),
            TermColor::Red => write!(f, "red"),
            TermColor::Green => write!(f, "grn"),
            TermColor::Yellow => write!(f, "yel"),
            TermColor::Blue => write!(f, "blu"),
            TermColor::Magenta => write!(f, "mag"),
            TermColor::Cyan => write!(f, "cyn"),
            TermColor::White => write!(f, "wht"),
            TermColor::BrightBlack => write!(f, "Blk"),
            TermColor::BrightRed => write!(f, "Red"),
            TermColor::BrightGreen => write!(f, "Grn"),
            TermColor::BrightYellow => write!(f, "Yel"),
            TermColor::BrightBlue => write!(f, "Blu"),
            TermColor::BrightMagenta => write!(f, "Mag"),
            TermColor::BrightCyan => write!(f, "Cyn"),
            TermColor::BrightWhite => write!(f, "Wht"),
            TermColor::Palette(n) => write!(f, "p{n}"),
            TermColor::Rgb(r, g, b) => write!(f, "#{r:02x}{g:02x}{b:02x}"),
        }
    }
}

impl TermColor {
    /// Convert to an RGBA color for screenshot rendering.
    pub fn to_rgba(&self) -> [u8; 4] {
        match self {
            TermColor::Default => [0, 0, 0, 255], // black bg default
            TermColor::Black => [0, 0, 0, 255],
            TermColor::Red => [170, 0, 0, 255],
            TermColor::Green => [0, 170, 0, 255],
            TermColor::Yellow => [170, 85, 0, 255],
            TermColor::Blue => [0, 0, 170, 255],
            TermColor::Magenta => [170, 0, 170, 255],
            TermColor::Cyan => [0, 170, 170, 255],
            TermColor::White => [170, 170, 170, 255],
            TermColor::BrightBlack => [85, 85, 85, 255],
            TermColor::BrightRed => [255, 85, 85, 255],
            TermColor::BrightGreen => [85, 255, 85, 255],
            TermColor::BrightYellow => [255, 255, 85, 255],
            TermColor::BrightBlue => [85, 85, 255, 255],
            TermColor::BrightMagenta => [255, 85, 255, 255],
            TermColor::BrightCyan => [85, 255, 255, 255],
            TermColor::BrightWhite => [255, 255, 255, 255],
            TermColor::Palette(n) => palette_to_rgb(*n),
            TermColor::Rgb(r, g, b) => [*r, *g, *b, 255],
        }
    }

    /// Convert to an RGBA color suitable for foreground text (uses white for Default).
    pub fn to_fg_rgba(&self) -> [u8; 4] {
        match self {
            TermColor::Default => [204, 204, 204, 255], // light gray
            other => other.to_rgba(),
        }
    }
}

fn vt100_color_to_term(color: vt100::Color) -> TermColor {
    match color {
        vt100::Color::Default => TermColor::Default,
        vt100::Color::Idx(0) => TermColor::Black,
        vt100::Color::Idx(1) => TermColor::Red,
        vt100::Color::Idx(2) => TermColor::Green,
        vt100::Color::Idx(3) => TermColor::Yellow,
        vt100::Color::Idx(4) => TermColor::Blue,
        vt100::Color::Idx(5) => TermColor::Magenta,
        vt100::Color::Idx(6) => TermColor::Cyan,
        vt100::Color::Idx(7) => TermColor::White,
        vt100::Color::Idx(8) => TermColor::BrightBlack,
        vt100::Color::Idx(9) => TermColor::BrightRed,
        vt100::Color::Idx(10) => TermColor::BrightGreen,
        vt100::Color::Idx(11) => TermColor::BrightYellow,
        vt100::Color::Idx(12) => TermColor::BrightBlue,
        vt100::Color::Idx(13) => TermColor::BrightMagenta,
        vt100::Color::Idx(14) => TermColor::BrightCyan,
        vt100::Color::Idx(15) => TermColor::BrightWhite,
        vt100::Color::Idx(n) => TermColor::Palette(n),
        vt100::Color::Rgb(r, g, b) => TermColor::Rgb(r, g, b),
    }
}

/// Convert a 256-palette index to RGB.
fn palette_to_rgb(n: u8) -> [u8; 4] {
    match n {
        0..=15 => {
            // Standard colors — handled by TermColor variants
            let tc = match n {
                0 => TermColor::Black,
                1 => TermColor::Red,
                2 => TermColor::Green,
                3 => TermColor::Yellow,
                4 => TermColor::Blue,
                5 => TermColor::Magenta,
                6 => TermColor::Cyan,
                7 => TermColor::White,
                8 => TermColor::BrightBlack,
                9 => TermColor::BrightRed,
                10 => TermColor::BrightGreen,
                11 => TermColor::BrightYellow,
                12 => TermColor::BrightBlue,
                13 => TermColor::BrightMagenta,
                14 => TermColor::BrightCyan,
                15 => TermColor::BrightWhite,
                _ => unreachable!(),
            };
            tc.to_rgba()
        }
        16..=231 => {
            // 6×6×6 color cube
            let n = n - 16;
            let r = (n / 36) % 6;
            let g = (n / 6) % 6;
            let b = n % 6;
            let to_val = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
            [to_val(r), to_val(g), to_val(b), 255]
        }
        232..=255 => {
            // Grayscale ramp
            let v = 8 + 10 * (n - 232);
            [v, v, v, 255]
        }
    }
}

/// Full screen capture with per-cell style information.
pub struct ScreenCapture {
    pub rows: u16,
    pub cols: u16,
    pub cells: Vec<Vec<StyledCell>>,
}

impl ScreenCapture {
    /// Capture the current PTY harness screen with style information.
    pub fn from_pty(harness: &super::harness::TuiTestHarness) -> Self {
        let parser = harness.parser();
        let screen = parser.screen();
        let (rows, cols) = harness.size();

        let mut cells = Vec::with_capacity(rows as usize);
        for row in 0..rows {
            let mut row_cells = Vec::with_capacity(cols as usize);
            for col in 0..cols {
                let cell = screen.cell(row, col).unwrap();
                row_cells.push(StyledCell {
                    text: cell.contents().to_string(),
                    fg: vt100_color_to_term(cell.fgcolor()),
                    bg: vt100_color_to_term(cell.bgcolor()),
                    bold: cell.bold(),
                    dim: cell.dim(),
                    italic: cell.italic(),
                    underline: cell.underline(),
                    inverse: cell.inverse(),
                });
            }
            cells.push(row_cells);
        }

        Self { rows, cols, cells }
    }

    /// Capture from raw ANSI text (e.g., tmux capture-pane -e output).
    pub fn from_ansi(ansi: &str, rows: u16, cols: u16) -> Self {
        let mut parser = Parser::new(rows, cols, 0);
        parser.process(ansi.as_bytes());
        let screen = parser.screen();

        let mut cells = Vec::with_capacity(rows as usize);
        for row in 0..rows {
            let mut row_cells = Vec::with_capacity(cols as usize);
            for col in 0..cols {
                let cell = screen.cell(row, col).unwrap();
                row_cells.push(StyledCell {
                    text: cell.contents().to_string(),
                    fg: vt100_color_to_term(cell.fgcolor()),
                    bg: vt100_color_to_term(cell.bgcolor()),
                    bold: cell.bold(),
                    dim: cell.dim(),
                    italic: cell.italic(),
                    underline: cell.underline(),
                    inverse: cell.inverse(),
                });
            }
            cells.push(row_cells);
        }

        Self { rows, cols, cells }
    }

    /// Format as a styled text representation suitable for snapshot testing.
    ///
    /// Output format encodes color runs compactly. Each row is one line.
    /// Color changes are marked with `{fg/bg}` tags.
    /// Style modifiers: `*` bold, `_` underline, `~` dim, `/` italic.
    ///
    /// Example:
    /// ```text
    /// {cyn/-} Messages {Blk/-}──────────────────
    /// {-/-}│                                    │
    /// ```
    pub fn styled_text(&self) -> String {
        let mut out = String::new();

        for (row_idx, row) in self.cells.iter().enumerate() {
            if row_idx > 0 {
                out.push('\n');
            }

            let mut prev_fg = TermColor::Default;
            let mut prev_bg = TermColor::Default;
            let mut prev_bold = false;
            let mut prev_underline = false;

            for cell in row {
                // Emit style tag if anything changed
                let style_changed = cell.fg != prev_fg
                    || cell.bg != prev_bg
                    || cell.bold != prev_bold
                    || cell.underline != prev_underline;

                if style_changed {
                    out.push('{');
                    if cell.bold {
                        out.push('*');
                    }
                    if cell.underline {
                        out.push('_');
                    }
                    out.push_str(&cell.fg.to_string());
                    out.push('/');
                    out.push_str(&cell.bg.to_string());
                    out.push('}');

                    prev_fg = cell.fg.clone();
                    prev_bg = cell.bg.clone();
                    prev_bold = cell.bold;
                    prev_underline = cell.underline;
                }

                let ch = if cell.text.is_empty() { " " } else { &cell.text };
                out.push_str(ch);
            }

            // Trim trailing default-styled spaces
            while out.ends_with("{-/-} ") {
                out.truncate(out.len() - 6);
            }
        }

        // Trim trailing empty lines
        while out.ends_with('\n') {
            out.pop();
        }

        out
    }

    /// Format as plain text (one row per line, trailing spaces trimmed).
    pub fn plain_text(&self) -> String {
        let mut out = String::new();
        for (row_idx, row) in self.cells.iter().enumerate() {
            if row_idx > 0 {
                out.push('\n');
            }
            let mut line = String::new();
            for cell in row {
                let ch = if cell.text.is_empty() { " " } else { &cell.text };
                line.push_str(ch);
            }
            out.push_str(line.trim_end());
        }

        // Trim trailing empty lines
        while out.ends_with('\n') {
            out.pop();
        }

        out
    }
}

/// Assert that the current PTY screen matches an insta text snapshot.
macro_rules! assert_text_snapshot {
    ($name:expr, $harness:expr) => {{
        let text = $harness.screen_text();
        insta::assert_snapshot!($name, text);
    }};
}
pub(crate) use assert_text_snapshot;

/// Assert that the current PTY screen matches an insta styled snapshot.
///
/// Captures cell-level color and style information.
macro_rules! assert_styled_snapshot {
    ($name:expr, $harness:expr) => {{
        let capture = super::snapshot::ScreenCapture::from_pty($harness);
        let styled = capture.styled_text();
        insta::assert_snapshot!($name, styled);
    }};
}
pub(crate) use assert_styled_snapshot;

/// Assert that tmux pane text matches an insta snapshot.
macro_rules! assert_tmux_snapshot {
    ($name:expr, $harness:expr) => {{
        let text = $harness.capture_text();
        // Trim trailing blank lines for cleaner snapshots
        let trimmed: String = text
            .lines()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .skip_while(|l| l.trim().is_empty())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        insta::assert_snapshot!($name, trimmed);
    }};
}
pub(crate) use assert_tmux_snapshot;

/// Assert that the current PTY screen matches an insta snapshot,
/// with dynamic content (git status, token counts) normalized.
macro_rules! assert_normalized_snapshot {
    ($name:expr, $harness:expr) => {{
        let text = $harness.screen_text();
        let normalized = super::snapshot::normalize_screen_text(&text);
        insta::assert_snapshot!($name, normalized);
    }};
}
pub(crate) use assert_normalized_snapshot;

/// Assert on just the structural elements: first row, status bar, and
/// panel borders. Ignores volatile message content entirely.
macro_rules! assert_structure_snapshot {
    ($name:expr, $harness:expr) => {{
        let structure = super::snapshot::extract_structure($harness);
        insta::assert_snapshot!($name, structure);
    }};
}
pub(crate) use assert_structure_snapshot;

/// Assert that tmux pane text matches an insta snapshot, normalized.
macro_rules! assert_tmux_normalized_snapshot {
    ($name:expr, $harness:expr) => {{
        let text = $harness.capture_text();
        let trimmed: String = text
            .lines()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .skip_while(|l| l.trim().is_empty())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        let normalized = super::snapshot::normalize_screen_text(&trimmed);
        insta::assert_snapshot!($name, normalized);
    }};
}
pub(crate) use assert_tmux_normalized_snapshot;

/// Assert that tmux pane ANSI output matches an insta styled snapshot.
macro_rules! assert_tmux_styled_snapshot {
    ($name:expr, $harness:expr) => {{
        let ansi = $harness.capture_ansi();
        let (rows, cols) = $harness.size();
        let capture = super::snapshot::ScreenCapture::from_ansi(&ansi, rows, cols);
        let styled = capture.styled_text();
        insta::assert_snapshot!($name, styled);
    }};
}
pub(crate) use assert_tmux_styled_snapshot;

/// Normalize dynamic content in screen text for stable snapshot comparison.
///
/// Replaces volatile fields (git status counters, timestamps, etc.)
/// with stable placeholders so snapshots don't break on unrelated changes.
pub fn normalize_screen_text(text: &str) -> String {
    text.lines().map(|l| normalize_line(l)).collect::<Vec<_>>().join("\n")
}

fn normalize_line(line: &str) -> String {
    let mut s = line.to_string();

    // Normalize git branch status: strip dirty indicators entirely.
    // "main *~6?37" / "main *?7" / "main *N" / "main" → "main"
    // This eliminates snapshot oscillation between clean and dirty working trees.
    let re_git = regex::Regex::new(r"\s*\*[~?+\dN]+").unwrap();
    s = re_git.replace_all(&s, "").to_string();

    // Normalize token counters: "0/200.0k" → "N/N.Nk"
    let re_tokens = regex::Regex::new(r"\d+/\d+\.\d+k").unwrap();
    s = re_tokens.replace_all(&s, "N/N.Nk").to_string();

    // Normalize worktree IDs: "main-aa048f02" → "main-XXXXXXXX"
    let re_wt = regex::Regex::new(r"(main|master)-[0-9a-f]{6,}").unwrap();
    s = re_wt.replace_all(&s, "${1}-XXXXXXXX").to_string();

    // Normalize commit hashes
    let re_hash = regex::Regex::new(r"\b[0-9a-f]{7,40}\b").unwrap();
    s = re_hash.replace_all(&s, "HASH").to_string();

    // Strip "Worktree: <anything>" entirely — this line appears/disappears
    // depending on git session state, causing snapshot instability
    let re_worktree = regex::Regex::new(r"Worktree:\s*\S+").unwrap();
    s = re_worktree.replace_all(&s, "").to_string();

    // Normalize model names — covers claude, qwen, gpt, gemini, deepseek, llama, etc.
    let re_model = regex::Regex::new(
        r"(?:claude-[a-z0-9.-]+|gpt-[a-z0-9.-]+|o[1-4]-[a-z0-9.-]*|qwen[a-z0-9.:_-]+|gemini[a-z0-9.:/_-]+|deepseek[a-z0-9.:_-]+|llama[a-z0-9.:_-]+|mistral[a-z0-9.:_-]+|phi[a-z0-9.:_-]+)"
    ).unwrap();
    s = re_model.replace_all(&s, "MODEL").to_string();

    // Catch-all: normalize model name in status bar "idle | <model> |" context
    let re_sb_model = regex::Regex::new(r"(idle\s*\|\s*)\S+(\s*\|)").unwrap();
    s = re_sb_model.replace_all(&s, "${1}MODEL${2}").to_string();

    // Strip the working directory path from status bar lines.
    // The status bar renders "idle | model | /path/to/cwd" but the path varies by
    // machine and gets truncated differently depending on terminal width.
    // Only apply to status bar lines (contain mode badge).
    if s.contains(" NORMAL ") || s.contains(" INSERT ") {
        // Strip "| /path..." suffix (absolute path after last pipe)
        let re_cwd = regex::Regex::new(r"\s*\|\s*/\S*\s*$").unwrap();
        s = re_cwd.replace_all(&s, "").to_string();

        // Strip trailing bare "|" left over from truncated paths on narrow terminals
        let re_bare_pipe = regex::Regex::new(r"\s*\|\s*$").unwrap();
        s = re_bare_pipe.replace_all(&s, "").to_string();

        // Strip trailing state info that leaks through width-truncation.
        let re_state_leak = regex::Regex::new(r"\s+(idle|id|streaming|str|command|com|dialog|dia)\s*$").unwrap();
        s = re_state_leak.replace_all(&s, "").to_string();
    }

    // Normalize trailing whitespace differences
    s.trim_end().to_string()
}

/// Extract structural UI elements from the screen, ignoring volatile
/// message content. Returns a compact representation of:
/// - Header row (panel titles, navigation hints)
/// - Status bar (mode, model, token count)
/// - Panel borders and titles
/// - Input area
///
/// This is stable across runs regardless of session state.
pub fn extract_structure(harness: &super::harness::TuiTestHarness) -> String {
    let text = harness.screen_text();
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        // Keep structural lines: borders, titles, status bar, input
        let is_border = trimmed.chars().any(|c| matches!(c, '┌' | '┐' | '└' | '┘' | '├' | '┤'));
        let is_status = trimmed.contains("NORMAL")
            || trimmed.contains("INSERT")
            || trimmed.contains("VISUAL")
            || trimmed.contains("COMMAND");
        let is_input = trimmed.contains("Input");
        let is_first = i == 0;
        let is_last = i == lines.len() - 1;
        let is_panel_title = trimmed.contains("Todo")
            || trimmed.contains("Files")
            || trimmed.contains("Messages")
            || trimmed.contains("Subagents")
            || trimmed.contains("Peers")
            || trimmed.contains("Space")
            || trimmed.contains("Commands");

        if is_border || is_status || is_input || is_first || is_last || is_panel_title {
            let mut normalized = normalize_line(line);

            // Strip cursor/input artifacts: partial text fragments that appear
            // between the last status bar pipe and border (from PTY timing).
            // Catches: "idle | /│", "idle | cla └", "MODEL | /ho │"
            let re_artifact = regex::Regex::new(r"\|\s*[/a-zA-Z`][/a-zA-Z0-9._-]*(\s*[│┘┐┤└])").unwrap();
            normalized = re_artifact.replace_all(&normalized, "|$1").to_string();

            // Strip volatile chat content bleeding into panel border lines
            // e.g. "│press i to sta┌ Space ──" → "│┌ Space ──"
            // Also catches: "│  clankers — c┌ Space ──" (greeting behind popup)
            let re_bleed = regex::Regex::new(r"│[^│┌┐└┘┤├─]{2,}(┌)").unwrap();
            normalized = re_bleed.replace_all(&normalized, "│$1").to_string();

            // Normalize whitespace before border chars — collapse any
            // amount of whitespace (0+) to exactly one space
            let re_space = regex::Regex::new(r"\s*([│┘┐┤└])").unwrap();
            normalized = re_space.replace_all(&normalized, " $1").to_string();

            // Strip artifacts after the last border char
            if let Some(pos) = normalized.rfind(|c: char| matches!(c, '│' | '┘' | '┐' | '┤' | '└')) {
                let end = pos + '│'.len_utf8();
                if end < normalized.len() {
                    let trailing = &normalized[end..];
                    if trailing.trim().len() <= 2 {
                        normalized.truncate(end);
                    }
                }
            }

            out.push(format!("{:>3}: {}", i, normalized));
        }
    }

    out.join("\n")
}

/// Save a raw ANSI capture to a file for visual inspection.
/// View with `cat tests/tui/captures/NAME.ansi` in a terminal.
pub fn save_ansi_capture(name: &str, ansi: &str) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("tui").join("captures");
    std::fs::create_dir_all(&dir).ok();
    let path = dir.join(format!("{name}.ansi"));
    std::fs::write(&path, ansi).expect("failed to write ANSI capture");
}
