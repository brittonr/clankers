//! Block-oriented message rendering — Warp-style terminal blocks

mod helpers;
mod render;

use std::collections::HashMap;

use helpers::char_to_byte;
use helpers::slice_visible_window;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;
use render::render_active_block;
use render::render_conversation_block;
use render::render_system_message;
use unicode_width::UnicodeWidthStr;

use super::block::BlockEntry;
use super::block::ConversationBlock;
use super::progress_renderer::ProgressRenderer;
use super::streaming_output::StreamingOutputManager;
use crate::app::ActiveToolExecution;
use crate::components::messages::MessageScroll;
use crate::panel::render_scrollbar;
use crate::selection::TextSelection;
use crate::theme::Theme;

/// Branch metadata passed to the block renderer.
#[derive(Debug, Clone)]
pub struct BlockBranchInfo {
    /// Sibling index among blocks sharing the same parent (0-based)
    pub sibling_index: usize,
    /// Total number of sibling blocks at this level
    pub sibling_total: usize,
    /// Number of child blocks branching from this block (0 = leaf, >1 = branch point)
    pub children_count: usize,
    /// Whether to show block IDs in the header
    pub show_id: bool,
    /// Names/previews of the child branches diverging from this block.
    /// Empty if not a branch point. Each entry is the first prompt of that child branch.
    pub child_branch_previews: Vec<(usize, String, bool)>, // (block_id, preview, is_active)
}

/// Maximum lines of tool output to show while streaming (compact view)
const LIVE_OUTPUT_MAX_LINES: usize = 8;

/// Lines of tool output to show when the tool is focused for scrolling
const FOCUSED_OUTPUT_LINES: usize = 32;

/// Spinner characters for animated indicators
const SPINNER: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Render all blocks into the messages area.
/// Returns the plain-text lines that were rendered (for selection extraction).
/// `branch_info` maps block_id → `BlockBranchInfo` with sibling, children, and ID display metadata.
#[allow(clippy::too_many_arguments)]
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(function_length, reason = "sequential setup/dispatch logic")
)]
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
    branch_info: &std::collections::HashMap<usize, BlockBranchInfo>,
    search: &super::output_search::OutputSearch,
    search_scroll_target: Option<usize>,
    active_tools: &HashMap<String, ActiveToolExecution>,
    progress: &ProgressRenderer,
    streaming_outputs: &mut StreamingOutputManager,
    tick: u64,
    highlighter: &dyn clanker_tui_types::SyntaxHighlighter,
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
                let info = branch_info.get(&block.id).cloned();
                render_conversation_block(
                    block,
                    is_focused,
                    show_thinking,
                    theme,
                    inner_width,
                    info,
                    active_tools,
                    progress,
                    streaming_outputs,
                    tick,
                    highlighter,
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
            progress,
            streaming_outputs,
            tick,
            highlighter,
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

    // Scrollbar for messages area
    let messages_inner = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };
    render_scrollbar(frame, messages_inner, total_visual_lines, scroll.offset, visible_height);

    plain_lines
}

#[cfg(test)]
mod tests {
    use helpers::format_elapsed;
    use render::render_response_message;

    use super::*;
    use crate::app::DisplayMessage;
    use crate::app::MessageRole;

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
    fn streaming_tool_result_shows_output_from_buffer() {
        // Build a DisplayMessage with tool_name set (indicating in-progress)
        let msg = DisplayMessage {
            role: MessageRole::ToolResult,
            content: (1..=20).map(|i| format!("line {}", i)).collect::<Vec<_>>().join("\n"),
            tool_name: Some("call_123".to_string()),
            tool_input: None,
            is_error: false,
            images: Vec::new(),
        };

        // Create an active tool entry
        let mut active_tools = HashMap::new();
        active_tools.insert("call_123".to_string(), ActiveToolExecution {
            tool_name: "bash".to_string(),
            started_at: std::time::Instant::now(),
            line_count: 20,
        });

        // Feed the same lines into the streaming output manager
        let mut streaming_outputs = StreamingOutputManager::new();
        for i in 1..=20 {
            streaming_outputs.add_line("call_123", &format!("line {}", i));
        }

        let theme = Theme::dark();
        let progress = ProgressRenderer::new();
        let border_style = Style::default().fg(Color::DarkGray);
        let mut lines = Vec::new();
        render_response_message(
            &mut lines,
            &msg,
            border_style,
            &theme,
            &active_tools,
            &progress,
            &mut streaming_outputs,
            0,
            &clanker_tui_types::PlainHighlighter,
        );

        let plain: Vec<String> = lines.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect()).collect();

        // First line should contain "running" (no structured progress → fallback)
        assert!(plain[0].contains("running"), "expected spinner header, got: {}", plain[0]);

        // Should show LIVE_OUTPUT_MAX_LINES of output via the streaming buffer
        let output_lines: Vec<_> = plain.iter().filter(|l| l.contains("│ line")).collect();
        assert_eq!(output_lines.len(), LIVE_OUTPUT_MAX_LINES);

        // Last visible output line should be "line 20"
        let last_output = output_lines.last().unwrap();
        assert!(last_output.contains("line 20"), "expected line 20, got: {}", last_output);

        // Should have a stats footer (output > LIVE_OUTPUT_MAX_LINES)
        assert!(plain.iter().any(|l| l.contains("20 lines")), "expected stats footer");
    }

    #[test]
    fn completed_tool_result_shows_all_lines() {
        let msg = DisplayMessage {
            role: MessageRole::ToolResult,
            content: "line 1\nline 2\nline 3".to_string(),
            tool_name: None, // completed
            tool_input: None,
            is_error: false,
            images: Vec::new(),
        };

        let active_tools = HashMap::new();
        let progress = ProgressRenderer::new();
        let mut streaming_outputs = StreamingOutputManager::new();
        let theme = Theme::dark();
        let border_style = Style::default().fg(Color::DarkGray);
        let mut lines = Vec::new();
        render_response_message(
            &mut lines,
            &msg,
            border_style,
            &theme,
            &active_tools,
            &progress,
            &mut streaming_outputs,
            0,
            &clanker_tui_types::PlainHighlighter,
        );

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
            tool_input: None,
            is_error: false,
            images: Vec::new(),
        };

        let mut active_tools = HashMap::new();
        active_tools.insert("call_456".to_string(), ActiveToolExecution {
            tool_name: "bash".to_string(),
            started_at: std::time::Instant::now(),
            line_count: 1,
        });

        let mut streaming_outputs = StreamingOutputManager::new();
        streaming_outputs.add_line("call_456", "short output");

        let theme = Theme::dark();
        let progress = ProgressRenderer::new();
        let border_style = Style::default().fg(Color::DarkGray);
        let mut lines = Vec::new();
        render_response_message(
            &mut lines,
            &msg,
            border_style,
            &theme,
            &active_tools,
            &progress,
            &mut streaming_outputs,
            0,
            &clanker_tui_types::PlainHighlighter,
        );

        let plain: Vec<String> = lines.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect()).collect();

        // Should have spinner header + 1 output line, no "lines above"
        assert!(plain[0].contains("running"));
        assert!(!plain.iter().any(|l| l.contains("lines above")));
        assert!(plain[1].contains("short output"));
    }

    #[test]
    fn streaming_with_structured_progress_shows_bar() {
        use clanker_tui_types::ToolProgress;

        let msg = DisplayMessage {
            role: MessageRole::ToolResult,
            content: "output line".to_string(),
            tool_name: Some("call_progress".to_string()),
            tool_input: None,
            is_error: false,
            images: Vec::new(),
        };

        let mut active_tools = HashMap::new();
        active_tools.insert("call_progress".to_string(), ActiveToolExecution {
            tool_name: "bash".to_string(),
            started_at: std::time::Instant::now(),
            line_count: 1,
        });

        // Set up progress renderer with structured progress
        let mut progress = ProgressRenderer::new();
        progress.update("call_progress", ToolProgress::lines(42, None));

        let mut streaming_outputs = StreamingOutputManager::new();
        streaming_outputs.add_line("call_progress", "output line");

        let theme = Theme::dark();
        let border_style = Style::default().fg(Color::DarkGray);
        let mut lines = Vec::new();
        render_response_message(
            &mut lines,
            &msg,
            border_style,
            &theme,
            &active_tools,
            &progress,
            &mut streaming_outputs,
            0,
            &clanker_tui_types::PlainHighlighter,
        );

        let plain: Vec<String> = lines.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect()).collect();

        // First line should show structured progress (not just "running")
        assert!(plain[0].contains("42"), "expected line count in progress: {}", plain[0]);
        assert!(plain[0].contains("lines"), "expected unit in progress: {}", plain[0]);
        // Should NOT contain "running" fallback
        assert!(!plain[0].contains("running"), "should use structured progress, not fallback: {}", plain[0]);
    }

    #[test]
    fn streaming_with_progress_bar_and_total() {
        use clanker_tui_types::ToolProgress;

        let msg = DisplayMessage {
            role: MessageRole::ToolResult,
            content: "data".to_string(),
            tool_name: Some("call_dl".to_string()),
            tool_input: None,
            is_error: false,
            images: Vec::new(),
        };

        let mut active_tools = HashMap::new();
        active_tools.insert("call_dl".to_string(), ActiveToolExecution {
            tool_name: "web".to_string(),
            started_at: std::time::Instant::now(),
            line_count: 0,
        });

        let mut progress = ProgressRenderer::new();
        progress.update("call_dl", ToolProgress::bytes(500, Some(1000)));

        let mut streaming_outputs = StreamingOutputManager::new();
        streaming_outputs.add_line("call_dl", "data");

        let theme = Theme::dark();
        let border_style = Style::default().fg(Color::DarkGray);
        let mut lines = Vec::new();
        render_response_message(
            &mut lines,
            &msg,
            border_style,
            &theme,
            &active_tools,
            &progress,
            &mut streaming_outputs,
            0,
            &clanker_tui_types::PlainHighlighter,
        );

        let plain: Vec<String> = lines.iter().map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect()).collect();

        // Should contain a progress bar
        assert!(plain[0].contains("█"), "expected progress bar: {}", plain[0]);
        assert!(plain[0].contains("500/1000"), "expected byte count: {}", plain[0]);
    }
}
