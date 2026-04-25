//! File activity panel — tracks files read/edited/written during the session
//!
//! Populated by intercepting tool results for read, edit, write, and bash tools.
//! Shows a list of recently touched files with operation type and count.
//! Implements [`Panel`] for unified layout, key handling, and rendering.

use std::collections::HashMap;
use std::time::Instant;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;

use super::prelude::*;
use crate::components::diff_view::DiffView;
use crate::panel::ListNav;

// ── Data types ──────────────────────────────────────────────────────────────

/// Type of file operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileOp {
    Read,
    Edit,
    Write,
    Create,
}

impl FileOp {
    pub fn icon(&self) -> &'static str {
        match self {
            FileOp::Read => "👁",
            FileOp::Edit => "✎",
            FileOp::Write => "💾",
            FileOp::Create => "✚",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            FileOp::Read => Color::DarkGray,
            FileOp::Edit => Color::Yellow,
            FileOp::Write => Color::Green,
            FileOp::Create => Color::Cyan,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            FileOp::Read => "read",
            FileOp::Edit => "edit",
            FileOp::Write => "write",
            FileOp::Create => "create",
        }
    }
}

/// A tracked file entry
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Display path
    pub path: String,
    /// Last operation performed
    pub last_op: FileOp,
    /// Total number of operations on this file
    pub op_count: usize,
    /// When the file was last touched
    pub last_touched: Instant,
    /// Snapshot of the file content captured on first read.
    /// `None` for files created by the agent (no prior content).
    pub original_content: Option<String>,
}

// ── View mode ───────────────────────────────────────────────────────────────

/// Which view the file activity panel is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileView {
    /// The file list (default)
    #[default]
    List,
    /// Viewing an in-process diff for the selected file
    Diff,
}

// ── Panel state ─────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct FileActivityPanel {
    /// Files keyed by their path
    pub files: HashMap<String, FileEntry>,
    /// Ordered list of file paths (most recent last)
    pub order: Vec<String>,
    /// Selection / scroll state
    pub nav: ListNav,
    /// CWD for path display (set by the app)
    pub cwd: String,
    /// Current view mode
    pub view: FileView,
    /// Active diff view (populated when `view == FileView::Diff`)
    pub diff_view: Option<DiffView>,
}

impl FileActivityPanel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a file operation.
    ///
    /// On the first encounter of a file (typically a Read), the current
    /// content is snapshotted so that later diffs show exactly what the
    /// agent changed.  For Create operations the snapshot is `None`.
    pub fn record(&mut self, path: String, op: FileOp) {
        let now = Instant::now();
        if let Some(entry) = self.files.get_mut(&path) {
            entry.last_op = op;
            entry.op_count += 1;
            entry.last_touched = now;
            // Move to end of order list
            self.order.retain(|p| p != &path);
            self.order.push(path);
        } else {
            // First time seeing this file — snapshot current content
            let original_content = if op == FileOp::Create {
                // Agent is creating this file; no prior content
                None
            } else {
                // Read the file as it exists *right now* (before the agent
                // modifies it). For Read ops this is the content being read;
                // for Edit/Write ops that arrive without a prior Read it's
                // the content just before the first mutation.
                std::fs::read_to_string(&path).ok()
            };
            self.files.insert(path.clone(), FileEntry {
                path: path.clone(),
                last_op: op,
                op_count: 1,
                last_touched: now,
                original_content,
            });
            self.order.push(path);
        }
    }

    /// Shorten a path relative to cwd for display
    pub fn display_path<'a>(path: &'a str, cwd: &str) -> &'a str {
        path.strip_prefix(cwd).map(|p| p.strip_prefix('/').unwrap_or(p)).unwrap_or(path)
    }

    /// Total number of files tracked
    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    /// Total number of operations
    pub fn total_ops(&self) -> usize {
        self.files.values().map(|e| e.op_count).sum()
    }

    pub fn select_next(&mut self) {
        self.nav.next(self.order.len());
    }

    pub fn select_prev(&mut self) {
        self.nav.prev(self.order.len());
    }

    /// Clear all tracked files
    pub fn clear(&mut self) {
        self.files.clear();
        self.order.clear();
        self.nav.selected = 0;
    }

    /// Open the diff view for the currently selected file.
    ///
    /// Uses `similar` to diff the snapshotted original against the file's
    /// current content on disk. No external processes.
    pub fn open_diff(&mut self) {
        if self.order.is_empty() {
            return;
        }
        // Display order is reversed (most recent first), so map selected → path
        let display_idx = self.nav.selected;
        let actual_idx = self.order.len().saturating_sub(1).saturating_sub(display_idx);
        if let Some(path) = self.order.get(actual_idx).cloned() {
            let original = self.files.get(&path).and_then(|e| e.original_content.as_deref());
            let diff = DiffView::compute(&path, original);
            self.diff_view = Some(diff);
            self.view = FileView::Diff;
        }
    }

    /// Close the diff view and return to the file list.
    pub fn close_diff(&mut self) {
        self.view = FileView::List;
        self.diff_view = None;
    }

    /// Get a summary string
    pub fn summary(&self) -> String {
        if self.files.is_empty() {
            return "No files touched.".to_string();
        }
        let mut out = format!("{} file(s), {} op(s):\n", self.file_count(), self.total_ops());
        for path in self.order.iter().rev().take(20) {
            if let Some(entry) = self.files.get(path) {
                out.push_str(&format!("  {} {} (×{})\n", entry.last_op.icon(), entry.path, entry.op_count,));
            }
        }
        out
    }
}

// ── Panel trait impl ────────────────────────────────────────────────────────

impl Panel for FileActivityPanel {
    fn id(&self) -> PanelId {
        PanelId::Files
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn close_detail_view(&mut self) {
        self.close_diff();
    }

    fn title(&self) -> String {
        match self.view {
            FileView::List => {
                let count = self.file_count();
                let ops = self.total_ops();
                format!("Files ({count}, {ops} ops)")
            }
            FileView::Diff => {
                if let Some(ref dv) = self.diff_view {
                    let display = Self::display_path(&dv.file_path, &self.cwd);
                    if dv.empty {
                        format!("Diff: {display} — no changes")
                    } else {
                        format!("Diff: {display} +{} −{}", dv.additions, dv.deletions)
                    }
                } else {
                    "Diff".to_string()
                }
            }
        }
    }

    fn focus_hints(&self) -> &'static str {
        match self.view {
            FileView::List => " j/k Enter:diff ",
            FileView::Diff => " j/k Esc:back g/G:top/bot ",
        }
    }

    fn is_empty(&self) -> bool {
        // Diff view handles its own empty state internally
        if self.view == FileView::Diff {
            return false;
        }
        self.files.is_empty()
    }

    fn empty_text(&self) -> &'static str {
        "No files touched yet."
    }

    #[cfg_attr(
        dylint_lib = "tigerstyle",
        allow(catch_all_on_enum, reason = "default handler covers many variants uniformly")
    )]
    fn handle_key_event(&mut self, key: KeyEvent) -> Option<PanelAction> {
        match self.view {
            FileView::List => match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    self.nav.next(self.order.len());
                    Some(PanelAction::Consumed)
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.nav.prev(self.order.len());
                    Some(PanelAction::Consumed)
                }
                KeyCode::Enter | KeyCode::Char('d') => {
                    self.open_diff();
                    Some(PanelAction::Consumed)
                }
                KeyCode::Esc => Some(PanelAction::Unfocus),
                _ => None,
            },
            FileView::Diff => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.close_diff();
                    Some(PanelAction::Consumed)
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    if let Some(ref dv) = self.diff_view {
                        dv.scroll.scroll_down(1);
                    }
                    Some(PanelAction::Consumed)
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if let Some(ref dv) = self.diff_view {
                        dv.scroll.scroll_up(1);
                    }
                    Some(PanelAction::Consumed)
                }
                KeyCode::Char('g') => {
                    if let Some(ref dv) = self.diff_view {
                        dv.scroll.scroll_to_top();
                    }
                    Some(PanelAction::Consumed)
                }
                KeyCode::Char('G') => {
                    if let Some(ref dv) = self.diff_view {
                        dv.scroll.scroll_to_bottom();
                    }
                    Some(PanelAction::Consumed)
                }
                _ => None,
            },
        }
    }

    fn handle_scroll(&mut self, up: bool, lines: u16) {
        match self.view {
            FileView::List => {
                let len = self.order.len();
                for _ in 0..lines {
                    if up {
                        self.nav.prev(len);
                    } else {
                        self.nav.next(len);
                    }
                }
            }
            FileView::Diff => {
                if let Some(ref dv) = self.diff_view {
                    if up {
                        dv.scroll.scroll_up(lines);
                    } else {
                        dv.scroll.scroll_down(lines);
                    }
                }
            }
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
        match self.view {
            FileView::List => self.draw_list(frame, area, ctx),
            FileView::Diff => {
                if let Some(ref dv) = self.diff_view {
                    dv.draw(frame, area, ctx.theme);
                }
            }
        }
    }
}

// ── Private rendering helpers ───────────────────────────────────────────────

impl FileActivityPanel {
    fn draw_list(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
        // Show files in reverse order (most recent first)
        let mut lines = Vec::new();
        let display_order: Vec<&String> = self.order.iter().rev().collect();
        for (i, path) in display_order.iter().enumerate() {
            if let Some(entry) = self.files.get(*path) {
                let actual_idx = self.order.len() - 1 - i;
                let is_selected = actual_idx == self.nav.selected && ctx.focused;
                let display = Self::display_path(&entry.path, &self.cwd);

                let prefix = if is_selected { "▸ " } else { "  " };
                let mut spans = vec![
                    Span::styled(prefix, Style::default().fg(if is_selected { Color::Cyan } else { Color::DarkGray })),
                    Span::styled(format!("{} ", entry.last_op.icon()), Style::default().fg(entry.last_op.color())),
                ];

                let text_style = if is_selected {
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(ctx.theme.fg)
                };
                spans.push(Span::styled(display, text_style));

                if entry.op_count > 1 {
                    spans.push(Span::styled(format!(" ×{}", entry.op_count), Style::default().fg(Color::DarkGray)));
                }

                lines.push(Line::from(spans));
            }
        }

        let visible_height = area.height as usize;
        let total_items = self.order.len();
        let selected_display_idx = total_items.saturating_sub(1).saturating_sub(self.nav.selected);
        let scroll = if selected_display_idx >= visible_height {
            (selected_display_idx - visible_height + 1) as u16
        } else {
            0
        };

        let para = Paragraph::new(lines).scroll((scroll, 0)).wrap(Wrap { trim: false });
        frame.render_widget(para, area);
        render_scrollbar(frame, area, total_items, scroll as usize, visible_height);
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_new_file() {
        let mut panel = FileActivityPanel::new();
        panel.record("src/main.rs".to_string(), FileOp::Read);
        assert_eq!(panel.file_count(), 1);
        assert_eq!(panel.total_ops(), 1);
        assert!(!panel.is_empty());
    }

    #[test]
    fn test_record_same_file_increments() {
        let mut panel = FileActivityPanel::new();
        panel.record("src/main.rs".to_string(), FileOp::Read);
        panel.record("src/main.rs".to_string(), FileOp::Edit);
        assert_eq!(panel.file_count(), 1);
        assert_eq!(panel.total_ops(), 2);
        let entry = panel.files.get("src/main.rs").expect("file should exist after recording");
        assert_eq!(entry.last_op, FileOp::Edit);
    }

    #[test]
    fn test_display_path_strips_cwd() {
        assert_eq!(
            FileActivityPanel::display_path("/home/user/project/src/main.rs", "/home/user/project"),
            "src/main.rs"
        );
        assert_eq!(FileActivityPanel::display_path("/other/path/file.rs", "/home/user/project"), "/other/path/file.rs");
    }

    #[test]
    fn test_clear() {
        let mut panel = FileActivityPanel::new();
        panel.record("a.rs".to_string(), FileOp::Read);
        panel.record("b.rs".to_string(), FileOp::Write);
        panel.clear();
        assert!(panel.is_empty());
        assert_eq!(panel.file_count(), 0);
    }

    #[test]
    fn test_navigation() {
        let mut panel = FileActivityPanel::new();
        panel.record("a.rs".to_string(), FileOp::Read);
        panel.record("b.rs".to_string(), FileOp::Read);
        panel.record("c.rs".to_string(), FileOp::Read);
        panel.nav.selected = 0;
        panel.nav.next(panel.order.len());
        assert_eq!(panel.nav.selected, 1);
        panel.nav.prev(panel.order.len());
        assert_eq!(panel.nav.selected, 0);
        panel.nav.prev(panel.order.len()); // wraps
        assert_eq!(panel.nav.selected, 2);
    }

    #[test]
    fn test_order_moves_to_end() {
        let mut panel = FileActivityPanel::new();
        panel.record("a.rs".to_string(), FileOp::Read);
        panel.record("b.rs".to_string(), FileOp::Read);
        panel.record("a.rs".to_string(), FileOp::Edit);
        assert_eq!(panel.order, vec!["b.rs", "a.rs"]);
    }

    #[test]
    fn test_summary_empty() {
        let panel = FileActivityPanel::new();
        assert_eq!(panel.summary(), "No files touched.");
    }

    #[test]
    fn test_summary_with_files() {
        let mut panel = FileActivityPanel::new();
        panel.record("src/main.rs".to_string(), FileOp::Read);
        panel.record("src/lib.rs".to_string(), FileOp::Edit);
        let s = panel.summary();
        assert!(s.contains("2 file(s)"));
        assert!(s.contains("src/main.rs"));
        assert!(s.contains("src/lib.rs"));
    }

    #[test]
    fn test_panel_trait_title() {
        let mut panel = FileActivityPanel::new();
        assert_eq!(panel.title(), "Files (0, 0 ops)");
        panel.record("a.rs".to_string(), FileOp::Read);
        assert_eq!(panel.title(), "Files (1, 1 ops)");
    }

    // ── Snapshot & diff tests ───────────────────────────────────────

    #[test]
    fn test_record_snapshots_on_first_read() {
        let dir = tempfile::tempdir().expect("tempdir creation should succeed");
        let path = dir.path().join("snap.txt");
        std::fs::write(&path, "original content\n").expect("test file write should succeed");

        let mut panel = FileActivityPanel::new();
        let path_str = path.to_str().expect("path should be valid UTF-8").to_string();
        panel.record(path_str.clone(), FileOp::Read);

        let entry = panel.files.get(&path_str).expect("file should exist after recording");
        assert_eq!(entry.original_content.as_deref(), Some("original content\n"));
    }

    #[test]
    fn test_record_create_has_no_snapshot() {
        let mut panel = FileActivityPanel::new();
        panel.record("/tmp/__no_snapshot_test__".to_string(), FileOp::Create);

        let entry = panel.files.get("/tmp/__no_snapshot_test__").expect("file should exist after recording");
        assert!(entry.original_content.is_none());
    }

    #[test]
    fn test_snapshot_only_on_first_encounter() {
        let dir = tempfile::tempdir().expect("tempdir creation should succeed");
        let path = dir.path().join("once.txt");
        std::fs::write(&path, "version 1\n").expect("test file write should succeed");

        let mut panel = FileActivityPanel::new();
        let path_str = path.to_str().expect("path should be valid UTF-8").to_string();
        panel.record(path_str.clone(), FileOp::Read);

        // Mutate the file and record again
        std::fs::write(&path, "version 2\n").expect("test file write should succeed");
        panel.record(path_str.clone(), FileOp::Edit);

        // Original snapshot should still be version 1
        let entry = panel.files.get(&path_str).expect("file should exist after recording");
        assert_eq!(entry.original_content.as_deref(), Some("version 1\n"));
    }

    #[test]
    fn test_view_starts_as_list() {
        let panel = FileActivityPanel::new();
        assert_eq!(panel.view, FileView::List);
        assert!(panel.diff_view.is_none());
    }

    #[test]
    fn test_open_diff_computes_in_process() {
        let dir = tempfile::tempdir().expect("tempdir creation should succeed");
        let path = dir.path().join("diff_me.txt");
        std::fs::write(&path, "line1\nline2\n").expect("test file write should succeed");

        let mut panel = FileActivityPanel::new();
        let path_str = path.to_str().expect("path should be valid UTF-8").to_string();
        panel.record(path_str.clone(), FileOp::Read);

        // Mutate the file
        std::fs::write(&path, "line1\nmodified\nline2\n").expect("test file write should succeed");
        panel.record(path_str, FileOp::Edit);

        panel.open_diff();
        assert_eq!(panel.view, FileView::Diff);
        let dv = panel.diff_view.as_ref().expect("diff_view should be populated after open_diff");
        assert_eq!(dv.additions, 1);
        assert_eq!(dv.deletions, 0);
        assert!(!dv.empty);
    }

    #[test]
    fn test_open_diff_unchanged_file() {
        let dir = tempfile::tempdir().expect("tempdir creation should succeed");
        let path = dir.path().join("unchanged.txt");
        std::fs::write(&path, "same\n").expect("test file write should succeed");

        let mut panel = FileActivityPanel::new();
        let path_str = path.to_str().expect("path should be valid UTF-8").to_string();
        panel.record(path_str, FileOp::Read);
        // File is unchanged — diff should be empty
        panel.open_diff();
        assert_eq!(panel.view, FileView::Diff);
        assert!(panel.diff_view.as_ref().expect("diff_view should be populated after open_diff").empty);
    }

    #[test]
    fn test_close_diff_returns_to_list() {
        let dir = tempfile::tempdir().expect("tempdir creation should succeed");
        let path = dir.path().join("close.txt");
        std::fs::write(&path, "x\n").expect("test file write should succeed");

        let mut panel = FileActivityPanel::new();
        let path_str = path.to_str().expect("path should be valid UTF-8").to_string();
        panel.record(path_str, FileOp::Read);
        panel.open_diff();
        assert_eq!(panel.view, FileView::Diff);
        panel.close_diff();
        assert_eq!(panel.view, FileView::List);
        assert!(panel.diff_view.is_none());
    }

    #[test]
    fn test_open_diff_on_empty_panel_is_noop() {
        let mut panel = FileActivityPanel::new();
        panel.open_diff();
        assert_eq!(panel.view, FileView::List);
        assert!(panel.diff_view.is_none());
    }

    #[test]
    fn test_is_empty_false_in_diff_view() {
        let dir = tempfile::tempdir().expect("tempdir creation should succeed");
        let path = dir.path().join("empty_check.txt");
        std::fs::write(&path, "x\n").expect("test file write should succeed");

        let mut panel = FileActivityPanel::new();
        let path_str = path.to_str().expect("path should be valid UTF-8").to_string();
        panel.record(path_str, FileOp::Read);
        panel.open_diff();
        assert!(!panel.is_empty());
    }

    #[test]
    fn test_diff_title_shows_stats() {
        let dir = tempfile::tempdir().expect("tempdir creation should succeed");
        let path = dir.path().join("titled.txt");
        std::fs::write(&path, "old\n").expect("test file write should succeed");

        let mut panel = FileActivityPanel::new();
        panel.cwd = dir.path().to_str().expect("temp dir path should be valid UTF-8").to_string();
        let path_str = path.to_str().expect("path should be valid UTF-8").to_string();
        panel.record(path_str, FileOp::Read);
        std::fs::write(&path, "new\n").expect("test file write should succeed");
        panel.open_diff();
        let title = panel.title();
        assert!(title.starts_with("Diff:"), "title was: {title}");
        assert!(title.contains("+1"), "title was: {title}");
        assert!(title.contains("−1"), "title was: {title}");
    }

    #[test]
    fn test_focus_hints_change_with_view() {
        let dir = tempfile::tempdir().expect("tempdir creation should succeed");
        let path = dir.path().join("hints.txt");
        std::fs::write(&path, "x\n").expect("test file write should succeed");

        let mut panel = FileActivityPanel::new();
        assert!(panel.focus_hints().contains("Enter:diff"));
        let path_str = path.to_str().expect("path should be valid UTF-8").to_string();
        panel.record(path_str, FileOp::Read);
        panel.open_diff();
        assert!(panel.focus_hints().contains("Esc:back"));
    }

    #[test]
    fn test_diff_for_created_file() {
        let dir = tempfile::tempdir().expect("tempdir creation should succeed");
        let path = dir.path().join("created.txt");
        std::fs::write(&path, "brand new\ncontent\n").expect("test file write should succeed");

        let mut panel = FileActivityPanel::new();
        let path_str = path.to_str().expect("path should be valid UTF-8").to_string();
        panel.record(path_str, FileOp::Create);
        panel.open_diff();

        let dv = panel.diff_view.as_ref().expect("diff_view should be populated after open_diff");
        assert_eq!(dv.additions, 2);
        assert_eq!(dv.deletions, 0);
        assert!(!dv.empty);
    }
}
