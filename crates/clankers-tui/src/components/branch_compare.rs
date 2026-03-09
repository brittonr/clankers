//! Branch comparison view — side-by-side diff of two conversation branches
//!
//! Shows the divergence point (last common ancestor) at the top, then
//! unique blocks from each branch in a split-pane layout. Provides
//! navigation and actions (switch to either branch).

use ratatui::Frame;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
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

use crate::components::block::ConversationBlock;

/// Summary of a block in the comparison view.
#[derive(Debug, Clone)]
pub struct CompareBlock {
    pub id: usize,
    pub prompt_preview: String,
    pub response_count: usize,
    pub tool_count: usize,
    pub tokens: usize,
}

/// Result of comparing two branches.
#[derive(Debug, Clone)]
pub struct BranchComparison {
    /// The divergence point block ID (last common ancestor).
    /// `None` if branches share no common ancestor (shouldn't happen in practice).
    pub divergence_id: Option<usize>,
    /// Prompt preview at the divergence point
    pub divergence_prompt: String,
    /// Blocks unique to branch A (from divergence → leaf A)
    pub branch_a: Vec<CompareBlock>,
    /// Blocks unique to branch B (from divergence → leaf B)
    pub branch_b: Vec<CompareBlock>,
    /// Leaf block ID of branch A
    pub leaf_a: usize,
    /// Leaf block ID of branch B
    pub leaf_b: usize,
    /// Display name for branch A
    pub name_a: String,
    /// Display name for branch B
    pub name_b: String,
    /// Total tokens for branch A (unique portion only)
    pub tokens_a: usize,
    /// Total tokens for branch B (unique portion only)
    pub tokens_b: usize,
}

/// Compare two branches, returning their divergence and unique blocks.
pub fn compare_branches(leaf_a: usize, leaf_b: usize, all_blocks: &[ConversationBlock]) -> Option<BranchComparison> {
    let path_a = walk_to_root(leaf_a, all_blocks);
    let path_b = walk_to_root(leaf_b, all_blocks);

    // Find the last common block (divergence point)
    let mut divergence_idx = 0;
    for (i, (&a, &b)) in path_a.iter().zip(path_b.iter()).enumerate() {
        if a == b {
            divergence_idx = i;
        } else {
            break;
        }
    }

    let divergence_id = path_a.get(divergence_idx).copied();
    let divergence_prompt = divergence_id
        .and_then(|id| all_blocks.iter().find(|b| b.id == id))
        .map(|b| truncate_first_line(&b.prompt, 60))
        .unwrap_or_default();

    // Unique blocks: everything after the divergence point
    let unique_a: Vec<CompareBlock> = path_a[divergence_idx + 1..]
        .iter()
        .filter_map(|&id| all_blocks.iter().find(|b| b.id == id))
        .map(block_to_compare)
        .collect();

    let unique_b: Vec<CompareBlock> = path_b[divergence_idx + 1..]
        .iter()
        .filter_map(|&id| all_blocks.iter().find(|b| b.id == id))
        .map(block_to_compare)
        .collect();

    let tokens_a: usize = unique_a.iter().map(|b| b.tokens).sum();
    let tokens_b: usize = unique_b.iter().map(|b| b.tokens).sum();

    Some(BranchComparison {
        divergence_id,
        divergence_prompt,
        branch_a: unique_a,
        branch_b: unique_b,
        leaf_a,
        leaf_b,
        name_a: format!("branch (#{} leaf)", leaf_a),
        name_b: format!("branch (#{} leaf)", leaf_b),
        tokens_a,
        tokens_b,
    })
}

fn block_to_compare(b: &ConversationBlock) -> CompareBlock {
    use crate::app::MessageRole;
    CompareBlock {
        id: b.id,
        prompt_preview: truncate_first_line(&b.prompt, 50),
        response_count: b.responses.len(),
        tool_count: b.responses.iter().filter(|m| m.role == MessageRole::ToolCall).count(),
        tokens: b.tokens,
    }
}

/// Branch comparison overlay state.
#[derive(Debug, Default)]
pub struct BranchCompareView {
    /// The comparison data (None when not open)
    pub comparison: Option<BranchComparison>,
    /// Whether the view is visible
    pub visible: bool,
    /// Which pane is focused (false = left/A, true = right/B)
    pub right_focused: bool,
    /// Scroll offset for the left pane
    pub scroll_a: usize,
    /// Scroll offset for the right pane
    pub scroll_b: usize,
}

impl BranchCompareView {
    pub fn new() -> Self {
        Self::default()
    }

    /// Open the comparison view with two branch leaf IDs.
    pub fn open(&mut self, leaf_a: usize, leaf_b: usize, all_blocks: &[ConversationBlock]) {
        self.comparison = compare_branches(leaf_a, leaf_b, all_blocks);
        self.visible = true;
        self.right_focused = false;
        self.scroll_a = 0;
        self.scroll_b = 0;
    }

    /// Close the view.
    pub fn close(&mut self) {
        self.visible = false;
        self.comparison = None;
    }

    /// Scroll the focused pane down.
    pub fn scroll_down(&mut self) {
        if let Some(cmp) = &self.comparison {
            if self.right_focused {
                let max = cmp.branch_b.len().saturating_sub(1);
                self.scroll_b = (self.scroll_b + 1).min(max);
            } else {
                let max = cmp.branch_a.len().saturating_sub(1);
                self.scroll_a = (self.scroll_a + 1).min(max);
            }
        }
    }

    /// Scroll the focused pane up.
    pub fn scroll_up(&mut self) {
        if self.right_focused {
            self.scroll_b = self.scroll_b.saturating_sub(1);
        } else {
            self.scroll_a = self.scroll_a.saturating_sub(1);
        }
    }

    /// Toggle focus between left and right pane.
    pub fn toggle_focus(&mut self) {
        self.right_focused = !self.right_focused;
    }

    /// Get the leaf ID of the focused branch.
    pub fn focused_leaf_id(&self) -> Option<usize> {
        self.comparison.as_ref().map(|c| if self.right_focused { c.leaf_b } else { c.leaf_a })
    }

    /// Render the comparison view as a floating overlay.
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if !self.visible {
            return;
        }
        let cmp = match &self.comparison {
            Some(c) => c,
            None => return,
        };

        // Size: 80% width, 80% height, centered
        let width = (area.width * 80 / 100).max(50).min(area.width.saturating_sub(4));
        let height = (area.height * 80 / 100).max(15).min(area.height.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let popup_area = Rect::new(x, y, width, height);

        frame.render_widget(Clear, popup_area);

        let outer = Block::default()
            .title(Span::styled(" Branch Comparison ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = outer.inner(popup_area);
        frame.render_widget(outer, popup_area);

        if inner.height < 5 || inner.width < 10 {
            return;
        }

        // Top: divergence info (2 lines)
        let div_area = Rect::new(inner.x, inner.y, inner.width, 2);
        let div_lines = vec![
            Line::from(vec![
                Span::styled(" Diverges at: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    cmp.divergence_id.map(|id| format!("#{}", id)).unwrap_or_else(|| "root".to_string()),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(format!(" — {}", cmp.divergence_prompt), Style::default().fg(Color::Gray)),
            ]),
            Line::from(vec![Span::styled(
                " ←/→: pane  j/k: scroll  s: switch  q: close",
                Style::default().fg(Color::DarkGray),
            )]),
        ];
        frame.render_widget(Paragraph::new(div_lines).wrap(Wrap { trim: false }), div_area);

        // Split remaining area into two panes
        let pane_area = Rect::new(inner.x, inner.y + 2, inner.width, inner.height - 2);
        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(pane_area);

        render_comparison_pane(
            frame,
            &cmp.name_a,
            &cmp.branch_a,
            cmp.tokens_a,
            self.scroll_a,
            !self.right_focused,
            panes[0],
        );
        render_comparison_pane(
            frame,
            &cmp.name_b,
            &cmp.branch_b,
            cmp.tokens_b,
            self.scroll_b,
            self.right_focused,
            panes[1],
        );
    }
}

/// Render one pane of the comparison.
#[allow(clippy::too_many_arguments)]
fn render_comparison_pane(
    frame: &mut Frame,
    name: &str,
    blocks: &[CompareBlock],
    total_tokens: usize,
    scroll: usize,
    focused: bool,
    area: Rect,
) {
    let border_color = if focused { Color::Cyan } else { Color::DarkGray };
    let title = format!(" {} ({} unique, {}tok) ", name, blocks.len(), total_tokens,);

    let block = Block::default()
        .title(Span::styled(title, Style::default().fg(border_color).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if blocks.is_empty() {
        frame.render_widget(
            Paragraph::new(Span::styled("(no unique blocks)", Style::default().fg(Color::DarkGray))),
            inner,
        );
        return;
    }

    let mut lines = Vec::new();
    for (i, b) in blocks.iter().enumerate().skip(scroll) {
        let num_style = if focused {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::Gray)
        };

        lines.push(Line::from(vec![
            Span::styled(format!("#{} ", b.id), num_style),
            Span::styled(&b.prompt_preview, Style::default().fg(if i == scroll { Color::White } else { Color::Gray })),
        ]));

        // Compact stats
        let stats = if b.tool_count > 0 {
            format!("  {}r {}t {}tok", b.response_count, b.tool_count, b.tokens)
        } else {
            format!("  {}r {}tok", b.response_count, b.tokens)
        };
        lines.push(Line::from(Span::styled(stats, Style::default().fg(Color::DarkGray))));
    }

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), inner);
}

// ── Helpers ─────────────────────────────────────────────────────────────────

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

    fn make_block(id: usize, prompt: &str, parent: Option<usize>, tokens: usize) -> ConversationBlock {
        let mut b = ConversationBlock::new(id, prompt.to_string());
        b.parent_block_id = parent;
        b.streaming = false;
        b.tokens = tokens;
        b
    }

    #[test]
    fn compare_simple_fork() {
        let blocks = vec![
            make_block(0, "root question", None, 100),
            make_block(1, "answer-a", Some(0), 200),
            make_block(2, "answer-b", Some(0), 150),
        ];
        let cmp = compare_branches(1, 2, &blocks).unwrap();

        assert_eq!(cmp.divergence_id, Some(0));
        assert_eq!(cmp.branch_a.len(), 1);
        assert_eq!(cmp.branch_b.len(), 1);
        assert_eq!(cmp.branch_a[0].id, 1);
        assert_eq!(cmp.branch_b[0].id, 2);
        assert_eq!(cmp.tokens_a, 200);
        assert_eq!(cmp.tokens_b, 150);
    }

    #[test]
    fn compare_deep_fork() {
        // root → mid → deep-a
        //             → deep-b → deeper-b
        let blocks = vec![
            make_block(0, "root", None, 100),
            make_block(1, "mid", Some(0), 200),
            make_block(2, "deep-a", Some(1), 150),
            make_block(3, "deep-b", Some(1), 120),
            make_block(4, "deeper-b", Some(3), 80),
        ];
        let cmp = compare_branches(2, 4, &blocks).unwrap();

        // Diverges at block 1 (mid)
        assert_eq!(cmp.divergence_id, Some(1));
        // Branch A: [deep-a]
        assert_eq!(cmp.branch_a.len(), 1);
        assert_eq!(cmp.branch_a[0].id, 2);
        // Branch B: [deep-b, deeper-b]
        assert_eq!(cmp.branch_b.len(), 2);
        assert_eq!(cmp.branch_b[0].id, 3);
        assert_eq!(cmp.branch_b[1].id, 4);
    }

    #[test]
    fn compare_same_branch_no_unique() {
        let blocks = vec![make_block(0, "root", None, 100), make_block(1, "child", Some(0), 200)];
        // Comparing a branch with itself: leaf 1 vs leaf 1
        let cmp = compare_branches(1, 1, &blocks).unwrap();
        assert_eq!(cmp.divergence_id, Some(1));
        assert!(cmp.branch_a.is_empty());
        assert!(cmp.branch_b.is_empty());
    }

    #[test]
    fn compare_asymmetric_depths() {
        // root → a → a2 → a3
        //       → b
        let blocks = vec![
            make_block(0, "root", None, 50),
            make_block(1, "a", Some(0), 100),
            make_block(2, "a2", Some(1), 100),
            make_block(3, "a3", Some(2), 100),
            make_block(4, "b", Some(0), 200),
        ];
        let cmp = compare_branches(3, 4, &blocks).unwrap();

        assert_eq!(cmp.divergence_id, Some(0));
        assert_eq!(cmp.branch_a.len(), 3); // a, a2, a3
        assert_eq!(cmp.branch_b.len(), 1); // b
    }

    #[test]
    fn view_toggle_focus() {
        let mut view = BranchCompareView::new();
        assert!(!view.right_focused);
        view.toggle_focus();
        assert!(view.right_focused);
        view.toggle_focus();
        assert!(!view.right_focused);
    }

    #[test]
    fn view_scroll_clamps() {
        let blocks = vec![
            make_block(0, "root", None, 100),
            make_block(1, "a", Some(0), 200),
            make_block(2, "b", Some(0), 150),
        ];
        let mut view = BranchCompareView::new();
        view.open(1, 2, &blocks);

        // Each branch has 1 unique block
        view.scroll_down();
        assert_eq!(view.scroll_a, 0); // clamped (only 1 block)

        // Scroll up from 0 stays at 0
        view.scroll_up();
        assert_eq!(view.scroll_a, 0);
    }

    #[test]
    fn focused_leaf_id_tracks_pane() {
        let blocks = vec![
            make_block(0, "root", None, 100),
            make_block(1, "a", Some(0), 200),
            make_block(2, "b", Some(0), 150),
        ];
        let mut view = BranchCompareView::new();
        view.open(1, 2, &blocks);

        assert_eq!(view.focused_leaf_id(), Some(1)); // left focused
        view.toggle_focus();
        assert_eq!(view.focused_leaf_id(), Some(2)); // right focused
    }

    #[test]
    fn close_clears_state() {
        let blocks = vec![
            make_block(0, "root", None, 100),
            make_block(1, "a", Some(0), 200),
            make_block(2, "b", Some(0), 150),
        ];
        let mut view = BranchCompareView::new();
        view.open(1, 2, &blocks);
        assert!(view.visible);
        assert!(view.comparison.is_some());

        view.close();
        assert!(!view.visible);
        assert!(view.comparison.is_none());
    }
}
