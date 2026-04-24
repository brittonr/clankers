//! Cost detail overlay — per-model breakdown with budget progress bar
//!
//! Toggled via `C` in normal mode. Shows:
//! - Per-model table: model name, input/output tokens, cost, percentage
//! - Total cost
//! - Budget bar (if budget configured): visual progress toward limit
//! - Budget status badge

use std::sync::Arc;

use clanker_tui_types::BudgetStatus;
use clanker_tui_types::CostProvider;
use clanker_tui_types::CostSummary;

use super::prelude::*;

/// Render the cost detail overlay if visible.
pub fn render_cost_overlay(
    frame: &mut Frame,
    cost_tracker: Option<&Arc<dyn CostProvider>>,
    visible: bool,
    theme: &Theme,
) {
    if !visible {
        return;
    }

    let ct = match cost_tracker {
        Some(ct) => ct,
        None => {
            render_no_data(frame, theme);
            return;
        }
    };

    let summary = ct.summary();
    render_summary(frame, &summary, theme);
}

fn render_no_data(frame: &mut Frame, _theme: &Theme) {
    let screen = frame.area();
    let width = 40u16.min(screen.width.saturating_sub(4));
    let height = 5u16.min(screen.height.saturating_sub(4));
    let x = (screen.width.saturating_sub(width)) / 2;
    let y = (screen.height.saturating_sub(height)) / 2;
    let area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(" Cost Details ", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)));
    let msg =
        Paragraph::new(Line::from(Span::styled("No cost tracking active.", Style::default().fg(Color::DarkGray))))
            .block(block)
            .wrap(Wrap { trim: false });
    frame.render_widget(msg, area);
}

fn render_summary(frame: &mut Frame, summary: &CostSummary, theme: &Theme) {
    let screen = frame.area();
    let area = calculate_overlay_area(screen, summary);

    frame.render_widget(Clear, area);

    let (status_icon, status_color) = match &summary.budget_status {
        BudgetStatus::NoBudget => ("", Color::DarkGray),
        BudgetStatus::Ok { .. } => (" ✓", Color::Green),
        BudgetStatus::Warning { .. } => (" ⚠", Color::Yellow),
        BudgetStatus::Exceeded { .. } => (" ✖", Color::Red),
    };

    let title = format!(" Cost Details{status_icon} — C:close ");
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(status_color))
        .title(Span::styled(title, Style::default().fg(status_color).add_modifier(Modifier::BOLD)));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();

    // Column header
    let header_style = Style::default().fg(theme.fg).add_modifier(Modifier::BOLD);
    lines.push(Line::from(vec![
        Span::styled(format!("{:<24}", "Model"), header_style),
        Span::styled(format!("{:>10}", "Input"), header_style),
        Span::styled(format!("{:>10}", "Output"), header_style),
        Span::styled(format!("{:>10}", "Cost"), header_style),
        Span::styled(format!("{:>6}", "%"), header_style),
    ]));

    // Separator
    let sep: String = "─".repeat(inner.width as usize);
    lines.push(Line::from(Span::styled(sep.clone(), Style::default().fg(Color::DarkGray))));

    // Per-model rows
    render_model_rows(&mut lines, summary, theme);

    // Separator
    lines.push(Line::from(Span::styled(sep, Style::default().fg(Color::DarkGray))));

    // Total row
    render_total_row(&mut lines, summary, theme);

    // Budget bar (if budget configured)
    match &summary.budget_status {
        BudgetStatus::NoBudget => {}
        BudgetStatus::Ok { remaining } => {
            lines.push(Line::from(""));
            let bar = render_budget_bar(
                summary.total_cost,
                summary.total_cost + remaining,
                inner.width as usize,
                Color::Green,
            );
            lines.push(bar);
        }
        BudgetStatus::Warning {
            hard_limit_remaining, ..
        } => {
            lines.push(Line::from(""));
            let limit = summary.total_cost + hard_limit_remaining;
            let bar = render_budget_bar(summary.total_cost, limit, inner.width as usize, Color::Yellow);
            lines.push(bar);
        }
        BudgetStatus::Exceeded { over_hard_by } => {
            lines.push(Line::from(""));
            let limit = summary.total_cost - over_hard_by;
            let bar = render_budget_bar(summary.total_cost, limit, inner.width as usize, Color::Red);
            lines.push(bar);
        }
    }

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner);
}

/// Calculate the centered overlay area based on screen size and content
fn calculate_overlay_area(screen: Rect, summary: &CostSummary) -> Rect {
    // Size: 50% width (min 50, max screen-4), height based on content
    let popup_width = (screen.width * 50 / 100).max(50).min(screen.width.saturating_sub(4));
    // Header(1) + separator(1) + models + separator(1) + total(1) + budget(2) + padding
    let model_count = summary.by_model.len().max(1);
    let popup_height = (6 + model_count as u16 + 2).min(screen.height.saturating_sub(4));
    let x = (screen.width.saturating_sub(popup_width)) / 2;
    let y = (screen.height.saturating_sub(popup_height)) / 2;
    Rect::new(x, y, popup_width, popup_height)
}

/// Render per-model statistics rows, sorted by cost descending
fn render_model_rows(lines: &mut Vec<Line>, summary: &CostSummary, theme: &Theme) {
    let mut models = summary.by_model.clone();
    models.sort_by(|a, b| b.cost_usd.partial_cmp(&a.cost_usd).unwrap_or(std::cmp::Ordering::Equal));

    if models.is_empty() {
        lines.push(Line::from(Span::styled("  No usage recorded yet.", Style::default().fg(Color::DarkGray))));
        return;
    }

    for m in &models {
        let row_style = Style::default().fg(theme.fg);
        let name = if m.display_name.len() > 23 {
            format!("{}…", &m.display_name[..22])
        } else {
            m.display_name.clone()
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{:<24}", name), row_style),
            Span::styled(format!("{:>10}", format_tokens(m.input_tokens)), row_style),
            Span::styled(format!("{:>10}", format_tokens(m.output_tokens)), row_style),
            Span::styled(
                format!("{:>10}", format!("${:.4}", m.cost_usd)),
                Style::default().fg(if m.cost_usd > 0.0 {
                    Color::Yellow
                } else {
                    Color::DarkGray
                }),
            ),
            Span::styled(format!("{:>5.1}%", m.percentage), Style::default().fg(Color::DarkGray)),
        ]));
    }
}

/// Render the totals row
fn render_total_row(lines: &mut Vec<Line>, summary: &CostSummary, theme: &Theme) {
    lines.push(Line::from(vec![
        Span::styled(format!("{:<24}", "Total"), Style::default().fg(theme.fg).add_modifier(Modifier::BOLD)),
        Span::raw(format!("{:>10}", "")),
        Span::raw(format!("{:>10}", "")),
        Span::styled(
            format!("{:>10}", format!("${:.4}", summary.total_cost)),
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!("{:>6}", "")),
    ]));
}

/// Render a budget progress bar: `████████░░░░ $1.23 / $5.00`
fn render_budget_bar(current: f64, limit: f64, width: usize, color: Color) -> Line<'static> {
    let label = format!("${:.2} / ${:.2}", current, limit);
    // 3 for spaces around bar
    let bar_width = width.saturating_sub(label.len() + 3);
    let ratio = if limit > 0.0 { (current / limit).min(1.5) } else { 0.0 };
    let filled = ((ratio * bar_width as f64) as usize).min(bar_width);
    let empty = bar_width.saturating_sub(filled);

    let bar_filled: String = "█".repeat(filled);
    let bar_empty: String = "░".repeat(empty);

    Line::from(vec![
        Span::raw(" "),
        Span::styled(bar_filled, Style::default().fg(color)),
        Span::styled(bar_empty, Style::default().fg(Color::DarkGray)),
        Span::raw(" "),
        Span::styled(label, Style::default().fg(color).add_modifier(Modifier::BOLD)),
    ])
}

/// Format token counts: 1234 → "1.2k", 1234567 → "1.2M"
fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
