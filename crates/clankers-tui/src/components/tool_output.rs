//! Tool output display (collapsible)

use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;

/// A tool output block in the chat
pub struct ToolOutputBlock {
    pub tool_name: String,
    pub output: String,
    pub is_error: bool,
    pub collapsed: bool,
}

impl ToolOutputBlock {
    pub fn new(tool_name: String, output: String, is_error: bool) -> Self {
        // Auto-collapse if output is long
        let collapsed = output.lines().count() > 10;
        Self {
            tool_name,
            output,
            is_error,
            collapsed,
        }
    }

    pub fn toggle(&mut self) {
        self.collapsed = !self.collapsed;
    }

    /// Render to lines
    pub fn render(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        let icon = if self.collapsed { "▶" } else { "▼" };
        let status = if self.is_error { "✗" } else { "✓" };
        let color = if self.is_error { Color::Red } else { Color::Green };

        lines.push(Line::from(vec![
            Span::styled(format!("{} ", icon), Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{} ", status), Style::default().fg(color)),
            Span::styled(self.tool_name.clone(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]));

        if !self.collapsed {
            let max_lines = 50;
            for (i, line) in self.output.lines().enumerate() {
                if i >= max_lines {
                    lines.push(Line::from(Span::styled(
                        format!("  ... ({} more lines)", self.output.lines().count() - max_lines),
                        Style::default().fg(Color::DarkGray),
                    )));
                    break;
                }
                lines.push(Line::from(Span::styled(format!("  {}", line), Style::default().fg(Color::DarkGray))));
            }
        }
        lines
    }
}
