//! Session/Branch popup — visualizes conversation tree and branch structure
//!
//! Shows the conversation blocks as a tree with branch points highlighted.
//! Triggered by a keyboard shortcut, renders as a centered floating popup
//! similar to slash commands.

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

use crate::tui::app::App;
use crate::tui::components::block::BlockEntry;
use crate::tui::theme::Theme;

// ── Rendering ───────────────────────────────────────────────────────────────

/// Render the session/branch popup as a centered overlay
pub fn render_session_popup(frame: &mut Frame, app: &App, theme: &Theme) {
    if !app.session_popup_visible {
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

    let block_count = app.blocks.iter().filter(|b| matches!(b, BlockEntry::Conversation(_))).count();
    let branch_count = count_branch_points(app);
    let title = format!(" Session ({} turns, {} branches)  j/k:nav  h/l:branch  Esc:close ", block_count, branch_count);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Span::styled(title, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));

    if app.blocks.is_empty() {
        let empty =
            Paragraph::new(Line::from(Span::styled("No conversation yet.", Style::default().fg(Color::DarkGray))))
                .block(block)
                .wrap(Wrap { trim: false });
        frame.render_widget(empty, area);
        return;
    }

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();

    for entry in &app.blocks {
        match entry {
            BlockEntry::Conversation(conv) => {
                let is_focused = app.focused_block == Some(conv.id);
                let (sibling_idx, sibling_count) = app.block_siblings(conv.id);
                let has_branches = sibling_count > 1;

                // Tree connector
                let connector = if has_branches { "├─" } else { "│ " };
                let connector_color = if has_branches { Color::Yellow } else { Color::DarkGray };

                // Prompt preview (first 50 chars — wider popup allows more)
                let max_preview = (inner.width as usize).saturating_sub(20).max(20);
                let preview: String = conv.prompt.chars().take(max_preview).collect();
                let preview = if conv.prompt.len() > max_preview {
                    format!("{}…", preview)
                } else {
                    preview
                };

                let mut spans = vec![Span::styled(connector, Style::default().fg(connector_color))];

                // Block number
                let num_style = if is_focused {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                spans.push(Span::styled(format!("#{} ", conv.id), num_style));

                // Prompt text
                let text_style = if is_focused {
                    Style::default().fg(Color::White).bg(Color::DarkGray).add_modifier(Modifier::BOLD)
                } else if conv.collapsed {
                    Style::default().fg(Color::DarkGray)
                } else {
                    Style::default().fg(theme.fg)
                };
                spans.push(Span::styled(preview, text_style));

                // Branch indicator
                if has_branches {
                    spans.push(Span::styled(
                        format!(" ({}/{})", sibling_idx + 1, sibling_count),
                        Style::default().fg(Color::Yellow),
                    ));
                }

                // Collapsed indicator
                if conv.collapsed {
                    spans.push(Span::styled(" ▸", Style::default().fg(Color::DarkGray)));
                }

                // Token count if available
                if conv.tokens > 0 {
                    spans.push(Span::styled(format!(" {}t", conv.tokens), Style::default().fg(Color::DarkGray)));
                }

                lines.push(Line::from(spans));

                // Show response count on the next line (indented)
                let resp_count = conv.responses.len();
                if resp_count > 0 && !conv.collapsed {
                    let tool_count =
                        conv.responses.iter().filter(|m| m.role == crate::tui::app::MessageRole::ToolCall).count();
                    let info = if tool_count > 0 {
                        format!("   └ {} responses, {} tool calls", resp_count, tool_count)
                    } else {
                        format!("   └ {} responses", resp_count)
                    };
                    lines.push(Line::from(Span::styled(info, Style::default().fg(Color::DarkGray))));
                }
            }
            BlockEntry::System(msg) => {
                let icon = if msg.is_error { "⚠" } else { "ℹ" };
                let color = if msg.is_error { Color::Red } else { Color::DarkGray };
                let preview: String = msg.content.chars().take(35).collect();
                lines.push(Line::from(vec![
                    Span::styled(format!("  {} ", icon), Style::default().fg(color)),
                    Span::styled(preview, Style::default().fg(Color::DarkGray)),
                ]));
            }
        }
    }

    // Show active block if streaming
    if let Some(ref active) = app.active_block {
        let preview: String = active.prompt.chars().take(40).collect();
        lines.push(Line::from(vec![
            Span::styled("│ ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("#{} ", active.id), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(preview, Style::default().fg(Color::Yellow)),
            Span::styled(" ⏳", Style::default().fg(Color::Yellow)),
        ]));
    }

    let visible_height = inner.height as usize;

    // Auto-scroll to keep focused block visible, otherwise scroll to bottom
    let scroll = if let Some(focused_id) = app.focused_block {
        // Find the line index of the focused block
        let mut focused_line = None;
        let mut line_idx = 0;
        for entry in &app.blocks {
            match entry {
                BlockEntry::Conversation(conv) => {
                    if conv.id == focused_id {
                        focused_line = Some(line_idx);
                    }
                    line_idx += 1; // main line
                    if !conv.responses.is_empty() && !conv.collapsed {
                        line_idx += 1; // response summary line
                    }
                }
                BlockEntry::System(_) => {
                    line_idx += 1;
                }
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
    };

    let para = Paragraph::new(lines).scroll((scroll, 0)).wrap(Wrap { trim: false });
    frame.render_widget(para, inner);
}

/// Count how many blocks have multiple siblings (branch points)
fn count_branch_points(app: &App) -> usize {
    let mut parent_child_count = std::collections::HashMap::new();
    for block in &app.all_blocks {
        *parent_child_count.entry(block.parent_block_id).or_insert(0usize) += 1;
    }
    parent_child_count.values().filter(|&&count| count > 1).count()
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::theme::Theme;

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
}
