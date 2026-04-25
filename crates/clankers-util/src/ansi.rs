//! ANSI escape sequence utilities
//!
//! Provides a function for stripping ANSI codes from strings.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

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
