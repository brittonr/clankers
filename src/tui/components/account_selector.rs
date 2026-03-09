//! Account selector (fuzzy search picker)

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

/// Display info for an account entry
#[derive(Debug, Clone)]
pub struct AccountItem {
    pub name: String,
    pub label: Option<String>,
    pub is_active: bool,
    pub is_expired: bool,
}

impl AccountItem {}

#[derive(Default)]
pub struct AccountSelector {
    pub accounts: Vec<AccountItem>,
    pub filter: String,
    pub selected: usize,
    pub visible: bool,
}

impl AccountSelector {
    pub fn new() -> Self {
        Self {
            accounts: Vec::new(),
            filter: String::new(),
            selected: 0,
            visible: false,
        }
    }

    pub fn open(&mut self, accounts: Vec<AccountItem>) {
        self.accounts = accounts;
        self.visible = true;
        self.filter.clear();
        // Pre-select the active account
        self.selected = self.filtered_accounts().iter().position(|a| a.is_active).unwrap_or(0);
    }

    pub fn close(&mut self) {
        self.visible = false;
    }

    pub fn filtered_accounts(&self) -> Vec<&AccountItem> {
        let filter_lower = self.filter.to_lowercase();
        self.accounts
            .iter()
            .filter(|a| {
                filter_lower.is_empty()
                    || a.name.to_lowercase().contains(&filter_lower)
                    || a.label.as_ref().is_some_and(|l| l.to_lowercase().contains(&filter_lower))
            })
            .collect()
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        let max = self.filtered_accounts().len().saturating_sub(1);
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

    /// Returns the selected account name, if any
    pub fn select(&self) -> Option<String> {
        let filtered = self.filtered_accounts();
        filtered.get(self.selected).map(|a| a.name.clone())
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if !self.visible {
            return;
        }

        let width = 50.min(area.width.saturating_sub(4));
        let height = 15.min(area.height.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let popup_area = Rect::new(x, y, width, height);

        frame.render_widget(Clear, popup_area);

        let block = Block::default()
            .title(" Switch Account ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Magenta));

        let filter_line = Line::from(vec![
            Span::styled("Filter: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&self.filter, Style::default().fg(Color::White)),
            Span::styled("_", Style::default().fg(Color::White).add_modifier(Modifier::SLOW_BLINK)),
        ]);

        let filtered = self.filtered_accounts();
        let items: Vec<ListItem> = filtered
            .iter()
            .enumerate()
            .map(|(i, account)| {
                let marker = if account.is_active { "▸ " } else { "  " };
                let expired = if account.is_expired { " (expired)" } else { "" };
                let label = account.label.as_ref().map(|l| format!(" — {}", l)).unwrap_or_default();
                let text = format!("{}{}{}{}", marker, account.name, label, expired);

                let style = if i == self.selected {
                    Style::default().bg(Color::DarkGray).fg(Color::White)
                } else if account.is_expired {
                    Style::default().fg(Color::Red)
                } else if account.is_active {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(Span::styled(text, style))
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
