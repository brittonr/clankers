//! In-process diff viewer — computes and renders diffs using `similar`.
//!
//! No external CLI calls. Compares a snapshotted "original" (captured on
//! first read) against the current file on disk, producing a scrollable
//! colored unified diff inside the panel.

use similar::ChangeTag;
use similar::TextDiff;

use super::prelude::*;
use super::scroll::FreeScroll;

// ── Diff line classification ────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLineKind {
    /// File header (synthetic `--- a/` / `+++ b/` lines)
    FileHeader,
    /// Hunk header (`@@ … @@`)
    HunkHeader,
    /// Added line
    Added,
    /// Removed line
    Removed,
    /// Context (unchanged) line
    Context,
    /// Informational (e.g. "new file", "file deleted")
    Info,
}

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub text: String,
}

// ── Diff view state ─────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct DiffView {
    /// The file path this diff is for (display-ready, relative)
    pub file_path: String,
    /// Parsed diff lines
    pub lines: Vec<DiffLine>,
    /// Scroll state (interior-mutable for draw)
    pub scroll: FreeScroll,
    /// Stats: lines added
    pub additions: usize,
    /// Stats: lines removed
    pub deletions: usize,
    /// Whether the diff is empty (no changes)
    pub empty: bool,
}

impl DiffView {
    /// Compute a diff between `original` content and the current file on disk.
    ///
    /// - `original`: the file content captured on first read (or `None` for newly created files).
    /// - `file_path`: absolute or CWD-relative path to the file.
    pub fn compute(file_path: &str, original: Option<&str>) -> Self {
        let current = std::fs::read_to_string(file_path).ok();

        match (original, current.as_deref()) {
            // Both exist — normal diff
            (Some(old), Some(new)) => Self::diff_texts(file_path, old, new),
            // No original (agent created this file) — show all as added
            (None, Some(new)) => Self::new_file(file_path, new),
            // File was deleted since snapshot — show all as removed
            (Some(old), None) => Self::deleted_file(file_path, old),
            // No original, file gone — nothing to show
            (None, None) => Self::empty_diff(file_path),
        }
    }

    /// Diff two strings using `similar`, producing classified lines.
    fn diff_texts(file_path: &str, old: &str, new: &str) -> Self {
        if old == new {
            return Self::empty_diff(file_path);
        }

        let diff = TextDiff::from_lines(old, new);
        let mut lines = Vec::new();
        let mut additions = 0usize;
        let mut deletions = 0usize;

        // Synthetic file header
        lines.push(DiffLine {
            kind: DiffLineKind::FileHeader,
            text: format!("--- a/{file_path}"),
        });
        lines.push(DiffLine {
            kind: DiffLineKind::FileHeader,
            text: format!("+++ b/{file_path}"),
        });

        for group in diff.grouped_ops(3) {
            // Hunk header
            let (Some(first), Some(last)) = (group.first(), group.last()) else {
                continue;
            };
            let old_start = first.old_range().start + 1;
            let old_len = last.old_range().end - first.old_range().start;
            let new_start = first.new_range().start + 1;
            let new_len = last.new_range().end - first.new_range().start;
            lines.push(DiffLine {
                kind: DiffLineKind::HunkHeader,
                text: format!("@@ -{old_start},{old_len} +{new_start},{new_len} @@"),
            });

            for op in &group {
                for change in diff.iter_changes(op) {
                    let (kind, prefix) = match change.tag() {
                        ChangeTag::Equal => (DiffLineKind::Context, ' '),
                        ChangeTag::Insert => {
                            additions += 1;
                            (DiffLineKind::Added, '+')
                        }
                        ChangeTag::Delete => {
                            deletions += 1;
                            (DiffLineKind::Removed, '-')
                        }
                    };
                    let value = change.as_str().unwrap_or("");
                    let text = format!("{prefix}{}", value.trim_end_matches('\n'));
                    lines.push(DiffLine { kind, text });
                }
            }
        }

        Self {
            file_path: file_path.to_string(),
            lines,
            scroll: FreeScroll::new(),
            additions,
            deletions,
            empty: false,
        }
    }

    /// Entire file is new (no original snapshot).
    fn new_file(file_path: &str, content: &str) -> Self {
        let mut lines = vec![
            DiffLine {
                kind: DiffLineKind::Info,
                text: "new file".to_string(),
            },
            DiffLine {
                kind: DiffLineKind::FileHeader,
                text: format!("+++ b/{file_path}"),
            },
        ];
        let content_lines: Vec<&str> = content.lines().collect();
        let additions = content_lines.len();
        lines.push(DiffLine {
            kind: DiffLineKind::HunkHeader,
            text: format!("@@ -0,0 +1,{additions} @@"),
        });
        for l in &content_lines {
            lines.push(DiffLine {
                kind: DiffLineKind::Added,
                text: format!("+{l}"),
            });
        }
        Self {
            file_path: file_path.to_string(),
            lines,
            scroll: FreeScroll::new(),
            additions,
            deletions: 0,
            empty: false,
        }
    }

    /// File existed at snapshot time but has since been deleted.
    fn deleted_file(file_path: &str, content: &str) -> Self {
        let mut lines = vec![
            DiffLine {
                kind: DiffLineKind::Info,
                text: "deleted file".to_string(),
            },
            DiffLine {
                kind: DiffLineKind::FileHeader,
                text: format!("--- a/{file_path}"),
            },
        ];
        let content_lines: Vec<&str> = content.lines().collect();
        let deletions = content_lines.len();
        lines.push(DiffLine {
            kind: DiffLineKind::HunkHeader,
            text: format!("@@ -1,{deletions} +0,0 @@"),
        });
        for l in &content_lines {
            lines.push(DiffLine {
                kind: DiffLineKind::Removed,
                text: format!("-{l}"),
            });
        }
        Self {
            file_path: file_path.to_string(),
            lines,
            scroll: FreeScroll::new(),
            additions: 0,
            deletions,
            empty: false,
        }
    }

    fn empty_diff(file_path: &str) -> Self {
        Self {
            file_path: file_path.to_string(),
            lines: Vec::new(),
            scroll: FreeScroll::new(),
            additions: 0,
            deletions: 0,
            empty: true,
        }
    }

    /// Title for the diff view header.
    pub fn title(&self) -> String {
        if self.empty {
            format!(" {} — no changes ", self.file_path)
        } else {
            format!(" {} — +{} −{} ", self.file_path, self.additions, self.deletions)
        }
    }

    /// Render the diff content into `area`.
    pub fn draw(&self, frame: &mut Frame, area: Rect, theme: &Theme) {
        if area.height == 0 || area.width == 0 {
            return;
        }

        if self.empty {
            let msg = Paragraph::new(Line::from(Span::styled(
                "No changes from original snapshot.",
                Style::default().fg(Color::DarkGray),
            )));
            frame.render_widget(msg, area);
            return;
        }

        let rendered: Vec<Line> = self
            .lines
            .iter()
            .map(|dl| {
                let style = match dl.kind {
                    DiffLineKind::FileHeader => Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                    DiffLineKind::HunkHeader => Style::default().fg(Color::Cyan).add_modifier(Modifier::DIM),
                    DiffLineKind::Added => Style::default().fg(Color::Green),
                    DiffLineKind::Removed => Style::default().fg(Color::Red),
                    DiffLineKind::Context => Style::default().fg(theme.fg),
                    DiffLineKind::Info => Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC),
                };
                Line::from(Span::styled(&dl.text, style))
            })
            .collect();

        let total = rendered.len() as u16;
        let max_scroll = total.saturating_sub(area.height);
        let scroll = self.scroll.clamp(max_scroll);

        let para = Paragraph::new(rendered).scroll((scroll, 0)).wrap(Wrap { trim: false });
        frame.render_widget(para, area);
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_identical() {
        let view = DiffView::compute("/dev/null", Some(""));
        // /dev/null reads as empty, original is empty — identical
        assert!(view.empty);
    }

    #[test]
    fn test_diff_texts_additions() {
        let old = "line1\nline2\n";
        let new = "line1\ninserted\nline2\n";
        let view = DiffView::diff_texts("test.rs", old, new);
        assert_eq!(view.additions, 1);
        assert_eq!(view.deletions, 0);
        assert!(!view.empty);
        // Should have hunk header
        assert!(view.lines.iter().any(|l| l.kind == DiffLineKind::HunkHeader));
        // Should have the added line
        assert!(view.lines.iter().any(|l| l.kind == DiffLineKind::Added && l.text.contains("inserted")));
    }

    #[test]
    fn test_diff_texts_deletions() {
        let old = "aaa\nbbb\nccc\n";
        let new = "aaa\nccc\n";
        let view = DiffView::diff_texts("test.rs", old, new);
        assert_eq!(view.additions, 0);
        assert_eq!(view.deletions, 1);
        assert!(view.lines.iter().any(|l| l.kind == DiffLineKind::Removed && l.text.contains("bbb")));
    }

    #[test]
    fn test_diff_texts_modifications() {
        let old = "hello\nworld\n";
        let new = "hello\nearth\n";
        let view = DiffView::diff_texts("test.rs", old, new);
        assert_eq!(view.additions, 1); // "earth"
        assert_eq!(view.deletions, 1); // "world"
    }

    #[test]
    fn test_new_file() {
        let view = DiffView::new_file("new.rs", "fn main() {}\n");
        assert_eq!(view.additions, 1);
        assert_eq!(view.deletions, 0);
        assert!(!view.empty);
        assert!(view.lines.iter().any(|l| l.kind == DiffLineKind::Info && l.text.contains("new file")));
        assert!(view.lines.iter().any(|l| l.kind == DiffLineKind::Added && l.text.contains("fn main")));
    }

    #[test]
    fn test_deleted_file() {
        let view = DiffView::deleted_file("old.rs", "goodbye\n");
        assert_eq!(view.additions, 0);
        assert_eq!(view.deletions, 1);
        assert!(view.lines.iter().any(|l| l.kind == DiffLineKind::Info && l.text.contains("deleted")));
    }

    #[test]
    fn test_empty_diff() {
        let view = DiffView::empty_diff("nope.rs");
        assert!(view.empty);
        assert_eq!(view.lines.len(), 0);
    }

    #[test]
    fn test_scroll_operations() {
        let view = DiffView::diff_texts("f.rs", "a\n", "b\n");
        assert_eq!(view.scroll.get(), 0);
        view.scroll.scroll_down(5);
        assert_eq!(view.scroll.get(), 5);
        view.scroll.scroll_up(2);
        assert_eq!(view.scroll.get(), 3);
        view.scroll.scroll_to_top();
        assert_eq!(view.scroll.get(), 0);
        view.scroll.scroll_to_bottom();
        assert_eq!(view.scroll.get(), u16::MAX);
    }

    #[test]
    fn test_title_changes() {
        let view = DiffView::diff_texts("f.rs", "a\n", "b\nc\n");
        assert!(view.title().contains("+"));
        assert!(view.title().contains("−"));
    }

    #[test]
    fn test_title_no_changes() {
        let view = DiffView::empty_diff("f.rs");
        assert!(view.title().contains("no changes"));
    }

    #[test]
    fn test_compute_no_original_file_gone() {
        // No original, file doesn't exist on disk
        let view = DiffView::compute("/tmp/__nonexistent_test_file__", None);
        assert!(view.empty);
    }

    #[test]
    fn test_compute_with_real_file() {
        // Write a temp file, compute diff from original
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.txt");
        std::fs::write(&path, "modified\n").unwrap();
        let view = DiffView::compute(path.to_str().unwrap(), Some("original\n"));
        assert_eq!(view.additions, 1);
        assert_eq!(view.deletions, 1);
        assert!(!view.empty);
    }

    #[test]
    fn test_compute_new_file_on_disk() {
        // No original, file exists on disk — treated as new file
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("new.txt");
        std::fs::write(&path, "brand new\ncontent\n").unwrap();
        let view = DiffView::compute(path.to_str().unwrap(), None);
        assert_eq!(view.additions, 2);
        assert!(!view.empty);
    }

    #[test]
    fn test_file_header_lines() {
        let view = DiffView::diff_texts("src/main.rs", "old\n", "new\n");
        let headers: Vec<_> = view.lines.iter().filter(|l| l.kind == DiffLineKind::FileHeader).collect();
        assert_eq!(headers.len(), 2);
        assert!(headers[0].text.starts_with("--- a/"));
        assert!(headers[1].text.starts_with("+++ b/"));
    }

    #[test]
    fn test_multiline_diff_context() {
        let old = "a\nb\nc\nd\ne\nf\ng\nh\n";
        let new = "a\nb\nc\nX\ne\nf\ng\nh\n";
        let view = DiffView::diff_texts("ctx.rs", old, new);
        // Should have context lines around the change
        let context_count = view.lines.iter().filter(|l| l.kind == DiffLineKind::Context).count();
        assert!(context_count > 0);
        assert_eq!(view.additions, 1);
        assert_eq!(view.deletions, 1);
    }
}
