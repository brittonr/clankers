//! Model selector (fuzzy search picker)

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
use ratatui::widgets::List;
use ratatui::widgets::ListItem;
use ratatui::widgets::Paragraph;

pub struct ModelSelector {
    pub models: Vec<String>,
    pub filter: String,
    pub selected: usize,
    pub visible: bool,
}

impl ModelSelector {
    pub fn new(models: Vec<String>) -> Self {
        Self {
            models,
            filter: String::new(),
            selected: 0,
            visible: false,
        }
    }

    pub fn open(&mut self) {
        self.visible = true;
        self.filter.clear();
        self.selected = 0;
    }

    pub fn close(&mut self) {
        self.visible = false;
    }

    pub fn filtered_models(&self) -> Vec<&str> {
        let filter_lower = self.filter.to_lowercase();
        self.models
            .iter()
            .filter(|m| filter_lower.is_empty() || m.to_lowercase().contains(&filter_lower))
            .map(|m| m.as_str())
            .collect()
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        let max = self.filtered_models().len().saturating_sub(1);
        self.selected = (self.selected + 1).min(max);
    }

    pub fn type_char(&mut self, c: char) {
        self.filter.push(c);
        self.selected = 0;
    }

    pub fn backspace(&mut self) {
        self.filter.pop();
        self.selected = 0;
    }

    pub fn select(&mut self) -> Option<String> {
        let filtered = self.filtered_models();
        filtered.get(self.selected).map(|s| s.to_string())
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if !self.visible {
            return;
        }

        // Center a floating box
        let width = 50.min(area.width.saturating_sub(4));
        let height = 15.min(area.height.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let popup_area = Rect::new(x, y, width, height);

        frame.render_widget(Clear, popup_area);

        let block = Block::default()
            .title(" Select Model ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue));

        // Filter input
        let filter_line = Line::from(vec![
            Span::styled("Filter: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&self.filter, Style::default().fg(Color::White)),
            Span::styled("_", Style::default().fg(Color::White).add_modifier(Modifier::SLOW_BLINK)),
        ]);

        let filtered = self.filtered_models();
        let items: Vec<ListItem> = filtered
            .iter()
            .enumerate()
            .map(|(i, model)| {
                let style = if i == self.selected {
                    Style::default().bg(Color::DarkGray).fg(Color::White)
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(Span::styled(model.to_string(), style))
            })
            .collect();

        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        if inner.height > 1 {
            let filter_area = Rect::new(inner.x, inner.y, inner.width, 1);
            frame.render_widget(Paragraph::new(filter_line), filter_area);

            let list_area = Rect::new(inner.x, inner.y + 1, inner.width, inner.height - 1);
            frame.render_widget(List::new(items), list_area);
        }
    }
}
