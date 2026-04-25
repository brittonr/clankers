//! Structured progress rendering for tool execution
//!
//! Renders progress bars, spinners, phase indicators, and ETA estimates
//! based on `ToolProgress` events emitted by tools.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::time::Duration;
use std::time::Instant;

use clanker_tui_types::ProgressKind;
use clanker_tui_types::ToolProgress;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::text::Span;

/// Sample for ETA calculation
#[derive(Debug)]
struct ProgressSample {
    timestamp: Instant,
    value: f32,
}

/// State for a single tool's progress
#[derive(Debug)]
struct ProgressState {
    /// Latest progress update
    progress: ToolProgress,
    /// History for ETA calculation (last N samples)
    history: VecDeque<ProgressSample>,
}

/// Renderer that tracks and formats progress for active tools
#[derive(Debug)]
pub struct ProgressRenderer {
    /// Current progress state per call_id
    states: HashMap<String, ProgressState>,
}

impl ProgressRenderer {
    pub fn new() -> Self {
        Self { states: HashMap::new() }
    }

    /// Update progress for a call_id
    pub fn update(&mut self, call_id: &str, progress: ToolProgress) {
        let state = self.states.entry(call_id.to_string()).or_insert_with(|| ProgressState {
            progress: progress.clone(),
            history: VecDeque::with_capacity(10),
        });

        state.progress = progress.clone();

        // Add sample for ETA calculation
        if let Some(percent) = progress.kind.as_percentage() {
            state.history.push_back(ProgressSample {
                timestamp: Instant::now(),
                value: percent,
            });
            if state.history.len() > 10 {
                state.history.pop_front();
            }
        }
    }

    /// Remove state for a completed tool
    pub fn remove(&mut self, call_id: &str) {
        self.states.remove(call_id);
    }

    /// Render progress info as styled Line spans (to embed inline in the chat).
    ///
    /// Returns `None` if no progress state exists for this call_id.
    pub fn render_inline<'a>(&self, call_id: &str, tick: u64) -> Option<Vec<Span<'a>>> {
        let state = self.states.get(call_id)?;
        Some(match &state.progress.kind {
            ProgressKind::Bytes { current, total } => self.render_countable(state, tick, *current, *total, "bytes"),
            ProgressKind::Lines { current, total } => self.render_countable(state, tick, *current, *total, "lines"),
            ProgressKind::Items { current, total } => self.render_countable(state, tick, *current, *total, "items"),
            ProgressKind::Percentage { percent } => self.render_percentage(state, *percent),
            ProgressKind::Phase {
                name,
                step,
                total_steps,
            } => self.render_phase(state, tick, name, *step, *total_steps),
        })
    }

    /// Render countable progress (bytes/lines/items)
    fn render_countable<'a>(
        &self,
        state: &ProgressState,
        tick: u64,
        current: u64,
        total: Option<u64>,
        unit: &str,
    ) -> Vec<Span<'a>> {
        let mut spans = Vec::new();

        if let Some(total) = total {
            // Known total: show bar + count
            let percent = if total > 0 {
                (current as f32 / total as f32) * 100.0
            } else {
                0.0
            };
            let bar = progress_bar(percent, 20);
            spans.push(Span::styled(bar, Style::default().fg(Color::Cyan)));
            spans.push(Span::styled(
                format!(" {}/{} {}", format_count(current), format_count(total), unit),
                Style::default().fg(Color::DarkGray),
            ));

            // ETA
            if let Some(eta) = calculate_eta(state, percent) {
                spans
                    .push(Span::styled(format!(" ETA {}", format_duration(eta)), Style::default().fg(Color::DarkGray)));
            }
        } else {
            // Unknown total: spinner + count
            let spinner = spinner_char(tick);
            spans.push(Span::styled(format!("{} ", spinner), Style::default().fg(Color::Yellow)));
            spans.push(Span::styled(
                format!("{} {}", format_count(current), unit),
                Style::default().fg(Color::DarkGray),
            ));
        }

        // Message
        if let Some(ref message) = state.progress.message {
            spans.push(Span::styled(format!(" · {}", message), Style::default().fg(Color::DarkGray)));
        }

        spans
    }

    /// Render percentage progress
    fn render_percentage<'a>(&self, state: &ProgressState, percent: f32) -> Vec<Span<'a>> {
        let mut spans = Vec::new();
        let bar = progress_bar(percent, 20);
        spans.push(Span::styled(bar, Style::default().fg(Color::Cyan)));
        spans.push(Span::styled(format!(" {:.1}%", percent), Style::default().fg(Color::DarkGray)));

        if let Some(eta) = calculate_eta(state, percent) {
            spans.push(Span::styled(format!(" ETA {}", format_duration(eta)), Style::default().fg(Color::DarkGray)));
        }

        if let Some(ref message) = state.progress.message {
            spans.push(Span::styled(format!(" · {}", message), Style::default().fg(Color::DarkGray)));
        }

        spans
    }

    /// Render phase progress
    fn render_phase<'a>(
        &self,
        state: &ProgressState,
        tick: u64,
        name: &str,
        step: u32,
        total_steps: Option<u32>,
    ) -> Vec<Span<'a>> {
        let mut spans = Vec::new();
        let spinner = spinner_char(tick);

        if let Some(total) = total_steps {
            spans.push(Span::styled(
                format!("{} [{}/{}] {}", spinner, step, total, name),
                Style::default().fg(Color::Magenta),
            ));
        } else {
            spans.push(Span::styled(format!("{} {}", spinner, name), Style::default().fg(Color::Magenta)));
        }

        if let Some(ref message) = state.progress.message {
            spans.push(Span::styled(format!(" · {}", message), Style::default().fg(Color::DarkGray)));
        }

        spans
    }
}

impl Default for ProgressRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate ETA based on progress history
fn calculate_eta(state: &ProgressState, current_percent: f32) -> Option<Duration> {
    if state.history.len() < 2 {
        return None;
    }

    let oldest = state.history.front()?;
    let newest = state.history.back()?;

    let time_delta = newest.timestamp.duration_since(oldest.timestamp).as_secs_f32();
    let percent_delta = newest.value - oldest.value;

    if percent_delta <= 0.0 || time_delta <= 0.0 {
        return None;
    }

    let percent_remaining = 100.0 - current_percent;
    let rate = percent_delta / time_delta;
    let eta_seconds = percent_remaining / rate;

    // Cap at 24 hours, skip if unreasonable
    if eta_seconds > 0.0 && eta_seconds < 86400.0 {
        Some(Duration::from_secs_f32(eta_seconds))
    } else {
        None
    }
}

/// Render a text-based progress bar: `[████████░░░░░░░░░░░░]`
fn progress_bar(percent: f32, width: usize) -> String {
    let clamped = percent.clamp(0.0, 100.0);
    let filled = ((clamped / 100.0) * width as f32).round() as usize;
    let empty = width.saturating_sub(filled);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

/// Spinner animation frames, indexed by tick
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        unchecked_division,
        reason = "divisor guarded by is_empty/non-zero check or TUI layout constraint"
    )
)]
fn spinner_char(tick: u64) -> char {
    const FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    FRAMES[(tick as usize / 3) % FRAMES.len()]
}

/// Format a count with K/M suffixes
fn format_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 10_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/// Format a duration as human-readable (e.g., "2m 30s")
fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_bar_rendering() {
        assert_eq!(progress_bar(0.0, 10), "[░░░░░░░░░░]");
        assert_eq!(progress_bar(50.0, 10), "[█████░░░░░]");
        assert_eq!(progress_bar(100.0, 10), "[██████████]");
    }

    #[test]
    fn progress_bar_edge_cases() {
        assert_eq!(progress_bar(0.0, 0), "[]");
        assert_eq!(progress_bar(150.0, 10), "[██████████]"); // clamps to 100%
        assert_eq!(progress_bar(-10.0, 10), "[░░░░░░░░░░]"); // clamps to 0%
    }

    #[test]
    fn format_count_small() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(999), "999");
        assert_eq!(format_count(9999), "9999");
    }

    #[test]
    fn format_count_thousands() {
        assert_eq!(format_count(10000), "10.0K");
        assert_eq!(format_count(50000), "50.0K");
    }

    #[test]
    fn format_count_millions() {
        assert_eq!(format_count(1_000_000), "1.0M");
        assert_eq!(format_count(2_500_000), "2.5M");
    }

    #[test]
    fn format_duration_seconds() {
        assert_eq!(format_duration(Duration::from_secs(0)), "0s");
        assert_eq!(format_duration(Duration::from_secs(45)), "45s");
    }

    #[test]
    fn format_duration_minutes() {
        assert_eq!(format_duration(Duration::from_secs(90)), "1m 30s");
        assert_eq!(format_duration(Duration::from_secs(300)), "5m 0s");
    }

    #[test]
    fn format_duration_hours() {
        assert_eq!(format_duration(Duration::from_secs(3661)), "1h 1m");
    }

    #[test]
    fn spinner_cycles_through_frames() {
        let a = spinner_char(0);
        let b = spinner_char(3);
        assert_ne!(a, b, "spinner should cycle");
    }

    #[test]
    fn renderer_update_and_render_lines() {
        let mut renderer = ProgressRenderer::new();
        renderer.update("call-1", ToolProgress::lines(42, None));

        let spans = renderer.render_inline("call-1", 0).expect("should have state");
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("42"), "should show count: {}", text);
        assert!(text.contains("lines"), "should show unit: {}", text);
    }

    #[test]
    fn renderer_update_and_render_bytes_with_total() {
        let mut renderer = ProgressRenderer::new();
        renderer.update("call-2", ToolProgress::bytes(50, Some(100)));

        let spans = renderer.render_inline("call-2", 0).expect("should have state");
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("50/100"), "should show count: {}", text);
        assert!(text.contains("█"), "should show progress bar: {}", text);
    }

    #[test]
    fn renderer_update_and_render_phase() {
        let mut renderer = ProgressRenderer::new();
        renderer.update("call-3", ToolProgress::phase("Compiling", 2, Some(3)));

        let spans = renderer.render_inline("call-3", 0).expect("should have state");
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("2/3"), "should show step: {}", text);
        assert!(text.contains("Compiling"), "should show phase: {}", text);
    }

    #[test]
    fn renderer_returns_none_for_unknown_call() {
        let renderer = ProgressRenderer::new();
        assert!(renderer.render_inline("unknown", 0).is_none());
    }

    #[test]
    fn renderer_remove_clears_state() {
        let mut renderer = ProgressRenderer::new();
        renderer.update("call-1", ToolProgress::lines(10, None));
        assert!(renderer.render_inline("call-1", 0).is_some());

        renderer.remove("call-1");
        assert!(renderer.render_inline("call-1", 0).is_none());
    }

    #[test]
    fn eta_calculation_needs_history() {
        let state = ProgressState {
            progress: ToolProgress::percentage(50.0),
            history: VecDeque::new(),
        };
        // Not enough samples
        assert!(calculate_eta(&state, 50.0).is_none());
    }

    #[test]
    fn eta_calculation_with_samples() {
        let now = Instant::now();
        let mut history = VecDeque::new();
        // Simulate: 0% at t=0, 50% at t=10s → ETA for remaining 50% ≈ 10s
        history.push_back(ProgressSample {
            timestamp: now - Duration::from_secs(10),
            value: 0.0,
        });
        history.push_back(ProgressSample {
            timestamp: now,
            value: 50.0,
        });

        let state = ProgressState {
            progress: ToolProgress::percentage(50.0),
            history,
        };

        let eta = calculate_eta(&state, 50.0).expect("should calculate ETA");
        // Should be approximately 10 seconds (50% remaining at 5%/sec)
        assert!(eta.as_secs() >= 8 && eta.as_secs() <= 12, "ETA should be ~10s, got {:?}", eta);
    }

    #[test]
    fn renderer_with_message() {
        let mut renderer = ProgressRenderer::new();
        renderer.update("call-1", ToolProgress::lines(100, None).with_message("Searching /usr/lib"));

        let spans = renderer.render_inline("call-1", 0).expect("should have state");
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("Searching /usr/lib"), "should show message: {}", text);
    }
}
