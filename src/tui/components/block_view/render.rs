//! Core rendering functions for conversation blocks and messages

use std::collections::HashMap;

use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;

use super::super::block::ConversationBlock;
use super::super::markdown::MarkdownStyle;
use super::super::markdown::render_markdown;
use super::super::progress_renderer::ProgressRenderer;
use super::super::streaming_output::StreamingOutputManager;
use super::helpers::{format_elapsed, hrule, hrule_dotted};
use super::{BlockBranchInfo, FOCUSED_OUTPUT_LINES, LIVE_OUTPUT_MAX_LINES, SPINNER};
use crate::tui::app::ActiveToolExecution;
use crate::tui::app::DisplayMessage;
use crate::tui::app::MessageRole;
use crate::tui::theme::Theme;

/// Render a single conversation block into lines.
/// `branch_info` carries sibling/children/ID-display metadata when available.
pub fn render_conversation_block<'a>(
    block: &ConversationBlock,
    focused: bool,
    show_thinking: bool,
    theme: &Theme,
    width: usize,
    branch_info: Option<BlockBranchInfo>,
    active_tools: &HashMap<String, ActiveToolExecution>,
    progress: &ProgressRenderer,
    streaming_outputs: &mut StreamingOutputManager,
    tick: u64,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    let border_color = if focused {
        theme.block_border_focused
    } else {
        theme.block_border
    };
    let border_style = Style::default().fg(border_color);

    let time = block.timestamp.format("%H:%M:%S").to_string();
    let status_icon = if block.streaming {
        ("… ", Style::default().fg(Color::Yellow), 2)
    } else if block.error.is_some() {
        ("✗ ", Style::default().fg(theme.error), 2)
    } else {
        ("✓ ", Style::default().fg(Color::Green), 2)
    };

    // ── Top border ──────────────────────────────────
    // Build optional branch indicator like " ◂ 2/3 ▸"
    let branch_label = match &branch_info {
        Some(info) if info.sibling_total > 1 => {
            format!(" ◂ {}/{} ▸", info.sibling_index + 1, info.sibling_total)
        }
        _ => String::new(),
    };
    let branch_display_len = branch_label.len();

    // Optional block ID like " #5"
    let id_label = match &branch_info {
        Some(info) if info.show_id => format!(" #{}", block.id),
        _ => String::new(),
    };
    let id_display_len = id_label.len();

    // "┌─ " (3) + icon (varies) + time (8) + " " (1) + branch_label + id_label
    let top_prefix_len = 3 + status_icon.2 + time.len() + 1 + branch_display_len + id_display_len;
    let mut top_spans = vec![
        Span::styled("┌─ ", border_style),
        Span::styled(status_icon.0, status_icon.1),
        Span::styled(time, Style::default().fg(theme.block_timestamp)),
    ];
    if !branch_label.is_empty() {
        top_spans.push(Span::styled(branch_label, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
    }
    if !id_label.is_empty() {
        top_spans.push(Span::styled(id_label, Style::default().fg(Color::DarkGray)));
    }
    top_spans.push(Span::styled(format!(" {}", hrule(width, top_prefix_len)), border_style));
    lines.push(Line::from(top_spans));

    // ── User prompt ──────────────────────────────────
    for (i, line) in block.prompt.lines().enumerate() {
        let mut spans = vec![Span::styled("│ ", border_style)];
        if i == 0 {
            spans.push(Span::styled("❯ ", Style::default().fg(theme.user_msg).add_modifier(Modifier::BOLD)));
        } else {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(line.to_string(), Style::default().fg(theme.user_msg)));
        lines.push(Line::from(spans));
    }

    // ── Divider ──────────────────────────────────────
    // "│ " (2) before the dotted rule
    lines.push(Line::from(vec![
        Span::styled("│ ", border_style),
        Span::styled(hrule_dotted(width, 2), Style::default().fg(theme.block_border)),
    ]));

    if block.collapsed {
        // ── Collapsed summary ────────────────────────
        lines.push(Line::from(vec![
            Span::styled("│ ", border_style),
            Span::styled(block.summary(), Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC)),
        ]));
    } else {
        // ── Expanded responses ───────────────────────
        for msg in &block.responses {
            if !show_thinking && msg.role == MessageRole::Thinking {
                continue;
            }
            render_response_message(&mut lines, msg, border_style, theme, active_tools, progress, streaming_outputs, tick);
        }
    }

    // ── Branch point indicator (above bottom border) ─
    if let Some(info) = &branch_info
        && info.children_count > 1
    {
        // Header line
        let label = format!("├─ {} branches diverge ─", info.children_count);
        lines.push(Line::from(vec![
            Span::styled("│ ", border_style),
            Span::styled(
                label,
                Style::default().fg(Color::Yellow).add_modifier(Modifier::DIM),
            ),
        ]));
        // Show each child branch with a preview of its prompt
        for (i, (child_id, preview, is_active)) in info.child_branch_previews.iter().enumerate() {
            let connector = if i + 1 < info.child_branch_previews.len() {
                "│ ├─"
            } else {
                "│ └─"
            };
            let marker = if *is_active { " *" } else { "" };
            let style = if *is_active {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let id_str = if info.show_id {
                format!(" #{}", child_id)
            } else {
                String::new()
            };
            lines.push(Line::from(vec![
                Span::styled(connector, Style::default().fg(Color::Yellow).add_modifier(Modifier::DIM)),
                Span::styled(format!("{}{} {}", marker, id_str, preview), style),
            ]));
        }
    }

    // ── Bottom border ────────────────────────────────
    let meta = if block.tokens > 0 {
        format!(" {}tok ", block.tokens)
    } else {
        String::new()
    };
    // "└─" (2) + meta
    let bot_prefix_len = 2 + meta.len();
    lines.push(Line::from(vec![
        Span::styled("└─", border_style),
        Span::styled(meta, Style::default().fg(theme.block_timestamp)),
        Span::styled(hrule(width, bot_prefix_len), border_style),
    ]));

    // Blank line between blocks
    lines.push(Line::from(""));

    lines
}

/// Render a single response message (assistant text, tool call, tool result, thinking).
///
/// `active_tools` and `tick` enable live-streaming rendering for in-progress tool output:
/// a spinner, elapsed time, and scrollable output from the streaming buffer.
pub fn render_response_message<'a>(
    lines: &mut Vec<Line<'a>>,
    msg: &DisplayMessage,
    border_style: Style,
    theme: &Theme,
    active_tools: &HashMap<String, ActiveToolExecution>,
    progress: &ProgressRenderer,
    streaming_outputs: &mut StreamingOutputManager,
    tick: u64,
) {
    match msg.role {
        MessageRole::Assistant => {
            let md_style = MarkdownStyle::from_theme(theme, Style::default().fg(theme.assistant_msg));
            let md_lines = render_markdown(&msg.content, &md_style);
            for md_line in md_lines {
                let mut spans = vec![Span::styled("│ ", border_style), Span::raw("  ")];
                spans.extend(md_line.spans);
                lines.push(Line::from(spans));
            }
        }
        MessageRole::ToolCall => {
            let name = msg.tool_name.as_deref().unwrap_or(&msg.content);
            lines.push(Line::from(vec![
                Span::styled("│ ", border_style),
                Span::styled(format!("  🔧 {}", name), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ]));
        }
        MessageRole::ToolResult => {
            // Check if this is an in-progress tool result (tool_name is Some(call_id) while streaming)
            let is_streaming = msg.tool_name.as_ref().is_some_and(|id| active_tools.contains_key(id.as_str()));

            if is_streaming {
                // Safe: is_streaming checks tool_name.is_some() above
                let call_id = msg.tool_name.as_ref().expect("checked in is_streaming");
                let active = &active_tools[call_id.as_str()];
                let elapsed = format_elapsed(active.started_at.elapsed().as_secs());

                // Spinner + elapsed time header, with structured progress if available
                if let Some(progress_spans) = progress.render_inline(call_id, tick) {
                    let mut spans = vec![
                        Span::styled("│ ", border_style),
                        Span::raw("  "),
                    ];
                    spans.extend(progress_spans);
                    spans.push(Span::styled(format!(" ({})", elapsed), Style::default().fg(Color::DarkGray)));
                    lines.push(Line::from(spans));
                } else {
                    let spinner = SPINNER[(tick as usize / 3) % SPINNER.len()];
                    lines.push(Line::from(vec![
                        Span::styled("│ ", border_style),
                        Span::styled(format!("  {} running ", spinner), Style::default().fg(Color::Yellow)),
                        Span::styled(format!("({})", elapsed), Style::default().fg(Color::DarkGray)),
                    ]));
                }

                // Render scrollable streaming output from the buffer.
                // Focused tools show more lines; unfocused show a compact view.
                if let Some(output) = streaming_outputs.get_mut(call_id) {
                    let visible = if output.focused {
                        output.render_lines(FOCUSED_OUTPUT_LINES, border_style)
                    } else {
                        output.render_lines(LIVE_OUTPUT_MAX_LINES, border_style)
                    };
                    lines.extend(visible);

                    // Stats footer for focused or large outputs.
                    if output.focused || output.total_lines() > LIVE_OUTPUT_MAX_LINES {
                        lines.push(output.render_stats(border_style));
                    }
                } else {
                    // Fallback: no streaming buffer yet, show raw content tail.
                    let all_lines: Vec<&str> = msg.content.lines().collect();
                    let total = all_lines.len();
                    let skip = total.saturating_sub(LIVE_OUTPUT_MAX_LINES);

                    if skip > 0 {
                        lines.push(Line::from(vec![
                            Span::styled("│ ", border_style),
                            Span::styled(
                                format!("  ┄ {} lines above ┄", skip),
                                Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
                            ),
                        ]));
                    }

                    let output_style = Style::default().fg(Color::DarkGray);
                    for line in &all_lines[skip..] {
                        lines.push(Line::from(vec![
                            Span::styled("│ ", border_style),
                            Span::styled(format!("  │ {}", line), output_style),
                        ]));
                    }
                }
            } else {
                // Completed tool result — normal rendering
                let color = if msg.is_error { theme.error } else { Color::DarkGray };
                for line in msg.content.lines() {
                    lines.push(Line::from(vec![
                        Span::styled("│ ", border_style),
                        Span::styled(format!("  → {}", line), Style::default().fg(color)),
                    ]));
                }

                // Render inline image placeholders
                for (i, img) in msg.images.iter().enumerate() {
                    let size_bytes = img.data.len() * 3 / 4; // approximate decoded size
                    let size_str = if size_bytes >= 1024 * 1024 {
                        format!("{:.1} MB", size_bytes as f64 / (1024.0 * 1024.0))
                    } else if size_bytes >= 1024 {
                        format!("{:.1} KB", size_bytes as f64 / 1024.0)
                    } else {
                        format!("{} bytes", size_bytes)
                    };
                    let label = format!(
                        "  🖼 [image {}: {}, {}]",
                        i + 1,
                        img.media_type,
                        size_str,
                    );
                    lines.push(Line::from(vec![
                        Span::styled("│ ", border_style),
                        Span::styled(label, Style::default().fg(Color::Cyan)),
                    ]));
                }
            }
        }
        MessageRole::Thinking => {
            let style = Style::default().fg(theme.thinking_msg).add_modifier(Modifier::DIM);
            for (i, line) in msg.content.lines().enumerate() {
                let prefix = if i == 0 { "  thinking: " } else { "            " };
                lines.push(Line::from(vec![
                    Span::styled("│ ", border_style),
                    Span::styled(format!("{}{}", prefix, line), style),
                ]));
            }
        }
        _ => {
            for line in msg.content.lines() {
                lines.push(Line::from(vec![
                    Span::styled("│ ", border_style),
                    Span::styled(format!("  {}", line), Style::default().fg(theme.fg)),
                ]));
            }
        }
    }
}

/// Render a standalone system message
pub fn render_system_message<'a>(msg: &DisplayMessage, theme: &Theme) -> Vec<Line<'a>> {
    let color = if msg.is_error { theme.error } else { theme.system_msg };
    let mut lines = Vec::new();
    for line in msg.content.lines() {
        lines.push(Line::from(Span::styled(format!("  {}", line), Style::default().fg(color))));
    }
    lines.push(Line::from(""));
    lines
}

/// Render the active (still-streaming) block
pub fn render_active_block<'a>(
    block: &ConversationBlock,
    streaming_thinking: &str,
    streaming_text: &str,
    show_thinking: bool,
    theme: &Theme,
    width: usize,
    active_tools: &HashMap<String, ActiveToolExecution>,
    progress: &ProgressRenderer,
    streaming_outputs: &mut StreamingOutputManager,
    tick: u64,
) -> Vec<Line<'a>> {
    let border_color = theme.block_border_focused;
    let border_style = Style::default().fg(border_color);

    let mut lines = Vec::new();
    let time = block.timestamp.format("%H:%M:%S").to_string();

    // ── Top border ──────────────────────────────────
    let top_prefix_len = 3 + 2 + time.len() + 1;
    lines.push(Line::from(vec![
        Span::styled("┌─ ", border_style),
        Span::styled("… ", Style::default().fg(Color::Yellow)),
        Span::styled(time, Style::default().fg(theme.block_timestamp)),
        Span::styled(format!(" {}", hrule(width, top_prefix_len)), border_style),
    ]));

    // ── User prompt ──────────────────────────────────
    for (i, line) in block.prompt.lines().enumerate() {
        let mut spans = vec![Span::styled("│ ", border_style)];
        if i == 0 {
            spans.push(Span::styled("❯ ", Style::default().fg(theme.user_msg).add_modifier(Modifier::BOLD)));
        } else {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(line.to_string(), Style::default().fg(theme.user_msg)));
        lines.push(Line::from(spans));
    }

    // ── Divider ──────────────────────────────────────
    lines.push(Line::from(vec![
        Span::styled("│ ", border_style),
        Span::styled(hrule_dotted(width, 2), Style::default().fg(theme.block_border)),
    ]));

    // ── Already-committed responses ──────────────────
    for msg in &block.responses {
        if !show_thinking && msg.role == MessageRole::Thinking {
            continue;
        }
        render_response_message(&mut lines, msg, border_style, theme, active_tools, progress, streaming_outputs, tick);
    }

    // ── Streaming thinking ───────────────────────────
    if !streaming_thinking.is_empty() && show_thinking {
        let style = Style::default().fg(theme.thinking_msg).add_modifier(Modifier::DIM);
        for (i, line) in streaming_thinking.lines().enumerate() {
            let prefix = if i == 0 { "  thinking: " } else { "            " };
            lines.push(Line::from(vec![
                Span::styled("│ ", border_style),
                Span::styled(format!("{}{}", prefix, line), style),
            ]));
        }
    }

    // ── Streaming text (with live markdown rendering) ─
    if !streaming_text.is_empty() {
        let md_style = MarkdownStyle::from_theme(theme, Style::default().fg(theme.assistant_msg));
        let md_lines = render_markdown(streaming_text, &md_style);
        for md_line in md_lines {
            let mut spans = vec![Span::styled("│ ", border_style), Span::raw("  ")];
            spans.extend(md_line.spans);
            lines.push(Line::from(spans));
        }
    }

    // ── Bottom border (open — still streaming) ───────
    // Display widths: "└─ " = 3, "streaming…" = 10, " " = 1
    let label = "streaming…";
    let label_display_width = 10; // "streaming" (9) + "…" (1)
    let bot_prefix_len = 3 + label_display_width + 1;
    lines.push(Line::from(vec![
        Span::styled("└─ ", border_style),
        Span::styled(label, Style::default().fg(Color::Yellow).add_modifier(Modifier::DIM)),
        Span::styled(format!(" {}", hrule(width, bot_prefix_len)), border_style),
    ]));
    lines.push(Line::from(""));

    lines
}
