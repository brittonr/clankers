//! Confirmation dialog

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

use crate::app::BashConfirmState;

// Re-export from rat-widgets
pub use rat_widgets::confirm::ConfirmDialog;

/// Render a bash confirm dialog for attach mode protocol
pub fn render_bash_confirm(frame: &mut Frame, area: Rect, state: &BashConfirmState) {
    let width = 60.min(area.width.saturating_sub(4));
    let height = 7;
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    let popup = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Bash Confirm ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let cmd_display = if state.command.len() > 50 {
        format!("{}…", &state.command[..49])
    } else {
        state.command.clone()
    };

    let yes_style = if state.approved {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let no_style = if !state.approved {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let content = vec![
        Line::from(format!("dir: {}", state.working_dir)),
        Line::from(format!("cmd: {cmd_display}")),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [Y]es ", yes_style),
            Span::styled("  [N]o ", no_style),
        ]),
        Line::from(Span::styled("  y/n or ←/→ + Enter", Style::default().fg(Color::DarkGray))),
    ];

    let paragraph = Paragraph::new(content).block(block).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, popup);
}
