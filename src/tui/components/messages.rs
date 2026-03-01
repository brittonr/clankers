//! Message display with scrolling

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;

use crate::tui::app::DisplayMessage;
use crate::tui::app::MessageRole;
use crate::tui::selection::TextSelection;
use crate::tui::theme::Theme;

/// Scroll state for message view
#[derive(Debug, Clone)]
pub struct MessageScroll {
    pub offset: usize,
    pub auto_scroll: bool,
}

impl Default for MessageScroll {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageScroll {
    pub fn new() -> Self {
        Self {
            offset: 0,
            auto_scroll: true,
        }
    }

    pub fn scroll_up(&mut self, amount: usize) {
        self.offset = self.offset.saturating_sub(amount);
        self.auto_scroll = false;
    }

    pub fn scroll_down(&mut self, amount: usize) {
        self.offset = self.offset.saturating_add(amount);
        // auto_scroll is re-enabled during render when we detect we're at the bottom
    }

    pub fn scroll_to_top(&mut self) {
        self.offset = 0;
        self.auto_scroll = false;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.auto_scroll = true;
    }
}

/// Render messages with optional streaming text.
/// Returns the plain-text lines that were rendered (for selection extraction).
pub fn render_messages(
    frame: &mut Frame,
    messages: &[DisplayMessage],
    streaming_thinking: &str,
    streaming_text: &str,
    theme: &Theme,
    scroll: &mut MessageScroll,
    selection: &Option<TextSelection>,
    area: Rect,
) -> Vec<String> {
    let mut lines: Vec<Line> = Vec::new();
    let mut plain_lines: Vec<String> = Vec::new();

    for msg in messages {
        let (prefix, style) = match msg.role {
            MessageRole::User => ("You: ", Style::default().fg(theme.user_msg)),
            MessageRole::Assistant => ("Assistant: ", Style::default().fg(theme.assistant_msg)),
            MessageRole::System => ("System: ", Style::default().fg(theme.system_msg)),
            MessageRole::ToolCall => {
                let name = msg.tool_name.as_deref().unwrap_or("unknown");
                let text = format!("🔧 {}", name);
                lines.push(Line::from(Span::styled(
                    text.clone(),
                    Style::default().fg(theme.system_msg).add_modifier(Modifier::BOLD),
                )));
                plain_lines.push(text);
                continue;
            }
            MessageRole::ToolResult => {
                let color = if msg.is_error { theme.error } else { theme.system_msg };
                ("  → ", Style::default().fg(color))
            }
            MessageRole::Thinking => ("💭 ", Style::default().fg(theme.thinking_msg).add_modifier(Modifier::DIM)),
        };

        lines.push(Line::from(Span::styled(prefix, style.add_modifier(Modifier::BOLD))));
        plain_lines.push(prefix.to_string());

        // Tool results may contain ANSI-colored diffs — render them with
        // proper colors instead of dumping raw escape sequences.
        let has_ansi = msg.content.contains("\x1b[");
        if has_ansi {
            for ansi_line in crate::util::ansi::ansi_to_lines(&msg.content) {
                let plain = crate::util::ansi::strip_ansi(
                    &ansi_line.spans.iter().map(|s| s.content.as_ref()).collect::<String>(),
                );
                plain_lines.push(plain);
                lines.push(ansi_line);
            }
        } else {
            for line in msg.content.lines() {
                lines.push(Line::from(Span::styled(line, style)));
                plain_lines.push(line.to_string());
            }
        }
        lines.push(Line::from(""));
        plain_lines.push(String::new());
    }

    // Add streaming thinking if present
    if !streaming_thinking.is_empty() {
        let thinking_style = Style::default().fg(theme.thinking_msg).add_modifier(Modifier::DIM);
        let header = "💭 Thinking...";
        lines.push(Line::from(Span::styled(header, thinking_style.add_modifier(Modifier::BOLD))));
        plain_lines.push(header.to_string());
        for line in streaming_thinking.lines() {
            lines.push(Line::from(Span::styled(line, thinking_style)));
            plain_lines.push(line.to_string());
        }
        lines.push(Line::from(""));
        plain_lines.push(String::new());
    }

    // Add streaming text if present
    if !streaming_text.is_empty() {
        let header = "Assistant: ";
        lines.push(Line::from(Span::styled(
            header,
            Style::default().fg(theme.assistant_msg).add_modifier(Modifier::BOLD),
        )));
        plain_lines.push(header.to_string());
        for line in streaming_text.lines() {
            lines.push(Line::from(Span::styled(line, Style::default().fg(theme.assistant_msg))));
            plain_lines.push(line.to_string());
        }
    }

    // Apply selection highlighting
    let highlight_style = Style::default().bg(theme.highlight).fg(theme.bg);
    if let Some(sel) = selection
        && !sel.is_empty()
    {
        for (row, line) in lines.iter_mut().enumerate() {
            let plain_len = plain_lines.get(row).map(|s| s.len()).unwrap_or(0);
            if let Some((col_start, col_end)) = sel.col_range_for_row(row, plain_len) {
                // Rebuild this line with highlighting applied
                let plain = &plain_lines[row];
                let mut spans = Vec::new();

                // Grab the base style from the existing line
                let base_style = line.spans.first().map(|s| s.style).unwrap_or_default();

                // Byte offsets from column indices (handle char boundaries)
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

    // Auto-scroll: compute offset to show the bottom of content.
    // We must count *visual* lines (after wrapping) not logical lines,
    // since Paragraph::wrap will break long lines across multiple rows.
    let visible_height = area.height.saturating_sub(2) as usize; // subtract border
    let inner_width = area.width.saturating_sub(2) as usize; // subtract border
    let total_visual_lines: usize = if inner_width == 0 {
        lines.len()
    } else {
        lines
            .iter()
            .map(|line| {
                // Use unicode display width to match ratatui's Wrap behavior
                let line_width: usize =
                    line.spans.iter().map(|s| unicode_width::UnicodeWidthStr::width(s.content.as_ref())).sum();
                if line_width == 0 {
                    1
                } else {
                    line_width.div_ceil(inner_width)
                }
            })
            .sum()
    };
    let max_offset = total_visual_lines.saturating_sub(visible_height);
    if scroll.auto_scroll {
        scroll.offset = max_offset;
    } else {
        // Clamp scroll offset so we don't scroll past content
        scroll.offset = scroll.offset.min(max_offset);
        // Re-enable auto-scroll if user scrolled to the bottom
        if scroll.offset >= max_offset {
            scroll.auto_scroll = true;
        }
    }

    // Pre-slice lines to work around ratatui's u16 scroll limit (max 65535).
    let inner_width = area.width.saturating_sub(2) as usize;
    let (visible_lines, residual_offset) =
        super::block_view::slice_visible_window(&lines, scroll.offset, visible_height, inner_width);

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
