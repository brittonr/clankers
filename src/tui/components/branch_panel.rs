//! Branch panel — lists all conversation branches with metadata
//!
//! Shows leaf branches of the conversation tree, highlighting the active
//! branch and providing keybindings to switch, view details, or compare.
//! Implements the `Panel` trait for side-panel integration.

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

use crate::tui::components::block::ConversationBlock;
use crate::tui::panel::DrawContext;
use crate::tui::panel::ListNav;
use crate::tui::panel::Panel;
use crate::tui::panel::PanelAction;
use crate::tui::panel::PanelId;

/// Summary of a block on a branch (for the detail message list).
#[derive(Debug, Clone)]
pub struct BranchBlockSummary {
    /// Block ID
    pub id: usize,
    /// First line of the user prompt
    pub prompt_preview: String,
    /// Number of response messages in the block
    pub response_count: usize,
    /// Number of tool calls in the block
    pub tool_count: usize,
    /// Token usage for this block
    pub tokens: usize,
}

/// Metadata about a single conversation branch (rooted at a leaf block).
#[derive(Debug, Clone)]
pub struct BranchEntry {
    /// The leaf block ID (end of this branch)
    pub leaf_id: usize,
    /// Display name: label, fork reason, or auto-generated
    pub name: String,
    /// Number of blocks on this branch path (root → leaf)
    pub message_count: usize,
    /// Last block's prompt preview
    pub last_prompt: String,
    /// Whether this is the currently active branch
    pub is_active: bool,
    /// Block ID where this branch diverges from its nearest sibling
    pub divergence_id: Option<usize>,
    /// Total tokens used on this branch
    pub total_tokens: usize,
    /// Block path from root to leaf (for detail view)
    pub block_path: Vec<BranchBlockSummary>,
}

/// Branch panel state
#[derive(Debug)]
pub struct BranchPanel {
    /// All discovered branches
    pub entries: Vec<BranchEntry>,
    /// List navigation state
    nav: ListNav,
    /// Whether the detail view is open
    detail_view: bool,
    /// Scroll offset within the detail message list
    detail_scroll: usize,
}

impl Default for BranchPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl BranchPanel {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            nav: ListNav::new(),
            detail_view: false,
            detail_scroll: 0,
        }
    }

    /// Rebuild the branch list from the app's block tree.
    /// Call this whenever blocks change (new block, branch switch, etc.).
    pub fn refresh(&mut self, all_blocks: &[ConversationBlock], active_block_ids: &std::collections::HashSet<usize>) {
        // Find leaf blocks (blocks with no children)
        let has_children: std::collections::HashSet<usize> = all_blocks
            .iter()
            .filter_map(|b| b.parent_block_id)
            .collect();

        let leaves: Vec<&ConversationBlock> = all_blocks
            .iter()
            .filter(|b| !has_children.contains(&b.id))
            .collect();

        // Preserve selection across refresh
        let prev_leaf = self.entries.get(self.nav.selected).map(|e| e.leaf_id);

        self.entries = leaves
            .iter()
            .enumerate()
            .map(|(i, leaf)| {
                // Walk up to root to count messages and gather metadata
                let path = walk_to_root(leaf.id, all_blocks);
                let message_count = path.len();
                let is_active = active_block_ids.contains(&leaf.id);
                let total_tokens: usize = path
                    .iter()
                    .filter_map(|&id| all_blocks.iter().find(|b| b.id == id))
                    .map(|b| b.tokens)
                    .sum();

                // Find divergence point (where this branch splits from another)
                let divergence_id = find_divergence(leaf.id, all_blocks);

                // Generate a name
                let name = format!("branch-{}", i + 1);

                let last_prompt = truncate_first_line(&leaf.prompt, 40);

                // Build block summaries for the detail view
                let block_path: Vec<BranchBlockSummary> = path
                    .iter()
                    .filter_map(|&id| all_blocks.iter().find(|b| b.id == id))
                    .map(|b| {
                        let tool_count = b
                            .responses
                            .iter()
                            .filter(|m| m.role == crate::tui::app::MessageRole::ToolCall)
                            .count();
                        BranchBlockSummary {
                            id: b.id,
                            prompt_preview: truncate_first_line(&b.prompt, 50),
                            response_count: b.responses.len(),
                            tool_count,
                            tokens: b.tokens,
                        }
                    })
                    .collect();

                BranchEntry {
                    leaf_id: leaf.id,
                    name,
                    message_count,
                    last_prompt,
                    is_active,
                    divergence_id,
                    total_tokens,
                    block_path,
                }
            })
            .collect();

        // Sort: active branch first, then by leaf_id descending (most recent first)
        self.entries.sort_by(|a, b| {
            b.is_active
                .cmp(&a.is_active)
                .then(b.leaf_id.cmp(&a.leaf_id))
        });

        // Restore selection
        if let Some(prev) = prev_leaf
            && let Some(idx) = self.entries.iter().position(|e| e.leaf_id == prev)
        {
            self.nav.selected = idx;
        }
        self.nav.clamp(self.entries.len());
    }

    /// Get the currently selected branch entry.
    pub fn selected_entry(&self) -> Option<&BranchEntry> {
        self.entries.get(self.nav.selected)
    }

    /// Get the leaf block ID of the selected branch (for switching).
    pub fn selected_leaf_id(&self) -> Option<usize> {
        self.selected_entry().map(|e| e.leaf_id)
    }
}

// ── Panel trait impl ────────────────────────────────────────────────────────

impl Panel for BranchPanel {
    fn id(&self) -> PanelId {
        PanelId::Branches
    }

    fn title(&self) -> String {
        let active = self.entries.iter().filter(|e| e.is_active).count();
        let total = self.entries.len();
        if total <= 1 {
            "Branches".to_string()
        } else {
            format!("Branches ({}/{})", active, total)
        }
    }

    fn focus_hints(&self) -> &'static str {
        " j/k Enter:switch d:detail "
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn empty_text(&self) -> &'static str {
        "No branches. Use /fork or edit a block to branch."
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> Option<PanelAction> {
        if self.detail_view {
            return match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.detail_view = false;
                    self.detail_scroll = 0;
                    Some(PanelAction::Consumed)
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    // Scroll down in the message list
                    let max = self
                        .selected_entry()
                        .map(|e| e.block_path.len().saturating_sub(1))
                        .unwrap_or(0);
                    self.detail_scroll = (self.detail_scroll + 1).min(max);
                    Some(PanelAction::Consumed)
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.detail_scroll = self.detail_scroll.saturating_sub(1);
                    Some(PanelAction::Consumed)
                }
                KeyCode::Char('s') | KeyCode::Enter => {
                    // Switch to this branch
                    if let Some(entry) = self.selected_entry() {
                        let leaf_id = entry.leaf_id;
                        self.detail_view = false;
                        self.detail_scroll = 0;
                        Some(PanelAction::SwitchBranch(leaf_id))
                    } else {
                        Some(PanelAction::Consumed)
                    }
                }
                _ => None,
            };
        }

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.nav.next(self.entries.len());
                Some(PanelAction::Consumed)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.nav.prev(self.entries.len());
                Some(PanelAction::Consumed)
            }
            KeyCode::Enter => {
                // Switch to selected branch
                if let Some(entry) = self.selected_entry() {
                    Some(PanelAction::SwitchBranch(entry.leaf_id))
                } else {
                    Some(PanelAction::Consumed)
                }
            }
            KeyCode::Char('d') => {
                if !self.entries.is_empty() {
                    self.detail_view = true;
                }
                Some(PanelAction::Consumed)
            }
            KeyCode::Esc | KeyCode::Char('q') => Some(PanelAction::Unfocus),
            _ => None,
        }
    }

    fn handle_scroll(&mut self, up: bool, lines: u16) {
        for _ in 0..lines {
            if up {
                self.nav.prev(self.entries.len());
            } else {
                self.nav.next(self.entries.len());
            }
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
        if self.detail_view {
            render_detail_view(frame, self, area, ctx);
        } else {
            render_list_view(frame, self, area, ctx);
        }
    }
}

// ── Rendering ───────────────────────────────────────────────────────────────

fn render_list_view(frame: &mut Frame, panel: &BranchPanel, area: Rect, ctx: &DrawContext) {
    let mut lines = Vec::new();

    for (i, entry) in panel.entries.iter().enumerate() {
        let is_selected = i == panel.nav.selected;

        // Selection prefix
        let prefix = panel.nav.prefix_span(i, ctx.focused);

        // Active indicator
        let active_marker = if entry.is_active {
            Span::styled("● ", Style::default().fg(Color::Green))
        } else {
            Span::styled("○ ", Style::default().fg(Color::DarkGray))
        };

        // Branch name
        let name_style = panel.nav.item_style(i, ctx.focused, Style::default().fg(Color::White));
        let name = Span::styled(&entry.name, name_style);

        // Message count
        let count = Span::styled(
            format!(" ({})", entry.message_count),
            Style::default().fg(Color::DarkGray),
        );

        lines.push(Line::from(vec![prefix, active_marker, name, count]));

        // Second line: last prompt preview
        let preview_style = if is_selected && ctx.focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(&entry.last_prompt, preview_style),
        ]));
    }

    let visible_height = area.height as usize;
    let scroll_offset = panel.nav.scroll_offset(visible_height, 2);

    frame.render_widget(
        Paragraph::new(lines)
            .scroll((scroll_offset, 0))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_detail_view(frame: &mut Frame, panel: &BranchPanel, area: Rect, ctx: &DrawContext) {
    let entry = match panel.selected_entry() {
        Some(e) => e,
        None => return,
    };

    let active_label = if entry.is_active { "active" } else { "inactive" };
    let active_color = if entry.is_active { Color::Green } else { Color::DarkGray };

    let mut lines = vec![
        Line::from(vec![
            Span::styled(&entry.name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(format!(" ({})", active_label), Style::default().fg(active_color)),
        ]),
        Line::from(vec![
            Span::styled("  Messages: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}", entry.message_count), Style::default().fg(Color::White)),
            Span::styled("  Tokens: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}", entry.total_tokens), Style::default().fg(Color::White)),
        ]),
    ];

    if let Some(div_id) = entry.divergence_id {
        lines.push(Line::from(vec![
            Span::styled("  Fork from: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("#{}", div_id), Style::default().fg(Color::Yellow)),
        ]));
    }

    // Divider
    lines.push(Line::from(Span::styled(
        "─".repeat(area.width as usize),
        Style::default().fg(Color::DarkGray),
    )));

    // Scrollable message list
    let visible_start = panel.detail_scroll;
    for (i, block) in entry.block_path.iter().enumerate().skip(visible_start) {
        let is_divergence = entry.divergence_id == Some(block.id);
        let is_leaf = block.id == entry.leaf_id;

        // Connector
        let connector = if is_leaf {
            "└─"
        } else {
            "├─"
        };
        let connector_color = if is_divergence {
            Color::Yellow
        } else {
            Color::DarkGray
        };

        // Block number + prompt
        let num_style = if is_leaf {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else if is_divergence {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::White)
        };

        let mut spans = vec![
            Span::styled(connector, Style::default().fg(connector_color)),
            Span::styled(format!(" #{} ", block.id), num_style),
            Span::styled(
                &block.prompt_preview,
                Style::default().fg(if i == visible_start { Color::White } else { Color::Gray }),
            ),
        ];

        // Divergence marker
        if is_divergence {
            spans.push(Span::styled(" ⑂", Style::default().fg(Color::Yellow)));
        }

        lines.push(Line::from(spans));

        // Response summary line (compact)
        if block.tool_count > 0 {
            lines.push(Line::from(vec![
                Span::styled("│  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{}r {}t {}tok", block.response_count, block.tool_count, block.tokens),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        } else if block.tokens > 0 {
            lines.push(Line::from(vec![
                Span::styled("│  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{}r {}tok", block.response_count, block.tokens),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    }

    // Hints
    lines.push(Line::from(""));
    if ctx.focused {
        lines.push(Line::from(Span::styled(
            " s:switch  j/k:scroll  Esc:back",
            Style::default().fg(Color::DarkGray),
        )));
    }

    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }),
        area,
    );
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Walk from a block up to the root, returning the path as a list of block IDs.
fn walk_to_root(leaf_id: usize, all_blocks: &[ConversationBlock]) -> Vec<usize> {
    let mut path = Vec::new();
    let mut current = Some(leaf_id);
    while let Some(id) = current {
        path.push(id);
        current = all_blocks
            .iter()
            .find(|b| b.id == id)
            .and_then(|b| b.parent_block_id);
    }
    path.reverse();
    path
}

/// Find the block ID where this branch diverges from a sibling branch.
/// Returns the parent block ID that has multiple children.
fn find_divergence(leaf_id: usize, all_blocks: &[ConversationBlock]) -> Option<usize> {
    let mut current = Some(leaf_id);
    while let Some(id) = current {
        let block = all_blocks.iter().find(|b| b.id == id)?;
        if let Some(parent_id) = block.parent_block_id {
            // Count siblings at this level
            let sibling_count = all_blocks
                .iter()
                .filter(|b| b.parent_block_id == Some(parent_id))
                .count();
            if sibling_count > 1 {
                return Some(parent_id);
            }
        }
        current = block.parent_block_id;
    }
    None
}

/// Truncate text to the first line and a max character count.
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
    fn walk_to_root_linear() {
        let blocks = vec![
            make_block(0, "root", None, 100),
            make_block(1, "second", Some(0), 200),
            make_block(2, "third", Some(1), 150),
        ];
        let path = walk_to_root(2, &blocks);
        assert_eq!(path, vec![0, 1, 2]);
    }

    #[test]
    fn walk_to_root_single() {
        let blocks = vec![make_block(0, "only", None, 50)];
        let path = walk_to_root(0, &blocks);
        assert_eq!(path, vec![0]);
    }

    #[test]
    fn find_divergence_no_branches() {
        let blocks = vec![
            make_block(0, "root", None, 100),
            make_block(1, "child", Some(0), 200),
        ];
        assert_eq!(find_divergence(1, &blocks), None);
    }

    #[test]
    fn find_divergence_with_fork() {
        let blocks = vec![
            make_block(0, "root", None, 100),
            make_block(1, "branch-a", Some(0), 200),
            make_block(2, "branch-b", Some(0), 150),
        ];
        // Both branches diverge at block 0
        assert_eq!(find_divergence(1, &blocks), Some(0));
        assert_eq!(find_divergence(2, &blocks), Some(0));
    }

    #[test]
    fn find_divergence_deep_fork() {
        let blocks = vec![
            make_block(0, "root", None, 100),
            make_block(1, "mid", Some(0), 200),
            make_block(2, "deep-a", Some(1), 150),
            make_block(3, "deep-b", Some(1), 120),
        ];
        // deep-a and deep-b diverge at block 1
        assert_eq!(find_divergence(2, &blocks), Some(1));
        assert_eq!(find_divergence(3, &blocks), Some(1));
    }

    #[test]
    fn refresh_discovers_branches() {
        let blocks = vec![
            make_block(0, "root", None, 100),
            make_block(1, "branch-a child", Some(0), 200),
            make_block(2, "branch-b child", Some(0), 150),
        ];
        let active: std::collections::HashSet<usize> = [0, 1].into_iter().collect();

        let mut panel = BranchPanel::new();
        panel.refresh(&blocks, &active);

        assert_eq!(panel.entries.len(), 2);
        // Active branch should sort first
        assert!(panel.entries[0].is_active);
        assert_eq!(panel.entries[0].leaf_id, 1);
        assert!(!panel.entries[1].is_active);
        assert_eq!(panel.entries[1].leaf_id, 2);
    }

    #[test]
    fn refresh_linear_single_branch() {
        let blocks = vec![
            make_block(0, "root", None, 100),
            make_block(1, "second", Some(0), 200),
            make_block(2, "third", Some(1), 150),
        ];
        let active: std::collections::HashSet<usize> = [0, 1, 2].into_iter().collect();

        let mut panel = BranchPanel::new();
        panel.refresh(&blocks, &active);

        // Linear conversation = 1 branch (the leaf)
        assert_eq!(panel.entries.len(), 1);
        assert!(panel.entries[0].is_active);
        assert_eq!(panel.entries[0].message_count, 3);
    }

    #[test]
    fn refresh_counts_tokens() {
        let blocks = vec![
            make_block(0, "root", None, 100),
            make_block(1, "child", Some(0), 200),
        ];
        let active: std::collections::HashSet<usize> = [0, 1].into_iter().collect();

        let mut panel = BranchPanel::new();
        panel.refresh(&blocks, &active);

        assert_eq!(panel.entries[0].total_tokens, 300);
    }

    #[test]
    fn truncate_first_line_short() {
        assert_eq!(truncate_first_line("hello", 10), "hello");
    }

    #[test]
    fn truncate_first_line_long() {
        assert_eq!(
            truncate_first_line("hello world this is a long prompt", 10),
            "hello worl…"
        );
    }

    #[test]
    fn truncate_first_line_multiline() {
        assert_eq!(truncate_first_line("first\nsecond\nthird", 20), "first");
    }

    #[test]
    fn selected_entry_empty() {
        let panel = BranchPanel::new();
        assert!(panel.selected_entry().is_none());
        assert!(panel.selected_leaf_id().is_none());
    }

    #[test]
    fn navigation_wraps() {
        let blocks = vec![
            make_block(0, "root", None, 100),
            make_block(1, "a", Some(0), 200),
            make_block(2, "b", Some(0), 150),
        ];
        let active: std::collections::HashSet<usize> = [0, 1].into_iter().collect();

        let mut panel = BranchPanel::new();
        panel.refresh(&blocks, &active);

        assert_eq!(panel.nav.selected, 0);
        panel.nav.next(panel.entries.len());
        assert_eq!(panel.nav.selected, 1);
        panel.nav.next(panel.entries.len());
        assert_eq!(panel.nav.selected, 0); // wrapped
    }
}
