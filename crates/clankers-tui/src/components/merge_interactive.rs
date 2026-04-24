//! Interactive merge view — checkbox overlay for selective branch merging
//!
//! Shows unique messages from a source branch with toggleable checkboxes,
//! letting the user pick which messages to merge into the target branch.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;

/// A single item in the interactive merge list.
#[derive(Debug, Clone)]
pub struct MergeItem {
    pub id: String,
    pub label: String,
    pub variant_label: &'static str,
    pub selected: bool,
}

/// Overlay for interactive selective merge.
#[derive(Debug)]
pub struct MergeInteractiveView {
    pub visible: bool,
    /// Set to true when the user presses Enter to confirm
    pub confirmed: bool,
    items: Vec<MergeItem>,
    cursor: usize,
    scroll_offset: usize,
    source_leaf: Option<String>,
    target_leaf: Option<String>,
    source_name: String,
    target_name: String,
}

impl Default for MergeInteractiveView {
    fn default() -> Self {
        Self::new()
    }
}

impl MergeInteractiveView {
    pub fn new() -> Self {
        Self {
            visible: false,
            confirmed: false,
            items: Vec::new(),
            cursor: 0,
            scroll_offset: 0,
            source_leaf: None,
            target_leaf: None,
            source_name: String::new(),
            target_name: String::new(),
        }
    }

    /// Open the interactive merge view with unique messages from the source branch.
    /// All messages are selected by default.
    pub fn open(
        &mut self,
        source_leaf: String,
        target_leaf: String,
        source_name: &str,
        target_name: &str,
        messages: &[clanker_tui_types::MergeMessageView],
    ) {
        self.items = messages
            .iter()
            .map(|m| MergeItem {
                id: m.id.clone(),
                label: m.preview.clone(),
                variant_label: m.variant_label,
                selected: true,
            })
            .collect();
        self.source_leaf = Some(source_leaf);
        self.target_leaf = Some(target_leaf);
        self.source_name = source_name.to_string();
        self.target_name = target_name.to_string();
        self.cursor = 0;
        self.scroll_offset = 0;
        self.confirmed = false;
        self.visible = true;
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.confirmed = false;
        self.items.clear();
        self.source_leaf = None;
        self.target_leaf = None;
    }

    /// Toggle the selected state of the current item.
    pub fn toggle(&mut self) {
        if let Some(item) = self.items.get_mut(self.cursor) {
            item.selected = !item.selected;
        }
    }

    pub fn select_all(&mut self) {
        for item in &mut self.items {
            item.selected = true;
        }
    }

    pub fn deselect_all(&mut self) {
        for item in &mut self.items {
            item.selected = false;
        }
    }

    pub fn move_up(&mut self) {
        if self.items.is_empty() {
            return;
        }
        if self.cursor == 0 {
            self.cursor = self.items.len() - 1;
        } else {
            self.cursor -= 1;
        }
        self.adjust_scroll();
    }

    #[cfg_attr(dylint_lib = "tigerstyle", allow(unchecked_division, reason = "divisor guarded by is_empty/non-zero check or TUI layout constraint"))]
    pub fn move_down(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.cursor = (self.cursor + 1) % self.items.len();
        self.adjust_scroll();
    }

    /// Return the IDs of all selected items.
    pub fn selected_ids(&self) -> Vec<String> {
        self.items.iter().filter(|i| i.selected).map(|i| i.id.clone()).collect()
    }

    pub fn selected_count(&self) -> usize {
        self.items.iter().filter(|i| i.selected).count()
    }

    pub fn source_leaf(&self) -> Option<&str> {
        self.source_leaf.as_deref()
    }

    pub fn target_leaf(&self) -> Option<&str> {
        self.target_leaf.as_deref()
    }

    fn adjust_scroll(&mut self) {
        // Keep 2 lines padding if possible
        if self.cursor < self.scroll_offset {
            self.scroll_offset = self.cursor;
        }
        // We'll use visible_height in render; here just ensure cursor >= scroll_offset
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if !self.visible || self.items.is_empty() {
            return;
        }

        // Size: 70% width, 60% height, centered
        let width = (area.width * 70 / 100).max(40).min(area.width.saturating_sub(4));
        let height = (area.height * 60 / 100).max(10).min(area.height.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let popup_area = Rect::new(x, y, width, height);

        frame.render_widget(Clear, popup_area);

        let title = format!(
            " Merge: {} → {} ({}/{} selected) ",
            self.source_name,
            self.target_name,
            self.selected_count(),
            self.items.len(),
        );

        let outer = Block::default()
            .title(Span::styled(title, Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Magenta));

        let inner = outer.inner(popup_area);
        frame.render_widget(outer, popup_area);

        if inner.height < 3 || inner.width < 10 {
            return;
        }

        // Reserve 2 lines for footer
        let list_height = (inner.height as usize).saturating_sub(2);

        // Adjust scroll to keep cursor visible within list_height
        let scroll = if self.cursor >= self.scroll_offset + list_height {
            self.cursor - list_height + 1
        } else {
            self.scroll_offset
        };

        let mut lines: Vec<Line<'_>> = Vec::new();

        for (i, item) in self.items.iter().enumerate().skip(scroll).take(list_height) {
            let is_cursor = i == self.cursor;
            let checkbox = if item.selected { "[x]" } else { "[ ]" };

            let variant_color = match item.variant_label {
                "User" => Color::Green,
                "Assistant" => Color::Cyan,
                "Tool" => Color::Yellow,
                "Bash" => Color::Red,
                _ => Color::Gray,
            };

            let cursor_indicator = if is_cursor { "▸ " } else { "  " };
            let bg = if is_cursor {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };

            lines.push(Line::from(vec![
                Span::styled(cursor_indicator, bg.fg(Color::Magenta)),
                Span::styled(
                    format!("{} ", checkbox),
                    bg.fg(if item.selected { Color::Green } else { Color::DarkGray }),
                ),
                Span::styled(format!("{:<9} ", item.variant_label), bg.fg(variant_color)),
                Span::styled(&item.label, bg.fg(Color::White)),
            ]));
        }

        // Render the list
        let list_area = Rect::new(inner.x, inner.y, inner.width, list_height as u16);
        frame.render_widget(Paragraph::new(lines), list_area);

        // Footer
        let footer_y = inner.y + inner.height - 2;
        let footer_area = Rect::new(inner.x, footer_y, inner.width, 2);
        let footer = vec![
            Line::from(vec![
                Span::styled("Space", Style::default().fg(Color::Cyan)),
                Span::raw(": toggle  "),
                Span::styled("a", Style::default().fg(Color::Cyan)),
                Span::raw(": all  "),
                Span::styled("n", Style::default().fg(Color::Cyan)),
                Span::raw(": none  "),
                Span::styled("Enter", Style::default().fg(Color::Cyan)),
                Span::raw(": merge  "),
                Span::styled("Esc", Style::default().fg(Color::Cyan)),
                Span::raw(": cancel"),
            ]),
            Line::from(Span::styled(
                format!(" {} of {} messages selected", self.selected_count(), self.items.len(),),
                Style::default().fg(Color::DarkGray),
            )),
        ];
        frame.render_widget(Paragraph::new(footer), footer_area);
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Truncate to first line, then max chars.
#[cfg(test)]
fn truncate_preview(text: &str, max: usize) -> String {
    let first_line = text.lines().next().unwrap_or(text);
    let trimmed = first_line.trim();
    if trimmed.chars().count() > max {
        let preview: String = trimmed.chars().take(max).collect();
        format!("{}…", preview)
    } else {
        trimmed.to_string()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use clanker_tui_types::MergeMessageView;

    use super::*;

    fn make_view(id: &str, text: &str) -> MergeMessageView {
        MergeMessageView {
            id: id.to_string(),
            preview: text.to_string(),
            variant_label: "User",
        }
    }

    #[test]
    fn open_populates_items_all_selected() {
        let views = vec![make_view("a", "hello"), make_view("b", "world")];
        let mut view = MergeInteractiveView::new();
        view.open("src".into(), "tgt".into(), "source-branch", "target-branch", &views);
        assert!(view.visible);
        assert_eq!(view.items.len(), 2);
        assert!(view.items.iter().all(|i| i.selected));
        assert_eq!(view.selected_count(), 2);
    }

    #[test]
    fn toggle_changes_selection() {
        let views = vec![make_view("a", "hello")];
        let mut view = MergeInteractiveView::new();
        view.open("s".into(), "t".into(), "s", "t", &views);

        assert!(view.items[0].selected);
        view.toggle();
        assert!(!view.items[0].selected);
        view.toggle();
        assert!(view.items[0].selected);
    }

    #[test]
    fn select_all_deselect_all() {
        let views = vec![make_view("a", "x"), make_view("b", "y"), make_view("c", "z")];
        let mut view = MergeInteractiveView::new();
        view.open("s".into(), "t".into(), "s", "t", &views);

        view.deselect_all();
        assert_eq!(view.selected_count(), 0);

        view.select_all();
        assert_eq!(view.selected_count(), 3);
    }

    #[test]
    fn navigation_wraps() {
        let views = vec![make_view("a", "x"), make_view("b", "y"), make_view("c", "z")];
        let mut view = MergeInteractiveView::new();
        view.open("s".into(), "t".into(), "s", "t", &views);

        assert_eq!(view.cursor, 0);
        view.move_down();
        assert_eq!(view.cursor, 1);
        view.move_down();
        assert_eq!(view.cursor, 2);
        view.move_down(); // wraps
        assert_eq!(view.cursor, 0);

        view.move_up(); // wraps
        assert_eq!(view.cursor, 2);
    }

    #[test]
    fn selected_ids_returns_only_selected() {
        let views = vec![make_view("a", "x"), make_view("b", "y"), make_view("c", "z")];
        let mut view = MergeInteractiveView::new();
        view.open("s".into(), "t".into(), "s", "t", &views);

        // Deselect middle item
        view.move_down(); // cursor on "b"
        view.toggle();

        let ids = view.selected_ids();
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0], "a");
        assert_eq!(ids[1], "c");
    }

    #[test]
    fn close_resets_state() {
        let views = vec![make_view("a", "x")];
        let mut view = MergeInteractiveView::new();
        view.open("s".into(), "t".into(), "s", "t", &views);
        assert!(view.visible);

        view.close();
        assert!(!view.visible);
        assert!(view.items.is_empty());
        assert!(view.source_leaf.is_none());
        assert!(view.target_leaf.is_none());
    }

    #[test]
    fn truncate_long_preview() {
        let text = "a".repeat(100);
        let result = truncate_preview(&text, 20);
        assert_eq!(result.chars().count(), 21); // 20 chars + "…"
        assert!(result.ends_with('…'));
    }
}
