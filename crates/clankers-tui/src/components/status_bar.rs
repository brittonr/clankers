//! Footer status bar

use clanker_tui_types::BudgetStatus;
use clanker_tui_types::ConnectionMode;
use clanker_tui_types::InputMode;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;

use crate::app::AppState;
use crate::app::RouterStatus;
use crate::theme::Theme;

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
    pub thinking_level: clanker_tui_types::ThinkingLevel,
    /// Plugin-contributed status bar segments
    pub plugin_spans: Vec<Span<'a>>,
    /// Context window gauge span
    pub context_span: Span<'a>,
    /// Git status span (None if not in a repo)
    pub git_span: Option<Span<'a>>,
    /// Process stats span (None if no active processes)
    pub process_span: Option<Span<'a>>,
    /// Active account name (empty if none)
    pub active_account: &'a str,
    /// Router daemon connection status
    pub router_status: RouterStatus,
    /// Budget status for cost display coloring
    pub budget_status: BudgetStatus,
    /// Active tool activity summary (None if no tools running)
    pub tool_activity: Option<Span<'a>>,
    /// Loop mode status (None if not in a loop)
    pub loop_status: Option<Span<'a>>,
    /// Whether prompt improve is enabled
    pub prompt_improve: bool,
    /// Connection mode (embedded, attached, reconnecting)
    pub connection_mode: ConnectionMode,
}

/// Render status bar
pub fn render_status_bar(frame: &mut Frame, data: &StatusBarData, theme: &Theme, area: Rect) {
    let mut spans = Vec::new();

    // Left section: mode indicators
    render_mode_indicators(&mut spans, data);

    // Center section: status badges and info
    render_status_badges(&mut spans, data);

    // Right section: trailing info
    render_trailing_info(&mut spans, data, theme);

    let line = Line::from(spans);
    let status = Paragraph::new(line);
    frame.render_widget(status, area);
}

/// Render mode indicators: input mode badge, streaming indicator, thinking badge
fn render_mode_indicators<'a>(spans: &mut Vec<Span<'a>>, data: &StatusBarData<'a>) {
    // Mode badge — distinct colours so it's always obvious
    let (mode_text, mode_style) = match data.input_mode {
        InputMode::Normal => {
            (" NORMAL ", Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD))
        }
        InputMode::Insert => {
            (" INSERT ", Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD))
        }
    };
    spans.push(Span::styled(mode_text, mode_style));

    // Streaming indicator
    if matches!(data.state, AppState::Streaming) {
        spans.push(Span::styled(
            " ⏳ ",
            Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
    }

    // Thinking level indicator
    if data.thinking_enabled {
        let level_label = format!(" 💭 {} ", data.thinking_level.label());
        let level_color = match data.thinking_level {
            clanker_tui_types::ThinkingLevel::Off => Color::DarkGray,
            clanker_tui_types::ThinkingLevel::Low => Color::Blue,
            clanker_tui_types::ThinkingLevel::Medium => Color::Magenta,
            clanker_tui_types::ThinkingLevel::High => Color::Yellow,
            clanker_tui_types::ThinkingLevel::Max => Color::Red,
        };
        spans.push(Span::styled(
            level_label,
            Style::default().fg(Color::Black).bg(level_color).add_modifier(Modifier::BOLD),
        ));
    }

    // Connection mode badge
    match data.connection_mode {
        ConnectionMode::Embedded => {
            // No badge in embedded mode (normal, quiet)
        }
        ConnectionMode::Attached => {
            spans.push(Span::styled(
                " ATTACHED ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        ConnectionMode::Remote { ref node_id_short } => {
            spans.push(Span::styled(
                format!(" 🌐 {node_id_short} "),
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        ConnectionMode::Reconnecting => {
            spans.push(Span::styled(
                " RECONNECTING ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            ));
        }
    }

    // Prompt improve indicator
    if data.prompt_improve {
        spans.push(Span::styled(
            " ✨ improve ",
            Style::default().fg(Color::Black).bg(Color::Magenta).add_modifier(Modifier::BOLD),
        ));
    }
}

/// Render status badges: account, router, context, git, process, tool activity, cost/budget,
/// plugins
fn render_status_badges<'a>(spans: &mut Vec<Span<'a>>, data: &StatusBarData<'a>) {
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

    // Process stats
    if let Some(ref proc) = data.process_span {
        spans.push(proc.clone());
    }

    // Loop mode
    if let Some(ref loop_span) = data.loop_status {
        spans.push(loop_span.clone());
    }

    // Active tool progress
    if let Some(ref tool) = data.tool_activity {
        spans.push(tool.clone());
    }

    // Cost / budget badge (color-coded)
    if data.total_tokens > 0 {
        let (cost_text, cost_color) = match &data.budget_status {
            BudgetStatus::NoBudget => (format!(" ${:.4} ", data.total_cost), Color::DarkGray),
            BudgetStatus::Ok { remaining } => {
                (format!(" ${:.2} (${:.2} left) ", data.total_cost, remaining), Color::Green)
            }
            BudgetStatus::Warning {
                over_soft_by: _,
                hard_limit_remaining,
            } => {
                if hard_limit_remaining.is_finite() {
                    (format!(" ${:.2} ⚠ (${:.2} to hard) ", data.total_cost, hard_limit_remaining), Color::Yellow)
                } else {
                    (format!(" ${:.2} ⚠ over budget ", data.total_cost), Color::Yellow)
                }
            }
            BudgetStatus::Exceeded { .. } => (format!(" ${:.2} ✖ exceeded ", data.total_cost), Color::Red),
        };
        spans.push(Span::styled(
            cost_text,
            Style::default().fg(Color::Black).bg(cost_color).add_modifier(Modifier::BOLD),
        ));
    }

    // Plugin status segments
    for span in &data.plugin_spans {
        spans.push(span.clone());
    }
}

/// Render trailing info section: state, model, tokens, cwd
fn render_trailing_info<'a>(spans: &mut Vec<Span<'a>>, data: &StatusBarData<'a>, theme: &Theme) {
    let state_str = match data.state {
        AppState::Idle => "idle",
        AppState::Streaming => "streaming",
        AppState::Command => "command",
        AppState::Dialog => "dialog",
    };

    let info = if data.total_tokens > 0 {
        format!(" {} | {} tok | {} | {}", state_str, data.total_tokens, data.model, data.cwd)
    } else {
        format!(" {} | {} | {}", state_str, data.model, data.cwd)
    };
    spans.push(Span::styled(info, Style::default().fg(theme.fg).bg(theme.bg)));
}
