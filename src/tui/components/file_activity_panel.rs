//! File activity panel — tracks files read/edited/written during the session
//!
//! Populated by intercepting tool results for read, edit, write, and bash tools.
//! Shows a list of recently touched files with operation type and count.
//! Implements [`Panel`] for unified layout, key handling, and rendering.

use std::collections::HashMap;
use std::time::Instant;

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;

use crate::tui::panel::DrawContext;
use crate::tui::panel::ListNav;
use crate::tui::panel::Panel;
use crate::tui::panel::PanelAction;
use crate::tui::panel::PanelId;

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
}

impl FileActivityPanel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a file operation
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
            self.files.insert(path.clone(), FileEntry {
                path: path.clone(),
                last_op: op,
                op_count: 1,
                last_touched: now,
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

    fn title(&self) -> String {
        let count = self.file_count();
        let ops = self.total_ops();
        format!("Files ({}, {} ops)", count, ops)
    }

    fn focus_hints(&self) -> &'static str {
        " j/k Tab "
    }

    fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    fn empty_text(&self) -> &'static str {
        "No files touched yet."
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> Option<PanelAction> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.nav.next(self.order.len());
                Some(PanelAction::Consumed)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.nav.prev(self.order.len());
                Some(PanelAction::Consumed)
            }
            KeyCode::Esc => Some(PanelAction::Unfocus),
            _ => None,
        }
    }

    fn handle_scroll(&mut self, up: bool, lines: u16) {
        let len = self.order.len();
        for _ in 0..lines {
            if up {
                self.nav.prev(len);
            } else {
                self.nav.next(len);
            }
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
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
    }
}

// ── Legacy render function (bridge to Panel trait) ──────────────────────────

pub fn render_file_activity_panel(
    frame: &mut Frame,
    panel: &FileActivityPanel,
    _cwd: &str, // now stored on the panel itself
    theme: &crate::tui::theme::Theme,
    area: Rect,
    focused: bool,
) {
    use crate::tui::panel::draw_panel;
    let ctx = DrawContext { theme, focused };
    draw_panel(frame, panel, area, &ctx);
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
        let entry = panel.files.get("src/main.rs").unwrap();
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
}
