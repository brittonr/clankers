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

use crate::provider::message::AgentMessage;
use crate::provider::message::Content;
use crate::provider::message::MessageId;
use crate::session::entry::MessageEntry;

/// A single item in the interactive merge list.
#[derive(Debug, Clone)]
pub struct MergeItem {
    pub id: MessageId,
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
    source_leaf: Option<MessageId>,
    target_leaf: Option<MessageId>,
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
        source_leaf: MessageId,
        target_leaf: MessageId,
        source_name: &str,
        target_name: &str,
        unique_messages: &[&MessageEntry],
    ) {
        self.items = unique_messages
            .iter()
            .map(|entry| {
                let (preview, variant) = message_preview(&entry.message, 70);
                MergeItem {
                    id: entry.id.clone(),
                    label: preview,
                    variant_label: variant,
                    selected: true,
                }
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

    pub fn move_down(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.cursor = (self.cursor + 1) % self.items.len();
        self.adjust_scroll();
    }

    /// Return the MessageIds of all selected items.
    pub fn selected_ids(&self) -> Vec<MessageId> {
        self.items
            .iter()
            .filter(|i| i.selected)
            .map(|i| i.id.clone())
            .collect()
    }

    pub fn selected_count(&self) -> usize {
        self.items.iter().filter(|i| i.selected).count()
    }

    pub fn source_leaf(&self) -> Option<&MessageId> {
        self.source_leaf.as_ref()
    }

    pub fn target_leaf(&self) -> Option<&MessageId> {
        self.target_leaf.as_ref()
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
            .title(Span::styled(
                title,
                Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            ))
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
                Span::styled(
                    format!("{:<9} ", item.variant_label),
                    bg.fg(variant_color),
                ),
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
                format!(
                    " {} of {} messages selected",
                    self.selected_count(),
                    self.items.len(),
                ),
                Style::default().fg(Color::DarkGray),
            )),
        ];
        frame.render_widget(Paragraph::new(footer), footer_area);
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Extract a preview string and variant label from an AgentMessage.
fn message_preview(msg: &AgentMessage, max_len: usize) -> (String, &'static str) {
    match msg {
        AgentMessage::User(m) => {
            let text = content_text(&m.content);
            (truncate_preview(&text, max_len), "User")
        }
        AgentMessage::Assistant(m) => {
            let text = content_text(&m.content);
            (truncate_preview(&text, max_len), "Assistant")
        }
        AgentMessage::ToolResult(m) => {
            let text = content_text(&m.content);
            let preview = if text.is_empty() {
                format!("[{}]", m.tool_name)
            } else {
                format!("[{}] {}", m.tool_name, text)
            };
            (truncate_preview(&preview, max_len), "Tool")
        }
        AgentMessage::BashExecution(m) => {
            (truncate_preview(&format!("$ {}", m.command), max_len), "Bash")
        }
        AgentMessage::Custom(m) => {
            (truncate_preview(&format!("[{}]", m.kind), max_len), "Custom")
        }
        AgentMessage::BranchSummary(m) => {
            (truncate_preview(&m.summary, max_len), "Branch")
        }
        AgentMessage::CompactionSummary(m) => {
            (truncate_preview(&m.summary, max_len), "Compact")
        }
    }
}

/// Extract text from Content blocks.
fn content_text(content: &[Content]) -> String {
    content
        .iter()
        .filter_map(|c| match c {
            Content::Text { text } => Some(text.as_str()),
            Content::Thinking { thinking } => Some(thinking.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Truncate to first line, then max chars.
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
    use super::*;
    use chrono::Utc;
    use crate::provider::message::UserMessage;

    fn make_entry(id: &str, text: &str) -> MessageEntry {
        MessageEntry {
            id: MessageId::new(id),
            parent_id: None,
            message: AgentMessage::User(UserMessage {
                id: MessageId::new(id),
                content: vec![Content::Text { text: text.to_string() }],
                timestamp: Utc::now(),
            }),
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn open_populates_items_all_selected() {
        let entries = vec![make_entry("a", "hello"), make_entry("b", "world")];
        let refs: Vec<&MessageEntry> = entries.iter().collect();
        let mut view = MergeInteractiveView::new();
        view.open(
            MessageId::new("src"),
            MessageId::new("tgt"),
            "source-branch",
            "target-branch",
            &refs,
        );
        assert!(view.visible);
        assert_eq!(view.items.len(), 2);
        assert!(view.items.iter().all(|i| i.selected));
        assert_eq!(view.selected_count(), 2);
    }

    #[test]
    fn toggle_changes_selection() {
        let entries = vec![make_entry("a", "hello")];
        let refs: Vec<&MessageEntry> = entries.iter().collect();
        let mut view = MergeInteractiveView::new();
        view.open(MessageId::new("s"), MessageId::new("t"), "s", "t", &refs);

        assert!(view.items[0].selected);
        view.toggle();
        assert!(!view.items[0].selected);
        view.toggle();
        assert!(view.items[0].selected);
    }

    #[test]
    fn select_all_deselect_all() {
        let entries = vec![make_entry("a", "x"), make_entry("b", "y"), make_entry("c", "z")];
        let refs: Vec<&MessageEntry> = entries.iter().collect();
        let mut view = MergeInteractiveView::new();
        view.open(MessageId::new("s"), MessageId::new("t"), "s", "t", &refs);

        view.deselect_all();
        assert_eq!(view.selected_count(), 0);

        view.select_all();
        assert_eq!(view.selected_count(), 3);
    }

    #[test]
    fn navigation_wraps() {
        let entries = vec![make_entry("a", "x"), make_entry("b", "y"), make_entry("c", "z")];
        let refs: Vec<&MessageEntry> = entries.iter().collect();
        let mut view = MergeInteractiveView::new();
        view.open(MessageId::new("s"), MessageId::new("t"), "s", "t", &refs);

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
        let entries = vec![make_entry("a", "x"), make_entry("b", "y"), make_entry("c", "z")];
        let refs: Vec<&MessageEntry> = entries.iter().collect();
        let mut view = MergeInteractiveView::new();
        view.open(MessageId::new("s"), MessageId::new("t"), "s", "t", &refs);

        // Deselect middle item
        view.move_down(); // cursor on "b"
        view.toggle();

        let ids = view.selected_ids();
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0], MessageId::new("a"));
        assert_eq!(ids[1], MessageId::new("c"));
    }

    #[test]
    fn close_resets_state() {
        let entries = vec![make_entry("a", "x")];
        let refs: Vec<&MessageEntry> = entries.iter().collect();
        let mut view = MergeInteractiveView::new();
        view.open(MessageId::new("s"), MessageId::new("t"), "s", "t", &refs);
        assert!(view.visible);

        view.close();
        assert!(!view.visible);
        assert!(view.items.is_empty());
        assert!(view.source_leaf.is_none());
        assert!(view.target_leaf.is_none());
    }

    #[test]
    fn message_preview_extracts_text() {
        let user_msg = AgentMessage::User(UserMessage {
            id: MessageId::new("u"),
            content: vec![Content::Text { text: "Hello world".to_string() }],
            timestamp: Utc::now(),
        });
        let (preview, variant) = super::message_preview(&user_msg, 50);
        assert_eq!(preview, "Hello world");
        assert_eq!(variant, "User");
    }

    #[test]
    fn truncate_long_preview() {
        let text = "a".repeat(100);
        let result = truncate_preview(&text, 20);
        assert_eq!(result.chars().count(), 21); // 20 chars + "…"
        assert!(result.ends_with('…'));
    }
}
