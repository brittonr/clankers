//! Rendering logic for the leader menu overlay.

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

use super::LeaderMenu;
use super::types::LeaderAction;

impl LeaderMenu {
    /// Render the leader menu overlay.
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if !self.visible {
            return;
        }

        let current = match self.current() {
            Some(m) => m,
            None => return,
        };

        // Calculate popup dimensions
        let item_count = current.items.len() as u16;
        // width: widest "  k  label…" + padding + borders
        let max_label_w = current
            .items
            .iter()
            .map(|i| {
                let suffix = u16::from(matches!(i.action, LeaderAction::Submenu(_))); // "…"
                // "  k  label" = 5 + label.len + suffix
                5 + i.label.len() as u16 + suffix
            })
            .max()
            .unwrap_or(10);
        let content_width = max_label_w + 2; // padding
        let width = (content_width + 2).min(area.width.saturating_sub(4)); // + borders
        let height = (item_count + 4).min(area.height.saturating_sub(4)); // items + title + spacer + hint + borders

        // Center the popup
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let popup_area = Rect::new(x, y, width, height);

        // Clear background
        frame.render_widget(Clear, popup_area);

        // Build title with breadcrumb
        let title = if self.breadcrumb.is_empty() {
            " Space ".to_string()
        } else {
            format!(" Space › {} ", self.breadcrumb.join(" › "))
        };

        let block = Block::default()
            .title(Span::styled(title, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));

        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        // Render menu items
        let mut lines: Vec<Line> = Vec::new();

        for item in &current.items {
            let is_submenu = matches!(item.action, LeaderAction::Submenu(_));
            let suffix = if is_submenu { "…" } else { "" };

            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(item.key.to_string(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::raw("  "),
                Span::styled(format!("{}{}", item.label, suffix), Style::default().fg(Color::White)),
            ]));
        }

        // Spacer
        lines.push(Line::from(""));

        // Hint
        let hint_text = if self.stack.len() > 1 { "esc back" } else { "esc close" };
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(hint_text, Style::default().fg(Color::DarkGray)),
        ]));

        let paragraph = Paragraph::new(lines);
        frame.render_widget(paragraph, inner);
    }
}
