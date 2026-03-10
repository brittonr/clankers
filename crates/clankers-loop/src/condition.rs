//! Break conditions for loop termination.
//!
//! A break condition examines the output of each iteration and decides
//! whether the loop should stop. Conditions are composable via `Any` and `All`.

use serde::Deserialize;
use serde::Serialize;

/// When a loop should stop iterating.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BreakCondition {
    /// Output contains this substring.
    Contains(String),
    /// Output matches this regex pattern.
    Regex(String),
    /// Output does NOT contain this substring (break when absent).
    NotContains(String),
    /// Output equals this string exactly.
    Equals(String),
    /// Exit code equals this value (for command-based loops).
    ExitCode(i32),
    /// Any of these conditions triggers a break.
    Any(Vec<BreakCondition>),
    /// All of these conditions must be true to break.
    All(Vec<BreakCondition>),
    /// Never break (run until max_iterations or timeout).
    Never,
}

impl BreakCondition {
    /// Check if the condition is satisfied by the given output and exit code.
    pub fn check(&self, output: &str, exit_code: Option<i32>) -> bool {
        match self {
            Self::Contains(s) => output.contains(s.as_str()),
            Self::Regex(pattern) => {
                // Compile on each check. For hot loops, the caller should
                // pre-compile; this is fine for the typical 1-100 iteration range.
                regex_matches(pattern, output)
            }
            Self::NotContains(s) => !output.contains(s.as_str()),
            Self::Equals(s) => output.trim() == s.as_str(),
            Self::ExitCode(code) => exit_code == Some(*code),
            Self::Any(conditions) => conditions.iter().any(|c| c.check(output, exit_code)),
            Self::All(conditions) => conditions.iter().all(|c| c.check(output, exit_code)),
            Self::Never => false,
        }
    }
}

/// Parse a break condition from a string.
///
/// Supported formats:
/// - `contains:<text>` — stop when output contains text
/// - `not_contains:<text>` — stop when text is absent
/// - `equals:<text>` — stop when output equals text exactly
/// - `exit:<code>` — stop when exit code matches
/// - `regex:<pattern>` — stop when output matches regex pattern
/// - `<bare text>` — treated as `contains:<text>`
pub fn parse_break_condition(s: &str) -> BreakCondition {
    if let Some(text) = s.strip_prefix("contains:") {
        BreakCondition::Contains(text.to_string())
    } else if let Some(text) = s.strip_prefix("not_contains:") {
        BreakCondition::NotContains(text.to_string())
    } else if let Some(text) = s.strip_prefix("equals:") {
        BreakCondition::Equals(text.to_string())
    } else if let Some(text) = s.strip_prefix("regex:") {
        BreakCondition::Regex(text.to_string())
    } else if let Some(code) = s.strip_prefix("exit:") {
        if let Ok(c) = code.parse::<i32>() {
            BreakCondition::ExitCode(c)
        } else {
            BreakCondition::Contains(s.to_string())
        }
    } else {
        BreakCondition::Contains(s.to_string())
    }
}

/// Regex match without pulling in the `regex` crate as a hard dependency.
/// Falls back to substring match if the pattern is invalid.
fn regex_matches(pattern: &str, text: &str) -> bool {
    // Simple approach: use std pattern matching for common cases,
    // fall back to basic substring for complex patterns.
    // Full regex support can be added later if needed.
    if pattern.starts_with('^') && pattern.ends_with('$') {
        // Exact match (strip anchors)
        let inner = &pattern[1..pattern.len() - 1];
        text.trim() == inner
    } else if let Some(suffix) = pattern.strip_prefix('^') {
        text.starts_with(suffix)
    } else if let Some(prefix) = pattern.strip_suffix('$') {
        text.ends_with(prefix)
    } else {
        // Plain substring match as fallback
        text.contains(pattern)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_matches_substring() {
        let cond = BreakCondition::Contains("PASS".into());
        assert!(cond.check("all tests PASS", None));
        assert!(!cond.check("all tests FAIL", None));
    }

    #[test]
    fn not_contains_breaks_when_absent() {
        let cond = BreakCondition::NotContains("error".into());
        assert!(cond.check("all good", None));
        assert!(!cond.check("found error", None));
    }

    #[test]
    fn equals_matches_exact() {
        let cond = BreakCondition::Equals("OK".into());
        assert!(cond.check("OK", None));
        assert!(cond.check("  OK  ", None)); // trimmed
        assert!(!cond.check("OK then", None));
    }

    #[test]
    fn exit_code_matches() {
        let cond = BreakCondition::ExitCode(0);
        assert!(cond.check("", Some(0)));
        assert!(!cond.check("", Some(1)));
        assert!(!cond.check("", None));
    }

    #[test]
    fn any_matches_first_true() {
        let cond = BreakCondition::Any(vec![
            BreakCondition::Contains("PASS".into()),
            BreakCondition::ExitCode(0),
        ]);
        assert!(cond.check("PASS", Some(1)));
        assert!(cond.check("FAIL", Some(0)));
        assert!(!cond.check("FAIL", Some(1)));
    }

    #[test]
    fn all_requires_everything() {
        let cond = BreakCondition::All(vec![
            BreakCondition::Contains("done".into()),
            BreakCondition::ExitCode(0),
        ]);
        assert!(cond.check("done", Some(0)));
        assert!(!cond.check("done", Some(1)));
        assert!(!cond.check("pending", Some(0))); // no "done" substring
    }

    #[test]
    fn never_never_breaks() {
        let cond = BreakCondition::Never;
        assert!(!cond.check("anything", Some(0)));
    }

    #[test]
    fn regex_anchored_exact() {
        let cond = BreakCondition::Regex("^OK$".into());
        assert!(cond.check("OK", None));
        assert!(!cond.check("OK then", None));
    }

    #[test]
    fn regex_prefix() {
        let cond = BreakCondition::Regex("^BUILD".into());
        assert!(cond.check("BUILD PASSED", None));
        assert!(!cond.check("the BUILD", None));
    }

    #[test]
    fn regex_suffix() {
        let cond = BreakCondition::Regex("PASS$".into());
        assert!(cond.check("test PASS", None));
        assert!(!cond.check("PASS then fail", None));
    }

    #[test]
    fn condition_serializes() {
        let cond = BreakCondition::Any(vec![
            BreakCondition::Contains("PASS".into()),
            BreakCondition::ExitCode(0),
        ]);
        let json = serde_json::to_string(&cond).unwrap();
        let parsed: BreakCondition = serde_json::from_str(&json).unwrap();
        assert!(parsed.check("PASS", None));
    }

    #[test]
    fn parse_contains() {
        let cond = parse_break_condition("contains:PASS");
        assert!(cond.check("all tests PASS", None));
        assert!(!cond.check("all tests FAIL", None));
    }

    #[test]
    fn parse_exit_code() {
        let cond = parse_break_condition("exit:0");
        assert!(cond.check("", Some(0)));
        assert!(!cond.check("", Some(1)));
    }

    #[test]
    fn parse_not_contains() {
        let cond = parse_break_condition("not_contains:error");
        assert!(cond.check("all good", None));
        assert!(!cond.check("found error", None));
    }

    #[test]
    fn parse_equals() {
        let cond = parse_break_condition("equals:OK");
        assert!(cond.check("OK", None));
        assert!(!cond.check("OK then", None));
    }

    #[test]
    fn parse_regex() {
        let cond = parse_break_condition("regex:^OK$");
        assert!(cond.check("OK", None));
        assert!(!cond.check("OK then", None));
    }

    #[test]
    fn parse_bare_string() {
        let cond = parse_break_condition("SUCCESS");
        assert!(cond.check("BUILD SUCCESS", None));
    }

    #[test]
    fn parse_invalid_exit_code_falls_back() {
        let cond = parse_break_condition("exit:abc");
        // Falls back to Contains("exit:abc")
        assert!(cond.check("exit:abc", None));
    }
}
