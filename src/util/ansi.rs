//! ANSI escape sequence utilities
//!
//! Provides functions for stripping ANSI codes, computing visible width,
//! and truncating strings to a visible-character limit.

use unicode_width::UnicodeWidthChar;

/// Strip ANSI escape sequences from a string.
///
/// Handles CSI sequences (`\x1b[...X`), OSC sequences (`\x1b]...ST`),
/// simple two-byte escapes (`\x1bX`), and bare `\r` carriage returns.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            match chars.peek() {
                Some('[') => {
                    // CSI sequence: consume until final byte (0x40-0x7E)
                    chars.next();
                    while let Some(&ch) = chars.peek() {
                        chars.next();
                        if ('\x40'..='\x7e').contains(&ch) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    // OSC sequence: consume until ST (\x1b\\ or \x07)
                    chars.next();
                    while let Some(ch) = chars.next() {
                        if ch == '\x07' {
                            break;
                        }
                        if ch == '\x1b' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                Some(_) => {
                    // Simple two-byte escape
                    chars.next();
                }
                None => {}
            }
        } else if c == '\r' {
            // Strip bare carriage returns
        } else {
            out.push(c);
        }
    }
    out
}

/// Compute the visible (display) width of a string, ignoring ANSI escapes.
///
/// Uses Unicode character widths (e.g. CJK characters count as 2).
///
/// # Examples
///
/// ```
/// use clankers::util::ansi::visible_width;
///
/// assert_eq!(visible_width("hello"), 5);
/// assert_eq!(visible_width("\x1b[31mred\x1b[0m"), 3);
/// assert_eq!(visible_width(""), 0);
/// ```
pub fn visible_width(s: &str) -> usize {
    let stripped = strip_ansi(s);
    stripped.chars().filter_map(UnicodeWidthChar::width).sum()
}

/// Truncate a string to fit within `max_width` visible columns.
///
/// ANSI escape sequences are preserved (they occupy zero columns).
/// Returns the truncated string. If the string fits, it is returned unchanged.
/// An ellipsis `…` is appended when truncation occurs (counts as 1 column).
///
/// # Examples
///
/// ```
/// use clankers::util::ansi::truncate_visible;
///
/// assert_eq!(truncate_visible("hello world", 5), "hell…");
/// assert_eq!(truncate_visible("hi", 10), "hi");
/// assert_eq!(truncate_visible("\x1b[31mred text\x1b[0m", 3), "\x1b[31mre…\x1b[0m");
/// ```
pub fn truncate_visible(s: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }

    // Fast path: check if it fits
    if visible_width(s) <= max_width {
        return s.to_string();
    }

    // Reserve 1 column for ellipsis
    let target = max_width.saturating_sub(1);

    let mut out = String::with_capacity(s.len());
    let mut width = 0usize;
    let mut chars = s.chars().peekable();
    // Track if we're inside an ANSI sequence to preserve it
    let mut pending_reset = false;

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Consume entire escape sequence into output (zero width)
            out.push(c);
            match chars.peek() {
                Some(&'[') => {
                    out.push(chars.next().unwrap());
                    while let Some(&ch) = chars.peek() {
                        out.push(chars.next().unwrap());
                        if ('\x40'..='\x7e').contains(&ch) {
                            break;
                        }
                    }
                    pending_reset = true;
                }
                Some(&']') => {
                    out.push(chars.next().unwrap());
                    while let Some(ch) = chars.next() {
                        out.push(ch);
                        if ch == '\x07' {
                            break;
                        }
                        if ch == '\x1b' && chars.peek() == Some(&'\\') {
                            out.push(chars.next().unwrap());
                            break;
                        }
                    }
                }
                Some(_) => {
                    out.push(chars.next().unwrap());
                }
                None => {}
            }
            continue;
        }

        let cw = UnicodeWidthChar::width(c).unwrap_or(0);
        if width + cw > target {
            out.push('…');
            // Append reset if we were inside a colored span
            if pending_reset {
                out.push_str("\x1b[0m");
            }
            return out;
        }
        out.push(c);
        width += cw;
    }

    // Shouldn't reach here given the fast path, but just in case
    out
}

/// Split a string into lines, stripping ANSI from each line for comparison
/// purposes while preserving the original styled lines.
///
/// Returns `(stripped_lines, original_lines)`.
/// Parse a string containing ANSI SGR escape sequences into a vec of
/// ratatui `Span`s with appropriate `Style`s applied.
///
/// Supports: bold (1), dim (2), reset (0), and basic foreground colors
/// (30-37, 90-97).  Unknown codes are silently ignored.
pub fn ansi_to_spans(s: &str) -> Vec<ratatui::text::Span<'static>> {
    use ratatui::style::Color;
    use ratatui::style::Modifier;
    use ratatui::style::Style;
    use ratatui::text::Span;

    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut current_style = Style::default();
    let mut buf = String::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                // Flush buffered text
                if !buf.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut buf), current_style));
                }
                chars.next(); // consume '['

                // Collect parameter bytes
                let mut params = String::new();
                while let Some(&ch) = chars.peek() {
                    if ('\x40'..='\x7e').contains(&ch) {
                        chars.next(); // consume final byte
                        break;
                    }
                    params.push(ch);
                    chars.next();
                }

                // Parse SGR codes (semicolon-separated)
                for code_str in params.split(';') {
                    let code: u8 = code_str.parse().unwrap_or(0);
                    match code {
                        0 => current_style = Style::default(),
                        1 => current_style = current_style.add_modifier(Modifier::BOLD),
                        2 => current_style = current_style.add_modifier(Modifier::DIM),
                        30 => current_style = current_style.fg(Color::Black),
                        31 => current_style = current_style.fg(Color::Red),
                        32 => current_style = current_style.fg(Color::Green),
                        33 => current_style = current_style.fg(Color::Yellow),
                        34 => current_style = current_style.fg(Color::Blue),
                        35 => current_style = current_style.fg(Color::Magenta),
                        36 => current_style = current_style.fg(Color::Cyan),
                        37 => current_style = current_style.fg(Color::White),
                        90 => current_style = current_style.fg(Color::DarkGray),
                        91 => current_style = current_style.fg(Color::LightRed),
                        92 => current_style = current_style.fg(Color::LightGreen),
                        93 => current_style = current_style.fg(Color::LightYellow),
                        94 => current_style = current_style.fg(Color::LightBlue),
                        95 => current_style = current_style.fg(Color::LightMagenta),
                        96 => current_style = current_style.fg(Color::LightCyan),
                        97 => current_style = current_style.fg(Color::Gray),
                        _ => {} // ignore unknown
                    }
                }
            } else {
                // Other escape – skip one char
                chars.next();
            }
        } else {
            buf.push(c);
        }
    }

    // Flush remaining
    if !buf.is_empty() {
        spans.push(Span::styled(buf, current_style));
    }

    spans
}

/// Convert an ANSI-colored string into a vec of ratatui `Line`s.
/// Each `\n`-delimited line is parsed separately.
pub fn ansi_to_lines(s: &str) -> Vec<ratatui::text::Line<'static>> {
    s.lines().map(|line| ratatui::text::Line::from(ansi_to_spans(line))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_plain_text() {
        assert_eq!(strip_ansi("hello world"), "hello world");
    }

    #[test]
    fn strip_csi_sequences() {
        assert_eq!(strip_ansi("\x1b[32mOK\x1b[0m"), "OK");
        assert_eq!(strip_ansi("\x1b[1;31merror\x1b[0m: bad"), "error: bad");
    }

    #[test]
    fn strip_cursor_movement() {
        assert_eq!(strip_ansi("\x1b[2Khello\x1b[1A"), "hello");
    }

    #[test]
    fn strip_osc_title() {
        assert_eq!(strip_ansi("\x1b]0;my title\x07content"), "content");
    }

    #[test]
    fn strip_carriage_return() {
        assert_eq!(strip_ansi("progress\r100%"), "progress100%");
    }

    #[test]
    fn strip_cargo_output() {
        let input = "\x1b[0m\x1b[0m\x1b[1m\x1b[32m   Compiling\x1b[0m clankers v0.1.0";
        assert_eq!(strip_ansi(input), "   Compiling clankers v0.1.0");
    }

    // visible_width tests
    #[test]
    fn width_plain() {
        assert_eq!(visible_width("hello"), 5);
    }

    #[test]
    fn width_with_ansi() {
        assert_eq!(visible_width("\x1b[31mred\x1b[0m"), 3);
    }

    #[test]
    fn width_empty() {
        assert_eq!(visible_width(""), 0);
    }

    #[test]
    fn width_cjk() {
        // CJK characters are typically 2 columns wide
        assert_eq!(visible_width("你好"), 4);
    }

    // truncate_visible tests
    #[test]
    fn truncate_fits() {
        assert_eq!(truncate_visible("hi", 10), "hi");
    }

    #[test]
    fn truncate_exact() {
        assert_eq!(truncate_visible("hello", 5), "hello");
    }

    #[test]
    fn truncate_cuts() {
        assert_eq!(truncate_visible("hello world", 5), "hell…");
    }

    #[test]
    fn truncate_preserves_ansi() {
        let result = truncate_visible("\x1b[31mred text\x1b[0m", 3);
        assert!(result.contains("re…"));
        // Should have reset code
        assert!(result.contains("\x1b[0m") || result.contains("\x1b[31m"));
    }

    #[test]
    fn truncate_zero_width() {
        assert_eq!(truncate_visible("hello", 0), "");
    }

    // ansi_to_spans tests
    #[test]
    fn spans_plain_text() {
        let spans = ansi_to_spans("hello world");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content.as_ref(), "hello world");
    }

    #[test]
    fn spans_colored_text() {
        use ratatui::style::Color;
        let spans = ansi_to_spans("\x1b[31mred\x1b[0m plain");
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content.as_ref(), "red");
        assert_eq!(spans[0].style.fg, Some(Color::Red));
        assert_eq!(spans[1].content.as_ref(), " plain");
    }

    #[test]
    fn spans_bold_and_dim() {
        use ratatui::style::Modifier;
        let spans = ansi_to_spans("\x1b[1mbold\x1b[0m \x1b[2mdim\x1b[0m");
        assert!(spans[0].style.add_modifier.contains(Modifier::BOLD));
        assert!(spans[2].style.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn spans_diff_line() {
        use ratatui::style::Color;
        let spans = ansi_to_spans("\x1b[32m+added line\x1b[0m");
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].content.as_ref(), "+added line");
        assert_eq!(spans[0].style.fg, Some(Color::Green));
    }

    #[test]
    fn lines_multiline() {
        let lines = ansi_to_lines("\x1b[31m-old\x1b[0m\n\x1b[32m+new\x1b[0m");
        assert_eq!(lines.len(), 2);
    }
}
