//! ANSI escape sequence utilities
//!
//! Provides a function for stripping ANSI codes from strings.

/// Strip ANSI escape sequences from a string.
///
/// Handles CSI sequences (`\x1b[...X`), OSC sequences (`\x1b]...ST`),
/// simple two-byte escapes (`\x1bX`), and bare `\r` carriage returns.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut index = 0usize;

    while index < chars.len() {
        let c = chars[index];
        index = index.saturating_add(1);

        if c == '\x1b' {
            index = skip_escape_sequence(&chars, index);
        } else if c != '\r' {
            out.push(c);
        }
    }

    assert!(out.len() <= s.len());
    assert!(!out.contains('\x1b'));
    out
}

fn skip_escape_sequence(chars: &[char], index: usize) -> usize {
    if index >= chars.len() {
        return index;
    }

    match chars[index] {
        '[' => skip_csi_sequence(chars, index.saturating_add(1)),
        ']' => skip_osc_sequence(chars, index.saturating_add(1)),
        _ => index.saturating_add(1),
    }
}

fn skip_csi_sequence(chars: &[char], mut index: usize) -> usize {
    while index < chars.len() {
        let ch = chars[index];
        index = index.saturating_add(1);
        if ('\x40'..='\x7e').contains(&ch) {
            break;
        }
    }
    index
}

fn skip_osc_sequence(chars: &[char], mut index: usize) -> usize {
    while index < chars.len() {
        let ch = chars[index];
        index = index.saturating_add(1);
        if ch == '\x07' {
            break;
        }
        if ch == '\x1b' && chars.get(index) == Some(&'\\') {
            index = index.saturating_add(1);
            break;
        }
    }
    index
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
}
