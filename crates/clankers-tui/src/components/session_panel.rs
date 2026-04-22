//! Session/Branch popup — visualizes conversation tree and branch structure
//!
//! Shows the conversation blocks as a tree with branch points highlighted.
//! Triggered by a keyboard shortcut, renders as a centered floating popup
//! similar to slash commands.

use std::collections::HashSet;

use super::prelude::*;
use crate::app::App;
use crate::app::MessageRole;
use crate::components::block::BlockEntry;
use crate::components::block::ConversationBlock;

// ── Rendering ───────────────────────────────────────────────────────────────

/// Render the session/branch popup as a centered overlay
pub fn render_session_popup(frame: &mut Frame, app: &App, theme: &Theme) {
    if !app.overlays.session_popup_visible {
        return;
    }

    let screen = frame.area();

    // Size: 60% width, 70% height, centered
    let popup_width = (screen.width * 60 / 100).max(40).min(screen.width.saturating_sub(4));
    let popup_height = (screen.height * 70 / 100).max(10).min(screen.height.saturating_sub(4));
    let x = (screen.width.saturating_sub(popup_width)) / 2;
    let y = (screen.height.saturating_sub(popup_height)) / 2;
    let area = Rect::new(x, y, popup_width, popup_height);

    frame.render_widget(Clear, area);

    let total_blocks = app.conversation.all_blocks.len();
    let branch_count = count_branch_points(app);
    let title = format!(
        " Session ({} turns, {} branches)  j/k:nav  h/l:branch  e:edit  Esc:close ",
        total_blocks, branch_count
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(title, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));

    if app.conversation.all_blocks.is_empty() {
        let empty =
            Paragraph::new(Line::from(Span::styled("No conversation yet.", Style::default().fg(Color::DarkGray))))
                .block(block)
                .wrap(Wrap { trim: false });
        frame.render_widget(empty, area);
        return;
    }

    let inner = block.inner(area);
    frame.render_widget(block, area);

    // Build set of active (visible) block IDs
    let active_ids: HashSet<usize> = app
        .conversation
        .blocks
        .iter()
        .filter_map(|e| match e {
            BlockEntry::Conversation(b) => Some(b.id),
            _ => None,
        })
        .collect();

    let max_preview = (inner.width as usize).saturating_sub(20).max(20);

    // DFS tree walk over all_blocks
    let mut lines = Vec::new();
    let roots: Vec<&ConversationBlock> =
        app.conversation.all_blocks.iter().filter(|b| b.parent_block_id.is_none()).collect();

    for root in &roots {
        render_tree_node(&mut lines, app, root, &active_ids, "", true, max_preview, theme);
    }

    // Show active block (streaming) at the end
    if let Some(active) = &app.conversation.active_block {
        let preview = truncate_preview(&active.prompt, max_preview);
        lines.push(Line::from(vec![
            Span::styled("│ ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("#{} ", active.id), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(preview, Style::default().fg(Color::Yellow)),
            Span::styled(" ⏳", Style::default().fg(Color::Yellow)),
        ]));
    }

    let visible_height = inner.height as usize;

    // Auto-scroll to keep focused block visible
    let scroll = compute_scroll(&lines, app.conversation.focused_block, visible_height, app);

    let para = Paragraph::new(lines).scroll((scroll, 0)).wrap(Wrap { trim: false });
    frame.render_widget(para, inner);
}

// ── Tree rendering ──────────────────────────────────────────────────────────

/// Render a single tree node and its descendants recursively.
/// `prefix` is the indentation string for the current depth.
/// `is_last` indicates whether this is the last child of its parent.
/// Render a session tree node. Recursion follows the conversation tree
/// (bounded by conversation depth, typically ≤10 for branching sessions).
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(no_recursion, reason = "conversation tree depth bounded by session branching limits")
)]
fn render_tree_node(
    lines: &mut Vec<Line<'static>>,
    app: &App,
    block: &ConversationBlock,
    active_ids: &HashSet<usize>,
    prefix: &str,
    is_last: bool,
    max_preview: usize,
    theme: &Theme,
) {
    let is_active = active_ids.contains(&block.id);
    let is_focused = app.conversation.focused_block == Some(block.id);

    // Tree connector character
    let connector = if prefix.is_empty() {
        ""
    } else if is_last {
        "└─"
    } else {
        "├─"
    };

    // Build the line
    let mut spans = Vec::new();

    // Prefix + connector
    if !prefix.is_empty() {
        spans.push(Span::styled(prefix.to_string(), Style::default().fg(Color::DarkGray)));
        spans.push(Span::styled(
            connector.to_string(),
            if is_active {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        ));
    }

    // Active marker
    if is_active {
        spans.push(Span::styled("* ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
    } else {
        spans.push(Span::raw("  "));
    }

    // Block number
    let num_style = if is_focused {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else if is_active {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    spans.push(Span::styled(format!("#{} ", block.id), num_style));

    // Prompt text
    let preview = truncate_preview(&block.prompt, max_preview);
    let text_style = if is_focused {
        Style::default().fg(Color::White).bg(Color::DarkGray).add_modifier(Modifier::BOLD)
    } else if is_active {
        Style::default().fg(theme.fg)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    spans.push(Span::styled(preview, text_style));

    // Collapsed indicator
    if block.collapsed {
        spans.push(Span::styled(" ▸", Style::default().fg(Color::DarkGray)));
    }

    // Token count
    if block.tokens > 0 {
        spans.push(Span::styled(format!(" {}t", block.tokens), Style::default().fg(Color::DarkGray)));
    }

    lines.push(Line::from(spans));

    // Response summary (indented under the block)
    if !block.collapsed && !block.responses.is_empty() {
        let resp_count = block.responses.len();
        let tool_count = block.responses.iter().filter(|m| m.role == MessageRole::ToolCall).count();
        let info = if tool_count > 0 {
            format!("{} responses, {} tool calls", resp_count, tool_count)
        } else {
            format!("{} responses", resp_count)
        };

        let detail_prefix = if prefix.is_empty() {
            "  ".to_string()
        } else if is_last {
            format!("{}   ", prefix)
        } else {
            format!("{}│  ", prefix)
        };

        let detail_style = if is_active {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM)
        };

        lines.push(Line::from(vec![
            Span::styled(detail_prefix, Style::default().fg(Color::DarkGray)),
            Span::styled(format!("└ {}", info), detail_style),
        ]));
    }

    // Find children and recurse
    let children: Vec<&ConversationBlock> =
        app.conversation.all_blocks.iter().filter(|b| b.parent_block_id == Some(block.id)).collect();

    if children.is_empty() {
        return;
    }

    // Build the child prefix
    let child_prefix = if prefix.is_empty() {
        String::new()
    } else if is_last {
        format!("{}   ", prefix)
    } else {
        format!("{}│  ", prefix)
    };

    for (i, child) in children.iter().enumerate() {
        let is_last_child = i == children.len() - 1;
        render_tree_node(lines, app, child, active_ids, &child_prefix, is_last_child, max_preview, theme);
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn truncate_preview(text: &str, max: usize) -> String {
    // Take only the first line
    let first_line = text.lines().next().unwrap_or(text);
    let preview: String = first_line.chars().take(max).collect();
    if first_line.len() > max {
        format!("{}…", preview)
    } else {
        preview
    }
}

fn compute_scroll(lines: &[Line], focused_block: Option<usize>, visible_height: usize, _app: &App) -> u16 {
    if let Some(focused_id) = focused_block {
        // Find the line index of the focused block by searching for its #ID
        let target = format!("#{} ", focused_id);
        let mut focused_line = None;
        for (i, line) in lines.iter().enumerate() {
            let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            if text.contains(&target) && !text.contains("└ ") {
                focused_line = Some(i);
                break;
            }
        }
        if let Some(fl) = focused_line {
            if fl >= visible_height {
                (fl - visible_height / 2) as u16
            } else {
                0
            }
        } else if lines.len() > visible_height {
            (lines.len() - visible_height) as u16
        } else {
            0
        }
    } else if lines.len() > visible_height {
        (lines.len() - visible_height) as u16
    } else {
        0
    }
}

/// Count how many blocks have multiple children (branch points)
fn count_branch_points(app: &App) -> usize {
    let mut parent_child_count = std::collections::HashMap::new();
    for block in &app.conversation.all_blocks {
        *parent_child_count.entry(block.parent_block_id).or_insert(0usize) += 1;
    }
    parent_child_count.values().filter(|&&count| count > 1).count()
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    #[test]
    fn test_count_branch_points_empty() {
        let app = App::new("test".into(), "/tmp".into(), Theme::dark());
        assert_eq!(count_branch_points(&app), 0);
    }

    #[test]
    fn test_count_branch_points_linear() {
        let mut app = App::new("test".into(), "/tmp".into(), Theme::dark());
        // Simulate linear conversation (no branches)
        app.start_block("first".into(), 0);
        app.finalize_active_block();
        app.start_block("second".into(), 1);
        app.finalize_active_block();
        assert_eq!(count_branch_points(&app), 0);
    }

    #[test]
    fn test_count_branch_points_with_fork() {
        let mut app = App::new("test".into(), "/tmp".into(), Theme::dark());
        // Block 0: root
        app.start_block("root question".into(), 0);
        app.finalize_active_block();
        // Block 1: first child of block 0
        app.start_block("first answer".into(), 1);
        app.finalize_active_block();
        // Block 2: second child of block 0 (fork)
        // Manually create a fork by manipulating all_blocks
        let mut forked = ConversationBlock::new_synthetic(2, "alternative answer".into());
        forked.parent_block_id = Some(0);
        forked.streaming = false;
        app.conversation.all_blocks.push(forked);
        // Now block 0 has 2 children → 1 branch point
        assert_eq!(count_branch_points(&app), 1);
    }

    #[test]
    fn test_truncate_preview() {
        assert_eq!(truncate_preview("hello", 10), "hello");
        assert_eq!(truncate_preview("hello world this is long", 10), "hello worl…");
        assert_eq!(truncate_preview("line1\nline2\nline3", 20), "line1");
    }

    #[test]
    fn test_tree_structure_active_marking() {
        let mut app = App::new("test".into(), "/tmp".into(), Theme::dark());
        app.start_block("root".into(), 0);
        app.finalize_active_block();
        app.start_block("child".into(), 1);
        app.finalize_active_block();

        let active_ids: HashSet<usize> = app
            .conversation
            .blocks
            .iter()
            .filter_map(|e| match e {
                BlockEntry::Conversation(b) => Some(b.id),
                _ => None,
            })
            .collect();

        // Both blocks should be active (on the visible path)
        assert!(active_ids.contains(&0));
        assert!(active_ids.contains(&1));
    }
}
