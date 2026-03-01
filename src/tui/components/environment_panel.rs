//! Environment panel — shows current configuration at a glance
//!
//! Displays model, thinking mode, session, cwd, and other settings
//! without needing `/status`. Can be rendered as a compact panel.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Block;
use ratatui::widgets::Borders;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;

use crate::tui::app::App;
use crate::tui::app::AppState;
use crate::tui::components::context_gauge::ContextGauge;
use crate::tui::components::git_status::GitStatus;
use crate::tui::theme::Theme;

// ── Rendering ───────────────────────────────────────────────────────────────

/// Render the environment panel
pub fn render_environment_panel(
    frame: &mut Frame,
    app: &App,
    context: &ContextGauge,
    git: &GitStatus,
    theme: &Theme,
    area: Rect,
    _focused: bool,
) {
    let border_color = theme.border;
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(" Environment ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();

    // Model
    lines.push(Line::from(vec![
        Span::styled("  Model  ", Style::default().fg(Color::DarkGray)),
        Span::styled(&app.model, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
    ]));

    // State
    let (state_str, state_color) = match app.state {
        AppState::Idle => ("idle", Color::Green),
        AppState::Streaming => ("streaming", Color::Yellow),
        AppState::Command => ("command", Color::Cyan),
        AppState::Dialog => ("dialog", Color::Magenta),
    };
    lines.push(Line::from(vec![
        Span::styled("  State  ", Style::default().fg(Color::DarkGray)),
        Span::styled(state_str, Style::default().fg(state_color)),
    ]));

    // Thinking
    let think_str = if app.thinking_enabled { "on" } else { "off" };
    let think_color = if app.thinking_enabled {
        Color::Magenta
    } else {
        Color::DarkGray
    };
    lines.push(Line::from(vec![
        Span::styled("  Think  ", Style::default().fg(Color::DarkGray)),
        Span::styled(think_str, Style::default().fg(think_color)),
    ]));

    // Context gauge
    let frac = context.usage_fraction();
    let used = ContextGauge::format_tokens(context.total_used());
    let total = ContextGauge::format_tokens(context.context_window);
    let ctx_color = context.usage_color();
    // Mini bar
    let bar_width = 10;
    let filled = (frac * bar_width as f64).round() as usize;
    let empty = bar_width - filled;
    let bar: String = format!("{}{}", "█".repeat(filled), "░".repeat(empty));
    lines.push(Line::from(vec![
        Span::styled("  Ctx    ", Style::default().fg(Color::DarkGray)),
        Span::styled(bar, Style::default().fg(ctx_color)),
        Span::styled(format!(" {}/{}", used, total), Style::default().fg(ctx_color)),
    ]));

    // Tokens / Cost
    if app.total_tokens > 0 {
        lines.push(Line::from(vec![
            Span::styled("  Tokens ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} (${:.4})", ContextGauge::format_tokens(app.total_tokens), app.total_cost),
                Style::default().fg(Color::White),
            ),
        ]));
    }

    // Git
    if git.is_repo {
        let branch = git.branch.as_deref().unwrap_or("???");
        let git_color = if git.is_dirty() { Color::Yellow } else { Color::Magenta };
        let mut git_spans = vec![
            Span::styled("  Git    ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!(" {}", branch), Style::default().fg(git_color)),
        ];
        if git.is_dirty() {
            let mut changes = Vec::new();
            if git.staged_count > 0 {
                changes.push(format!("+{}", git.staged_count));
            }
            if git.dirty_count > 0 {
                changes.push(format!("~{}", git.dirty_count));
            }
            if git.untracked_count > 0 {
                changes.push(format!("?{}", git.untracked_count));
            }
            git_spans.push(Span::styled(format!(" {}", changes.join("")), Style::default().fg(Color::Yellow)));
        }
        lines.push(Line::from(git_spans));
    }

    // Session ID (shortened)
    if !app.session_id.is_empty() {
        let short_id: String = app.session_id.chars().take(8).collect();
        lines.push(Line::from(vec![
            Span::styled("  Session ", Style::default().fg(Color::DarkGray)),
            Span::styled(short_id, Style::default().fg(Color::DarkGray)),
        ]));
    }

    // CWD (just the last component)
    let cwd_short = std::path::Path::new(&app.cwd)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| app.cwd.clone());
    lines.push(Line::from(vec![
        Span::styled("  Dir    ", Style::default().fg(Color::DarkGray)),
        Span::styled(cwd_short, Style::default().fg(Color::White)),
    ]));

    let para = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(para, inner);
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_panel_no_panic() {
        // Just ensure it doesn't panic with default state
        let _app = App::new("claude-sonnet-4-5".into(), "/tmp".into(), Theme::dark());
        let context = ContextGauge::new("claude-sonnet-4-5");
        let mut git = GitStatus::new("/tmp");
        git.is_repo = false;
        // We can't easily test rendering without a terminal, but
        // we can at least verify the data structures are sound
        let _ = context.summary();
        let _ = git.summary();
    }
}
