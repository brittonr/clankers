//! Header / banner

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;

use crate::tui::theme::Theme;

/// Render the application header
pub fn render_header(frame: &mut Frame, model: &str, theme: &Theme, area: Rect) {
    let header = Paragraph::new(vec![Line::from(vec![
        Span::styled(" clankers ", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
        Span::styled(format!(" {} ", model), Style::default().fg(theme.system_msg)),
        Span::styled(
            " Ctrl+C quit  Enter submit  m:model  a:account  Alt+Enter newline",
            Style::default().fg(theme.border),
        ),
    ])]);
    frame.render_widget(header, area);
}
