//! Render plugin Widget trees via ratatui

use clanker_tui_types::PluginNotification;
use clanker_tui_types::PluginUiState;
use clanker_tui_types::Widget;
use ratatui::Frame;
use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Clear;
use ratatui::widgets::Gauge;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Row;
use ratatui::widgets::Table;
use ratatui::widgets::Wrap;

use crate::theme::Theme;

/// Render a plugin widget tree into the given area
/// Render a plugin widget tree. Recursion follows the widget tree structure
/// (bounded by plugin API constraints — typically ≤5 levels deep).
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(no_recursion, reason = "widget tree depth bounded by plugin API")
)]
pub fn render_widget(frame: &mut Frame, widget: &Widget, area: Rect) {
    match widget {
        Widget::Text { content, bold, color } => {
            let mut style = Style::default();
            if *bold {
                style = style.add_modifier(Modifier::BOLD);
            }
            if let Some(c) = color {
                style = style.fg(parse_color(c));
            }
            let paragraph = Paragraph::new(Span::styled(content.clone(), style));
            frame.render_widget(paragraph, area);
        }
        Widget::Box { children, direction } => {
            if children.is_empty() {
                return;
            }
            let is_vertical = matches!(direction, clanker_tui_types::Direction::Vertical);
            let constraints: Vec<Constraint> =
                children.iter().map(|_| Constraint::Ratio(1, children.len() as u32)).collect();
            let layout = Layout::default()
                .direction(if is_vertical {
                    Direction::Vertical
                } else {
                    Direction::Horizontal
                })
                .constraints(constraints)
                .split(area);
            for (i, child) in children.iter().enumerate() {
                if i < layout.len() {
                    render_widget(frame, child, layout[i]);
                }
            }
        }
        Widget::List { items, selected } => {
            let lines: Vec<Line> = items
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    let style = if i == *selected {
                        Style::default().bg(Color::DarkGray)
                    } else {
                        Style::default()
                    };
                    Line::from(Span::styled(item.clone(), style))
                })
                .collect();
            let paragraph = Paragraph::new(lines);
            frame.render_widget(paragraph, area);
        }
        Widget::Input { value, placeholder } => {
            let display = if value.is_empty() { placeholder } else { value };
            let style = if value.is_empty() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
            };
            let paragraph = Paragraph::new(Span::styled(display.clone(), style));
            frame.render_widget(paragraph, area);
        }
        Widget::Spacer { .. } => {}
        Widget::Progress { label, value, color } => {
            let ratio = value.clamp(0.0, 1.0);
            let gauge_color = color.as_deref().map(parse_color).unwrap_or(Color::Cyan);
            let label_text = if label.is_empty() {
                format!("{}%", (ratio * 100.0) as u32)
            } else {
                format!("{} {}%", label, (ratio * 100.0) as u32)
            };
            let gauge = Gauge::default()
                .label(label_text)
                .ratio(ratio)
                .gauge_style(Style::default().fg(gauge_color).bg(Color::DarkGray));
            frame.render_widget(gauge, area);
        }
        Widget::Table { rows, headers } => {
            let header_row = if !headers.is_empty() {
                Some(Row::new(
                    headers
                        .iter()
                        .map(|h| Span::styled(h.clone(), Style::default().add_modifier(Modifier::BOLD)))
                        .collect::<Vec<_>>(),
                ))
            } else {
                None
            };
            let data_rows: Vec<Row> = rows
                .iter()
                .map(|row| Row::new(row.iter().map(|c| Span::raw(c.clone())).collect::<Vec<_>>()))
                .collect();
            let col_count = rows.iter().map(|r| r.len()).max().unwrap_or(1).max(headers.len());
            let widths: Vec<Constraint> = (0..col_count).map(|_| Constraint::Ratio(1, col_count as u32)).collect();
            let mut table = Table::new(data_rows, &widths);
            if let Some(h) = header_row {
                table = table.header(h);
            }
            frame.render_widget(table, area);
        }
    }
}

/// Render all plugin widget panels stacked vertically in the given area.
/// Returns the number of rows consumed.
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        unchecked_division,
        reason = "divisor guarded by is_empty/non-zero check or TUI layout constraint"
    )
)]
pub fn render_plugin_panels(frame: &mut Frame, plugin_ui: &PluginUiState, theme: &Theme, area: Rect) -> u16 {
    if plugin_ui.widgets.is_empty() {
        return 0;
    }

    let mut sorted_plugins: Vec<&String> = plugin_ui.widgets.keys().collect();
    sorted_plugins.sort();

    let panel_count = sorted_plugins.len();
    if panel_count == 0 {
        return 0;
    }
    let per_panel = area.height / panel_count as u16;
    if per_panel < 3 {
        return 0; // Not enough room
    }

    let mut y = area.y;
    for plugin_name in sorted_plugins {
        let widget = &plugin_ui.widgets[plugin_name];
        let panel_height = per_panel.min(area.y + area.height - y);
        if panel_height < 3 {
            break;
        }
        let panel_area = Rect::new(area.x, y, area.width, panel_height);

        let block = Block::default()
            .title(format!(" 🔌 {} ", plugin_name))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border));
        let inner = block.inner(panel_area);
        frame.render_widget(block, panel_area);
        render_widget(frame, widget, inner);

        y += panel_height;
    }

    y - area.y
}

/// Render plugin notifications as toast overlays
pub fn render_plugin_notifications(frame: &mut Frame, notifications: &[PluginNotification], area: Rect) {
    let max_visible = 3;
    let visible: Vec<&PluginNotification> =
        notifications.iter().rev().take(max_visible).collect::<Vec<_>>().into_iter().rev().collect();

    let width = 50.min(area.width.saturating_sub(4));
    let toast_height: u16 = 3;

    for (i, notif) in visible.iter().enumerate() {
        let (border_color, title) = match notif.level.as_str() {
            "warning" | "warn" => (Color::Yellow, " ⚠ Warning "),
            "error" | "err" => (Color::Red, " ✗ Error "),
            _ => (Color::Blue, " ℹ Info "),
        };

        let x = area.width.saturating_sub(width).saturating_sub(2);
        let y = 1 + (i as u16 * (toast_height + 1));
        if y + toast_height > area.height {
            break;
        }
        let popup = Rect::new(x, y, width, toast_height);

        frame.render_widget(Clear, popup);
        let block = Block::default()
            .title(format!("{} [{}]", title, notif.plugin))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));
        let paragraph = Paragraph::new(Line::from(notif.message.clone())).block(block).wrap(Wrap { trim: true });
        frame.render_widget(paragraph, popup);
    }
}

/// Render plugin status segments into a line of spans
pub fn plugin_status_spans(plugin_ui: &PluginUiState) -> Vec<Span<'static>> {
    if plugin_ui.status_segments.is_empty() {
        return Vec::new();
    }

    let mut sorted: Vec<(&String, &clanker_tui_types::StatusSegment)> = plugin_ui.status_segments.iter().collect();
    sorted.sort_by_key(|(name, _)| *name);

    let mut spans = Vec::new();
    for (name, segment) in sorted {
        let color = segment.color.as_deref().map(parse_color).unwrap_or(Color::Cyan);
        spans.push(Span::styled(
            format!(" 🔌{}: {} ", name, segment.text),
            Style::default().fg(Color::Black).bg(color).add_modifier(Modifier::BOLD),
        ));
    }
    spans
}

fn parse_color(s: &str) -> Color {
    match s.to_lowercase().as_str() {
        "red" => Color::Red,
        "green" => Color::Green,
        "blue" => Color::Blue,
        "yellow" => Color::Yellow,
        "cyan" => Color::Cyan,
        "magenta" => Color::Magenta,
        "white" => Color::White,
        "gray" | "grey" => Color::Gray,
        _ => Color::White,
    }
}
