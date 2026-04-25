//! Settings panel overlay

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

pub struct SettingsPanel {
    pub visible: bool,
    pub entries: Vec<SettingEntry>,
    pub selected: usize,
}

pub struct SettingEntry {
    pub key: String,
    pub value: String,
    pub description: String,
}

impl SettingsPanel {
    pub fn new(entries: Vec<SettingEntry>) -> Self {
        Self {
            visible: false,
            entries,
            selected: 0,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        self.selected = (self.selected + 1).min(self.entries.len().saturating_sub(1));
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if !self.visible {
            return;
        }

        let width = 60.min(area.width.saturating_sub(4));
        let height = (self.entries.len() as u16 * 2 + 2).min(area.height.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let popup = Rect::new(x, y, width, height);

        frame.render_widget(Clear, popup);
        let block = Block::default()
            .title(" Settings ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue));

        let inner = block.inner(popup);
        frame.render_widget(block, popup);

        let lines: Vec<Line> = self
            .entries
            .iter()
            .enumerate()
            .flat_map(|(i, entry)| {
                let style = if i == self.selected {
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                vec![
                    Line::from(vec![
                        Span::styled(&entry.key, style),
                        Span::styled(" = ", Style::default().fg(Color::DarkGray)),
                        Span::styled(&entry.value, Style::default().fg(Color::Yellow)),
                    ]),
                    Line::from(Span::styled(format!("  {}", entry.description), Style::default().fg(Color::DarkGray))),
                ]
            })
            .collect();

        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
    }
}
