//! Block-oriented message rendering — Warp-style terminal blocks

use std::collections::HashMap;

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use unicode_width::UnicodeWidthStr;

use super::block::BlockEntry;
use super::block::ConversationBlock;
use super::markdown::MarkdownStyle;
use super::markdown::render_markdown;
use crate::tui::app::ActiveToolExecution;
use crate::tui::app::DisplayMessage;
use crate::tui::app::MessageRole;
use crate::tui::components::messages::MessageScroll;
use crate::tui::selection::TextSelection;
use crate::tui::theme::Theme;

/// Maximum lines of tool output to show while streaming (tail window)
const LIVE_OUTPUT_MAX_LINES: usize = 8;

/// Spinner characters for animated indicators
const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Format a duration as human-readable elapsed time
fn format_elapsed(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else {
        format!("{}m{:02}s", secs / 60, secs % 60)
    }
}

/// Build a horizontal rule of `─` that fills the remaining width.
/// `used` is how many columns are already consumed by the prefix on this line.
fn hrule(width: usize, used: usize) -> String {
    let remaining = width.saturating_sub(used);
    "─".repeat(remaining)
}

/// Build a horizontal rule of `┄` that fills the remaining width.
fn hrule_dotted(width: usize, used: usize) -> String {
    let remaining = width.saturating_sub(used);
    "┄".repeat(remaining)
}

/// Render a single conversation block into lines.
/// `siblings` is `Some((current_index, total))` when there are multiple branches.
fn render_conversation_block<'a>(
    block: &ConversationBlock,
    focused: bool,
    show_thinking: bool,
    theme: &Theme,
    width: usize,
    siblings: Option<(usize, usize)>,
    active_tools: &HashMap<String, ActiveToolExecution>,
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
    let branch_label = match siblings {
        Some((idx, total)) if total > 1 => {
            format!(" ◂ {}/{} ▸", idx + 1, total)
        }
        _ => String::new(),
    };
    let branch_display_len = branch_label.len();

    // "┌─ " (3) + icon (varies) + time (8) + " " (1) + branch_label
    let top_prefix_len = 3 + status_icon.2 + time.len() + 1 + branch_display_len;
    let mut top_spans = vec![
        Span::styled("┌─ ", border_style),
        Span::styled(status_icon.0, status_icon.1),
        Span::styled(time, Style::default().fg(theme.block_timestamp)),
    ];
    if !branch_label.is_empty() {
        top_spans.push(Span::styled(branch_label, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
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
            render_response_message(&mut lines, msg, border_style, theme, active_tools, tick);
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
/// a spinner, elapsed time, and a tail window of the last N lines.
fn render_response_message<'a>(
    lines: &mut Vec<Line<'a>>,
    msg: &DisplayMessage,
    border_style: Style,
    theme: &Theme,
    active_tools: &HashMap<String, ActiveToolExecution>,
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
                let spinner = SPINNER[(tick as usize / 3) % SPINNER.len()];
                let elapsed = format_elapsed(active.started_at.elapsed().as_secs());

                // Spinner + elapsed time header
                lines.push(Line::from(vec![
                    Span::styled("│ ", border_style),
                    Span::styled(format!("  {} running ", spinner), Style::default().fg(Color::Yellow)),
                    Span::styled(format!("({})", elapsed), Style::default().fg(Color::DarkGray)),
                ]));

                // Show only the last N lines (tail window)
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
            } else {
                // Completed tool result — normal rendering
                let color = if msg.is_error { theme.error } else { Color::DarkGray };
                for line in msg.content.lines() {
                    lines.push(Line::from(vec![
                        Span::styled("│ ", border_style),
                        Span::styled(format!("  → {}", line), Style::default().fg(color)),
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
fn render_system_message<'a>(msg: &DisplayMessage, theme: &Theme) -> Vec<Line<'a>> {
    let color = if msg.is_error { theme.error } else { theme.system_msg };
    let mut lines = Vec::new();
    for line in msg.content.lines() {
        lines.push(Line::from(Span::styled(format!("  {}", line), Style::default().fg(color))));
    }
    lines.push(Line::from(""));
    lines
}

/// Render the active (still-streaming) block
fn render_active_block<'a>(
    block: &ConversationBlock,
    streaming_thinking: &str,
    streaming_text: &str,
    show_thinking: bool,
    theme: &Theme,
    width: usize,
    active_tools: &HashMap<String, ActiveToolExecution>,
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
        render_response_message(&mut lines, msg, border_style, theme, active_tools, tick);
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

/// Render all blocks into the messages area.
/// Returns the plain-text lines that were rendered (for selection extraction).
/// `sibling_info` maps block_id → (current_index, total_siblings).
#[allow(clippy::too_many_arguments)]
pub fn render_blocks(
    frame: &mut Frame,
    blocks: &[BlockEntry],
    focused_block: Option<usize>,
    active_block: Option<&ConversationBlock>,
    streaming_thinking: &str,
    streaming_text: &str,
    show_thinking: bool,
    theme: &Theme,
    scroll: &mut MessageScroll,
    selection: &Option<TextSelection>,
    area: Rect,
    sibling_info: &std::collections::HashMap<usize, (usize, usize)>,
    search: &super::output_search::OutputSearch,
    search_scroll_target: Option<usize>,
    active_tools: &HashMap<String, ActiveToolExecution>,
    tick: u64,
) -> Vec<String> {
    // Inner width of the Paragraph (inside the outer border)
    let inner_width = area.width.saturating_sub(2) as usize;

    let mut lines: Vec<Line> = Vec::new();
    let mut plain_lines: Vec<String> = Vec::new();

    // Track the line range of the focused block so we can scroll to it
    let mut focused_line_start: Option<usize> = None;
    let mut focused_line_end: Option<usize> = None;

    // Render completed blocks
    for entry in blocks {
        let is_focused_block = match entry {
            BlockEntry::Conversation(block) => focused_block == Some(block.id),
            _ => false,
        };
        let start_line = lines.len();
        let block_lines = match entry {
            BlockEntry::Conversation(block) => {
                let is_focused = focused_block == Some(block.id);
                let siblings = sibling_info.get(&block.id).copied();
                render_conversation_block(
                    block,
                    is_focused,
                    show_thinking,
                    theme,
                    inner_width,
                    siblings,
                    active_tools,
                    tick,
                )
            }
            BlockEntry::System(msg) => render_system_message(msg, theme),
        };
        for line in block_lines {
            let plain: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            plain_lines.push(plain);
            lines.push(line);
        }
        if is_focused_block {
            focused_line_start = Some(start_line);
            focused_line_end = Some(lines.len());
        }
    }

    // Render the active (streaming) block
    if let Some(active) = active_block {
        let block_lines = render_active_block(
            active,
            streaming_thinking,
            streaming_text,
            show_thinking,
            theme,
            inner_width,
            active_tools,
            tick,
        );
        for line in block_lines {
            let plain: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
            plain_lines.push(plain);
            lines.push(line);
        }
    }

    // Save original base styles before any highlighting modifies spans
    let original_base_styles: Vec<Style> =
        lines.iter().map(|l| l.spans.first().map(|s| s.style).unwrap_or_default()).collect();

    // Apply search match highlighting
    let match_style = Style::default().bg(theme.search_match).fg(Color::White);
    let current_match_style = Style::default().bg(theme.search_current).fg(Color::Black);
    super::output_search::apply_search_highlights(&mut lines, &plain_lines, search, match_style, current_match_style);

    // Apply selection highlighting (uses original base styles so it layers correctly)
    let highlight_style = Style::default().bg(theme.highlight).fg(theme.bg);
    if let Some(sel) = selection.as_ref().filter(|s| !s.is_empty()) {
        for (row, line) in lines.iter_mut().enumerate() {
            let plain_len = plain_lines.get(row).map(|s| s.len()).unwrap_or(0);
            if let Some((col_start, col_end)) = sel.col_range_for_row(row, plain_len) {
                let plain = &plain_lines[row];
                let base_style = original_base_styles.get(row).copied().unwrap_or_default();
                let mut spans = Vec::new();

                let byte_start = char_to_byte(plain, col_start);
                let byte_end = char_to_byte(plain, col_end);

                if byte_start > 0 {
                    spans.push(Span::styled(&plain[..byte_start], base_style));
                }
                spans.push(Span::styled(&plain[byte_start..byte_end], highlight_style));
                if byte_end < plain.len() {
                    spans.push(Span::styled(&plain[byte_end..], base_style));
                }

                *line = Line::from(spans);
            }
        }
    }

    // Helper: compute the number of visual lines for a slice of logical lines.
    // Uses Unicode display width to match ratatui's Wrap behaviour.
    let visual_lines_for = |line_range: std::ops::Range<usize>| -> usize {
        if inner_width == 0 {
            line_range.len()
        } else {
            lines[line_range]
                .iter()
                .map(|line| {
                    let display_width: usize =
                        line.spans.iter().map(|s| UnicodeWidthStr::width(s.content.as_ref())).sum();
                    if display_width == 0 {
                        1
                    } else {
                        display_width.div_ceil(inner_width)
                    }
                })
                .sum()
        }
    };

    // Auto-scroll
    let visible_height = area.height.saturating_sub(2) as usize;
    let total_visual_lines: usize = visual_lines_for(0..lines.len());
    let max_offset = total_visual_lines.saturating_sub(visible_height);
    if scroll.auto_scroll {
        scroll.offset = max_offset;
    } else {
        scroll.offset = scroll.offset.min(max_offset);
        // Only re-enable auto-scroll when at the bottom AND no block is focused,
        // otherwise navigating blocks near the bottom causes scroll fighting.
        if scroll.offset >= max_offset && focused_block.is_none() {
            scroll.auto_scroll = true;
        }
    }

    // Scroll to keep the focused block visible
    if let (Some(start), Some(end)) = (focused_line_start, focused_line_end) {
        let block_visual_start = visual_lines_for(0..start);
        let block_visual_end = block_visual_start + visual_lines_for(start..end);
        let block_height = block_visual_end - block_visual_start;

        if block_height >= visible_height {
            // Block is taller than the viewport — just show its top
            if block_visual_start != scroll.offset {
                scroll.offset = block_visual_start;
                scroll.auto_scroll = false;
            }
        } else if block_visual_start < scroll.offset {
            // Block's top is above the visible area — scroll up
            scroll.offset = block_visual_start;
            scroll.auto_scroll = false;
        } else if block_visual_end > scroll.offset + visible_height {
            // Block's bottom is below the visible area — scroll down
            scroll.offset = block_visual_end.saturating_sub(visible_height);
            scroll.auto_scroll = false;
        }

        scroll.offset = scroll.offset.min(max_offset);
    }

    // Scroll to the current search match (overrides focused-block scroll when requested)
    if let Some(target_row) = search_scroll_target
        && target_row < lines.len()
    {
        let match_visual_start = visual_lines_for(0..target_row);
        if match_visual_start < scroll.offset || match_visual_start >= scroll.offset + visible_height {
            // Match is outside the visible area — center it roughly
            scroll.offset = match_visual_start.saturating_sub(visible_height / 3);
            scroll.offset = scroll.offset.min(max_offset);
            scroll.auto_scroll = false;
        }
    }

    // Pre-slice lines to work around ratatui's u16 scroll limit (max 65535).
    // Instead of passing all lines + a large scroll offset, we find the first
    // logical line that falls within the visible window and only pass a small
    // residual offset to Paragraph::scroll.
    let (visible_lines, residual_offset) = slice_visible_window(&lines, scroll.offset, visible_height, inner_width);

    let paragraph = Paragraph::new(visible_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border))
                .title("Messages"),
        )
        .wrap(Wrap { trim: false })
        .scroll((residual_offset as u16, 0));

    frame.render_widget(paragraph, area);
    plain_lines
}

/// Convert a column index (character offset) to a byte offset in a string.
fn char_to_byte(s: &str, col: usize) -> usize {
    s.char_indices().nth(col).map(|(i, _)| i).unwrap_or(s.len())
}

/// Slice a list of logical lines to only those visible in the current scroll
/// window, returning the sliced lines and a small residual scroll offset.
///
/// This works around ratatui's `u16` scroll limit (max 65,535 visual lines).
/// Instead of passing all lines with a potentially huge offset, we skip logical
/// lines that are entirely above the viewport and only pass a small residual
/// offset for the first partially-visible logical line.
///
/// Returns `(visible_lines, residual_offset)` where `residual_offset` is always
/// small enough to fit in a `u16`.
pub fn slice_visible_window<'a>(
    lines: &[Line<'a>],
    scroll_offset: usize,
    visible_height: usize,
    inner_width: usize,
) -> (Vec<Line<'a>>, usize) {
    if inner_width == 0 || lines.is_empty() {
        return (lines.to_vec(), scroll_offset.min(u16::MAX as usize));
    }

    // Find the first logical line whose visual lines overlap with the viewport.
    let mut visual_pos: usize = 0;
    let mut first_logical = 0;
    let mut residual: usize = 0;

    for (i, line) in lines.iter().enumerate() {
        let display_width: usize = line.spans.iter().map(|s| UnicodeWidthStr::width(s.content.as_ref())).sum();
        let line_visual = if display_width == 0 {
            1
        } else {
            display_width.div_ceil(inner_width)
        };

        if visual_pos + line_visual > scroll_offset {
            // This logical line contains the scroll target
            first_logical = i;
            residual = scroll_offset - visual_pos;
            break;
        }
        visual_pos += line_visual;

        // If we've exhausted all lines without reaching the offset,
        // show the last line
        if i == lines.len() - 1 {
            first_logical = i;
            residual = 0;
        }
    }

    // Take enough logical lines to fill the viewport (with some buffer for
    // wrapped lines). We need at least `visible_height` visual lines past
    // the residual.
    let needed_visual = visible_height + residual;
    let mut collected_visual: usize = 0;
    let mut last_logical = first_logical;

    for line in &lines[first_logical..] {
        let display_width: usize = line.spans.iter().map(|s| UnicodeWidthStr::width(s.content.as_ref())).sum();
        let line_visual = if display_width == 0 {
            1
        } else {
            display_width.div_ceil(inner_width)
        };
        collected_visual += line_visual;
        last_logical += 1;
        if collected_visual >= needed_visual {
            break;
        }
    }

    let sliced = lines[first_logical..last_logical].to_vec();
    (sliced, residual)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_elapsed_seconds() {
        assert_eq!(format_elapsed(0), "0s");
        assert_eq!(format_elapsed(5), "5s");
        assert_eq!(format_elapsed(59), "59s");
    }

    #[test]
    fn format_elapsed_minutes() {
        assert_eq!(format_elapsed(60), "1m00s");
        assert_eq!(format_elapsed(61), "1m01s");
        assert_eq!(format_elapsed(125), "2m05s");
        assert_eq!(format_elapsed(3661), "61m01s");
    }

    #[test]
    fn streaming_tool_result_shows_tail_window() {
        // Build a DisplayMessage with tool_name set (indicating in-progress)
        let msg = DisplayMessage {
            role: MessageRole::ToolResult,
            content: (1..=20).map(|i| format!("line {}", i)).collect::<Vec<_>>().join("\n"),
            tool_name: Some("call_123".to_string()),
            is_error: false,
        };

        // Create an active tool entry
        let mut active_tools = HashMap::new();
        active_tools.insert("call_123".to_string(), ActiveToolExecution {
            tool_name: "bash".to_string(),
            started_at: std::time::Instant::now(),
            line_count: 20,
        });

        let theme = Theme::dark();
        let border_style = Style::default().fg(Color::DarkGray);
        let mut lines = Vec::new();
        render_response_message(&mut lines, &msg, border_style, &theme, &active_tools, 0);

        // Should have: 1 spinner header + 1 "lines above" + LIVE_OUTPUT_MAX_LINES output lines
        let plain: Vec<String> = lines.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect()).collect();

        // First line should contain "running"
        assert!(plain[0].contains("running"), "expected spinner header, got: {}", plain[0]);

        // Second line should mention hidden lines
        assert!(plain[1].contains("lines above"), "expected truncation note, got: {}", plain[1]);

        // Should show exactly LIVE_OUTPUT_MAX_LINES of output
        let output_lines: Vec<_> = plain.iter().filter(|l| l.contains("│ line")).collect();
        assert_eq!(output_lines.len(), LIVE_OUTPUT_MAX_LINES);

        // Last visible line should be "line 20"
        assert!(plain.last().unwrap().contains("line 20"));
    }

    #[test]
    fn completed_tool_result_shows_all_lines() {
        let msg = DisplayMessage {
            role: MessageRole::ToolResult,
            content: "line 1\nline 2\nline 3".to_string(),
            tool_name: None, // completed
            is_error: false,
        };

        let active_tools = HashMap::new();
        let theme = Theme::dark();
        let border_style = Style::default().fg(Color::DarkGray);
        let mut lines = Vec::new();
        render_response_message(&mut lines, &msg, border_style, &theme, &active_tools, 0);

        let plain: Vec<String> = lines.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect()).collect();
        assert_eq!(plain.len(), 3);
        assert!(plain[0].contains("→ line 1"));
        assert!(plain[1].contains("→ line 2"));
        assert!(plain[2].contains("→ line 3"));
    }

    #[test]
    fn short_streaming_output_no_truncation_header() {
        let msg = DisplayMessage {
            role: MessageRole::ToolResult,
            content: "short output".to_string(),
            tool_name: Some("call_456".to_string()),
            is_error: false,
        };

        let mut active_tools = HashMap::new();
        active_tools.insert("call_456".to_string(), ActiveToolExecution {
            tool_name: "bash".to_string(),
            started_at: std::time::Instant::now(),
            line_count: 1,
        });

        let theme = Theme::dark();
        let border_style = Style::default().fg(Color::DarkGray);
        let mut lines = Vec::new();
        render_response_message(&mut lines, &msg, border_style, &theme, &active_tools, 0);

        let plain: Vec<String> = lines.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect()).collect();

        // Should have spinner header + 1 output line, no "lines above"
        assert!(plain[0].contains("running"));
        assert!(!plain.iter().any(|l| l.contains("lines above")));
        assert!(plain[1].contains("short output"));
    }
}
