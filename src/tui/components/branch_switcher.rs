//! Branch switcher — floating overlay for quick branch switching with fuzzy filter
//!
//! Triggered by a keyboard shortcut, renders as a centered floating popup.
//! Provides type-ahead filtering to quickly find and switch to a branch.

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
use ratatui::widgets::Wrap;

use crate::tui::components::block::ConversationBlock;

/// A branch item for the switcher
#[derive(Debug, Clone)]
pub struct SwitcherItem {
    /// Leaf block ID
    pub leaf_id: usize,
    /// Display name
    pub name: String,
    /// Message count on this branch
    pub message_count: usize,
    /// Last prompt preview
    pub last_prompt: String,
    /// Whether this is the active branch
    pub is_active: bool,
    /// Total tokens
    pub tokens: usize,
}

/// Branch switcher overlay state
#[derive(Debug, Default)]
pub struct BranchSwitcher {
    /// All branches
    items: Vec<SwitcherItem>,
    /// Current filter text
    pub filter: String,
    /// Selected index in the filtered list
    pub selected: usize,
    /// Whether the switcher is visible
    pub visible: bool,
}

impl BranchSwitcher {
    pub fn new() -> Self {
        Self::default()
    }

    /// Open the switcher with branches from the block tree.
    pub fn open(&mut self, all_blocks: &[ConversationBlock], active_block_ids: &std::collections::HashSet<usize>) {
        let has_children: std::collections::HashSet<usize> =
            all_blocks.iter().filter_map(|b| b.parent_block_id).collect();

        self.items = all_blocks
            .iter()
            .filter(|b| !has_children.contains(&b.id))
            .enumerate()
            .map(|(i, leaf)| {
                let path = walk_to_root(leaf.id, all_blocks);
                let tokens: usize =
                    path.iter().filter_map(|&id| all_blocks.iter().find(|b| b.id == id)).map(|b| b.tokens).sum();

                SwitcherItem {
                    leaf_id: leaf.id,
                    name: format!("branch-{}", i + 1),
                    message_count: path.len(),
                    last_prompt: truncate_first_line(&leaf.prompt, 50),
                    is_active: active_block_ids.contains(&leaf.id),
                    tokens,
                }
            })
            .collect();

        // Sort: active first, then most recent
        self.items.sort_by(|a, b| b.is_active.cmp(&a.is_active).then(b.leaf_id.cmp(&a.leaf_id)));

        self.filter.clear();
        self.selected = 0;
        self.visible = true;
    }

    /// Close the switcher
    pub fn close(&mut self) {
        self.visible = false;
        self.filter.clear();
    }

    /// Get filtered items based on current filter text
    pub fn filtered_items(&self) -> Vec<&SwitcherItem> {
        let filter_lower = self.filter.to_lowercase();
        self.items
            .iter()
            .filter(|item| {
                filter_lower.is_empty()
                    || item.name.to_lowercase().contains(&filter_lower)
                    || item.last_prompt.to_lowercase().contains(&filter_lower)
            })
            .collect()
    }

    /// Move selection up
    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// Move selection down
    pub fn move_down(&mut self) {
        let max = self.filtered_items().len().saturating_sub(1);
        self.selected = (self.selected + 1).min(max);
    }

    /// Type a character into the filter
    pub fn type_char(&mut self, c: char) {
        self.filter.push(c);
        self.selected = 0;
    }

    /// Delete the last filter character
    pub fn backspace(&mut self) {
        self.filter.pop();
        self.selected = 0;
    }

    /// Get the selected item's leaf block ID
    pub fn selected_leaf_id(&self) -> Option<usize> {
        let filtered = self.filtered_items();
        filtered.get(self.selected).map(|item| item.leaf_id)
    }

    /// Render the switcher as a floating overlay
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if !self.visible {
            return;
        }

        let width = 60.min(area.width.saturating_sub(4));
        let height = 16.min(area.height.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let popup_area = Rect::new(x, y, width, height);

        frame.render_widget(Clear, popup_area);

        let block = Block::default()
            .title(Span::styled(" Switch Branch ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        if inner.height < 2 || inner.width < 4 {
            return;
        }

        // Filter input line
        let filter_line = Line::from(vec![
            Span::styled(" Filter: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&self.filter, Style::default().fg(Color::White)),
            Span::styled("_", Style::default().fg(Color::White).add_modifier(Modifier::SLOW_BLINK)),
        ]);

        let filter_area = Rect::new(inner.x, inner.y, inner.width, 1);
        frame.render_widget(Paragraph::new(filter_line), filter_area);

        // Branch list
        let list_area = Rect::new(inner.x, inner.y + 1, inner.width, inner.height - 1);
        let filtered = self.filtered_items();

        let mut lines = Vec::new();
        for (i, item) in filtered.iter().enumerate() {
            let is_selected = i == self.selected;

            let bg = if is_selected { Color::DarkGray } else { Color::Reset };
            let fg = if is_selected { Color::White } else { Color::Gray };

            let marker = if item.is_active {
                Span::styled("● ", Style::default().fg(Color::Green).bg(bg))
            } else {
                Span::styled("○ ", Style::default().fg(Color::DarkGray).bg(bg))
            };

            let name = Span::styled(
                &item.name,
                Style::default().fg(fg).bg(bg).add_modifier(if is_selected {
                    Modifier::BOLD
                } else {
                    Modifier::empty()
                }),
            );

            let meta = Span::styled(
                format!(" ({} msgs, {}tok)", item.message_count, item.tokens),
                Style::default().fg(Color::DarkGray).bg(bg),
            );

            lines.push(Line::from(vec![Span::styled(" ", Style::default().bg(bg)), marker, name, meta]));

            // Preview line
            lines.push(Line::from(vec![
                Span::styled("   ", Style::default().bg(bg)),
                Span::styled(&item.last_prompt, Style::default().fg(Color::DarkGray).bg(bg)),
            ]));
        }

        if filtered.is_empty() {
            lines.push(Line::from(Span::styled(" No matching branches", Style::default().fg(Color::DarkGray))));
        }

        // Scroll to keep selected visible
        let visible_height = list_area.height as usize;
        let selected_visual = self.selected * 2; // 2 lines per entry
        let scroll = if selected_visual >= visible_height {
            (selected_visual - visible_height / 2) as u16
        } else {
            0
        };

        frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)).wrap(Wrap { trim: false }), list_area);
    }
}

// ── Helpers (shared with branch_panel) ──────────────────────────────────────

/// Walk from a block up to the root, returning the path as block IDs.
fn walk_to_root(leaf_id: usize, all_blocks: &[ConversationBlock]) -> Vec<usize> {
    let mut path = Vec::new();
    let mut current = Some(leaf_id);
    while let Some(id) = current {
        path.push(id);
        current = all_blocks.iter().find(|b| b.id == id).and_then(|b| b.parent_block_id);
    }
    path.reverse();
    path
}

fn truncate_first_line(text: &str, max: usize) -> String {
    let first_line = text.lines().next().unwrap_or(text);
    let preview: String = first_line.chars().take(max).collect();
    if first_line.chars().count() > max {
        format!("{}…", preview)
    } else {
        preview
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_block(id: usize, prompt: &str, parent: Option<usize>) -> ConversationBlock {
        let mut b = ConversationBlock::new(id, prompt.to_string());
        b.parent_block_id = parent;
        b.streaming = false;
        b.tokens = 100;
        b
    }

    #[test]
    fn open_discovers_branches() {
        let blocks = vec![
            make_block(0, "root", None),
            make_block(1, "branch-a", Some(0)),
            make_block(2, "branch-b", Some(0)),
        ];
        let active: std::collections::HashSet<usize> = [0, 1].into_iter().collect();

        let mut switcher = BranchSwitcher::new();
        switcher.open(&blocks, &active);

        assert!(switcher.visible);
        assert_eq!(switcher.items.len(), 2);
        // Active branch first
        assert!(switcher.items[0].is_active);
    }

    #[test]
    fn filter_narrows_results() {
        let blocks = vec![
            make_block(0, "root", None),
            make_block(1, "implement auth", Some(0)),
            make_block(2, "fix bug in parser", Some(0)),
        ];
        let active: std::collections::HashSet<usize> = [0, 1].into_iter().collect();

        let mut switcher = BranchSwitcher::new();
        switcher.open(&blocks, &active);
        assert_eq!(switcher.filtered_items().len(), 2);

        switcher.type_char('a');
        switcher.type_char('u');
        switcher.type_char('t');
        switcher.type_char('h');
        // "auth" should match "implement auth"
        assert_eq!(switcher.filtered_items().len(), 1);
        assert!(switcher.filtered_items()[0].last_prompt.contains("auth"));
    }

    #[test]
    fn backspace_widens_filter() {
        let blocks = vec![
            make_block(0, "root", None),
            make_block(1, "alpha", Some(0)),
            make_block(2, "beta", Some(0)),
        ];
        let active: std::collections::HashSet<usize> = [0, 1].into_iter().collect();

        let mut switcher = BranchSwitcher::new();
        switcher.open(&blocks, &active);

        switcher.type_char('a');
        switcher.type_char('l');
        assert_eq!(switcher.filtered_items().len(), 1);

        switcher.backspace();
        switcher.backspace();
        assert_eq!(switcher.filtered_items().len(), 2);
    }

    #[test]
    fn navigation_clamps() {
        let blocks = vec![
            make_block(0, "root", None),
            make_block(1, "a", Some(0)),
            make_block(2, "b", Some(0)),
        ];
        let active: std::collections::HashSet<usize> = [0, 1].into_iter().collect();

        let mut switcher = BranchSwitcher::new();
        switcher.open(&blocks, &active);

        assert_eq!(switcher.selected, 0);
        switcher.move_down();
        assert_eq!(switcher.selected, 1);
        switcher.move_down();
        assert_eq!(switcher.selected, 1); // clamped

        switcher.move_up();
        assert_eq!(switcher.selected, 0);
        switcher.move_up();
        assert_eq!(switcher.selected, 0); // clamped
    }

    #[test]
    fn selected_leaf_id_returns_correct() {
        let blocks = vec![
            make_block(0, "root", None),
            make_block(1, "a", Some(0)),
            make_block(2, "b", Some(0)),
        ];
        let active: std::collections::HashSet<usize> = [0, 1].into_iter().collect();

        let mut switcher = BranchSwitcher::new();
        switcher.open(&blocks, &active);

        assert_eq!(switcher.selected_leaf_id(), Some(1)); // active first
        switcher.move_down();
        assert_eq!(switcher.selected_leaf_id(), Some(2));
    }

    #[test]
    fn close_resets_state() {
        let mut switcher = BranchSwitcher::new();
        switcher.visible = true;
        switcher.filter = "test".to_string();
        switcher.close();
        assert!(!switcher.visible);
        assert!(switcher.filter.is_empty());
    }

    #[test]
    fn empty_switcher_selected_is_none() {
        let switcher = BranchSwitcher::new();
        assert!(switcher.selected_leaf_id().is_none());
    }
}
