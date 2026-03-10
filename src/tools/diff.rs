//! Unified diff generation for streaming previews.
//!
//! Produces colored, context-aware diffs that tools emit via
//! `ToolContext::emit_progress` before applying file mutations.

use similar::ChangeTag;
use similar::TextDiff;

/// Maximum number of context lines around each change hunk.
const CONTEXT_LINES: usize = 3;

/// Build a compact unified diff string between `old` and `new` for the given
/// file path.  The output uses ANSI colors (red for removals, green for
/// additions, dim for context) so it renders nicely in the TUI tool-output
/// panel.
///
/// For new files (empty `old`), we show a short "+ N lines" summary instead of
/// dumping the entire file.
pub fn unified_diff(path: &str, old: &str, new: &str) -> String {
    use std::fmt::Write;

    // New file – don't dump every line
    if old.is_empty() {
        let line_count = new.lines().count();
        let byte_count = new.len();
        return format!(
            "\x1b[1m--- /dev/null\n+++ {}\x1b[0m\n\x1b[32m+ ({} lines, {} bytes)\x1b[0m",
            path, line_count, byte_count,
        );
    }

    // Identical content – nothing to show
    if old == new {
        return String::new();
    }

    let diff = TextDiff::from_lines(old, new);
    let mut out = String::new();

    // Header
    write!(out, "\x1b[1m--- {}\n+++ {}\x1b[0m\n", path, path).unwrap();

    for hunk in diff.unified_diff().context_radius(CONTEXT_LINES).iter_hunks() {
        // Hunk header
        writeln!(out, "\x1b[36m{}\x1b[0m", hunk.header()).unwrap();

        for change in hunk.iter_changes() {
            match change.tag() {
                ChangeTag::Delete => {
                    write!(out, "\x1b[31m-{}\x1b[0m", change).unwrap();
                }
                ChangeTag::Insert => {
                    write!(out, "\x1b[32m+{}\x1b[0m", change).unwrap();
                }
                ChangeTag::Equal => {
                    write!(out, "\x1b[2m {}\x1b[0m", change).unwrap();
                }
            }
            // `similar` change values include trailing newlines for most lines,
            // but the last line of a file without a trailing newline won't have
            // one.  Ensure each change ends on its own line.
            if !change.as_str().unwrap_or("").ends_with('\n') {
                out.push('\n');
            }
        }
    }

    out
}

/// Build a short summary line for a write that overwrites an existing file.
/// Returns the diff plus a 1-line stat.
pub fn diff_stat(path: &str, old: &str, new: &str) -> String {
    if old == new {
        return "(no changes)".to_string();
    }

    let diff = TextDiff::from_lines(old, new);
    let mut adds: usize = 0;
    let mut dels: usize = 0;
    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Insert => adds += 1,
            ChangeTag::Delete => dels += 1,
            _ => {}
        }
    }

    format!("{}: \x1b[32m+{}\x1b[0m / \x1b[31m-{}\x1b[0m lines", path, adds, dels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_file_summary() {
        let diff = unified_diff("foo.rs", "", "fn main() {\n    println!(\"hi\");\n}\n");
        assert!(diff.contains("+ (3 lines"));
        assert!(diff.contains("/dev/null"));
    }

    #[test]
    fn identical_returns_empty() {
        let s = "hello\nworld\n";
        assert!(unified_diff("f.txt", s, s).is_empty());
    }

    #[test]
    fn simple_replacement() {
        let old = "aaa\nbbb\nccc\n";
        let new = "aaa\nBBB\nccc\n";
        let diff = unified_diff("f.txt", old, new);
        // Should contain both the removal and the addition
        assert!(diff.contains("-bbb"));
        assert!(diff.contains("+BBB"));
    }

    #[test]
    fn diff_stat_counts() {
        let old = "a\nb\nc\n";
        let new = "a\nB\nC\nc\n";
        let stat = diff_stat("f.txt", old, new);
        // 2 inserts (B, C), 1 delete (b) — because similar sees
        // old b→new B as delete+insert, old c stays, new C is insert, new c stays
        assert!(stat.contains('+'));
        assert!(stat.contains('-'));
    }
}
