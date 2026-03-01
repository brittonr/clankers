//! Footer status bar

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;

use crate::config::keybindings::InputMode;
use crate::tui::app::AppState;
use crate::tui::app::RouterStatus;
use crate::tui::theme::Theme;

/// Data for status bar rendering
pub struct StatusBarData<'a> {
    pub cwd: &'a str,
    pub model: &'a str,
    pub total_tokens: usize,
    pub total_cost: f64,
    pub state: &'a AppState,
    pub session_id: &'a str,
    pub input_mode: InputMode,
    pub thinking_enabled: bool,
    pub thinking_level: crate::provider::ThinkingLevel,
    /// Plugin-contributed status bar segments
    pub plugin_spans: Vec<Span<'a>>,
    /// Context window gauge span
    pub context_span: Span<'a>,
    /// Git status span (None if not in a repo)
    pub git_span: Option<Span<'a>>,
    /// Active account name (empty if none)
    pub active_account: &'a str,
    /// Router daemon connection status
    pub router_status: RouterStatus,
}

/// Render status bar
pub fn render_status_bar(frame: &mut Frame, data: &StatusBarData, theme: &Theme, area: Rect) {
    let state_str = match data.state {
        AppState::Idle => "idle",
        AppState::Streaming => "streaming",
        AppState::Command => "command",
        AppState::Dialog => "dialog",
    };

    // Mode badge — distinct colours so it's always obvious
    let (mode_text, mode_style) = match data.input_mode {
        InputMode::Normal => {
            (" NORMAL ", Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD))
        }
        InputMode::Insert => {
            (" INSERT ", Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD))
        }
    };

    // Put state + model first so they're always visible; long cwd at the end
    // gets truncated gracefully when the center column is narrow.
    let info = if data.total_tokens > 0 {
        format!(
            " {} | {} tok | ${:.4} | {} | {}",
            state_str, data.total_tokens, data.total_cost, data.model, data.cwd
        )
    } else {
        format!(" {} | {} | {}", state_str, data.model, data.cwd)
    };

    let mut spans = vec![Span::styled(mode_text, mode_style)];

    if matches!(data.state, AppState::Streaming) {
        spans.push(Span::styled(
            " ⏳ ",
            Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
    }

    if data.thinking_enabled {
        let level_label = format!(" 💭 {} ", data.thinking_level.label());
        let level_color = match data.thinking_level {
            crate::provider::ThinkingLevel::Off => Color::DarkGray,
            crate::provider::ThinkingLevel::Low => Color::Blue,
            crate::provider::ThinkingLevel::Medium => Color::Magenta,
            crate::provider::ThinkingLevel::High => Color::Yellow,
            crate::provider::ThinkingLevel::Max => Color::Red,
        };
        spans.push(Span::styled(
            level_label,
            Style::default().fg(Color::Black).bg(level_color).add_modifier(Modifier::BOLD),
        ));
    }

    // Account badge
    if !data.active_account.is_empty() {
        spans.push(Span::styled(
            format!(" 👤 {} ", data.active_account),
            Style::default().fg(Color::Black).bg(Color::Blue).add_modifier(Modifier::BOLD),
        ));
    }

    // Router connection status badge
    match data.router_status {
        RouterStatus::Connected => {
            spans.push(Span::styled(
                " ⚡rtr ",
                Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD),
            ));
        }
        RouterStatus::Local => {
            // Local/in-process — no badge (quiet when everything is normal)
        }
        RouterStatus::Disconnected => {
            spans.push(Span::styled(
                " ✖rtr ",
                Style::default().fg(Color::Black).bg(Color::Red).add_modifier(Modifier::BOLD),
            ));
        }
    }

    // Context window gauge
    spans.push(data.context_span.clone());

    // Git status
    if let Some(ref git) = data.git_span {
        spans.push(git.clone());
    }

    // Plugin status segments
    for span in &data.plugin_spans {
        spans.push(span.clone());
    }

    spans.push(Span::styled(info, Style::default().fg(theme.fg).bg(theme.bg)));

    let line = Line::from(spans);

    let status = Paragraph::new(line);
    frame.render_widget(status, area);
}
