//! Context window gauge — shows token usage vs model context limit
//!
//! Rendered as a compact progress bar in the status bar area and
//! optionally as a small panel with detailed breakdown.

use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Span;

// ── Known model context windows ─────────────────────────────────────────────

/// Look up the context window size for a model name.
/// Returns (context_window_tokens, max_output_tokens).
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        nested_conditionals,
        reason = "complex control flow — extracting helpers would obscure logic"
    )
)]
pub fn model_context_window(model: &str) -> (usize, usize) {
    // Normalize: lowercase, strip leading provider prefixes
    let m = model.to_lowercase();
    let m = m.strip_prefix("anthropic/").unwrap_or(&m);

    if m.contains("opus") {
        (200_000, 32_000)
    } else if m.contains("sonnet") && (m.contains("3.5") || m.contains("3-5")) {
        (200_000, 8_192)
    } else if m.contains("sonnet") {
        (200_000, 16_384)
    } else if m.contains("haiku") && (m.contains("3.5") || m.contains("3-5")) {
        (200_000, 8_192)
    } else if m.contains("haiku") {
        (200_000, 4_096)
    } else if m.contains("gpt-4o") || m.contains("gpt-4-turbo") || m.contains("gpt-4-1") {
        (128_000, 16_384)
    } else if m.contains("gpt-4") {
        (8_192, 4_096)
    } else if m.contains("gpt-3.5") {
        (16_385, 4_096)
    } else if m.contains("gemini-1.5-pro") || m.contains("gemini-2") {
        (1_000_000, 8_192)
    } else if m.contains("gemini") || m.contains("deepseek") {
        (128_000, 8_192)
    } else {
        // Conservative default
        (200_000, 16_384)
    }
}

// ── State ───────────────────────────────────────────────────────────────────

/// Tracks context window usage
#[derive(Debug, Clone)]
pub struct ContextGauge {
    /// Total input tokens used so far
    pub input_tokens: usize,
    /// Total output tokens used so far
    pub output_tokens: usize,
    /// Cache creation tokens
    pub cache_creation_tokens: usize,
    /// Cache read tokens
    pub cache_read_tokens: usize,
    /// Context window size (from model)
    pub context_window: usize,
    /// Max output tokens (from model)
    pub max_output: usize,
}

impl Default for ContextGauge {
    fn default() -> Self {
        let (cw, mo) = model_context_window("claude-sonnet-4-5");
        Self {
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_tokens: 0,
            cache_read_tokens: 0,
            context_window: cw,
            max_output: mo,
        }
    }
}

impl ContextGauge {
    pub fn new(model: &str) -> Self {
        let (cw, mo) = model_context_window(model);
        Self {
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_tokens: 0,
            cache_read_tokens: 0,
            context_window: cw,
            max_output: mo,
        }
    }

    /// Update from a usage report
    pub fn update(&mut self, input: usize, output: usize, cache_create: usize, cache_read: usize) {
        self.input_tokens = input;
        self.output_tokens = output;
        self.cache_creation_tokens = cache_create;
        self.cache_read_tokens = cache_read;
    }

    /// Update the model (changes context window limits)
    pub fn set_model(&mut self, model: &str) {
        let (cw, mo) = model_context_window(model);
        self.context_window = cw;
        self.max_output = mo;
    }

    /// Total tokens in use
    pub fn total_used(&self) -> usize {
        self.input_tokens + self.output_tokens
    }

    /// Usage fraction (0.0 to 1.0)
    pub fn usage_fraction(&self) -> f64 {
        if self.context_window == 0 {
            return 0.0;
        }
        (self.total_used() as f64 / self.context_window as f64).min(1.0)
    }

    /// Color based on usage level
    pub fn usage_color(&self) -> Color {
        let frac = self.usage_fraction();
        if frac < 0.5 {
            Color::Green
        } else if frac < 0.75 {
            Color::Yellow
        } else if frac < 0.9 {
            Color::Rgb(255, 165, 0) // orange
        } else {
            Color::Red
        }
    }

    /// Format tokens as human-readable (e.g. "12.5k", "1.2M")
    pub fn format_tokens(n: usize) -> String {
        if n >= 1_000_000 {
            format!("{:.1}M", n as f64 / 1_000_000.0)
        } else if n >= 1_000 {
            format!("{:.1}k", n as f64 / 1_000.0)
        } else {
            n.to_string()
        }
    }

    /// Render as a compact status bar span: [████░░ 45k/200k]
    pub fn status_bar_span(&self) -> Span<'static> {
        let used = Self::format_tokens(self.total_used());
        let total = Self::format_tokens(self.context_window);
        let frac = self.usage_fraction();
        let color = self.usage_color();

        // Build a mini bar (8 chars wide)
        let bar_width = 8;
        let filled = (frac * bar_width as f64).round() as usize;
        let empty = bar_width - filled;
        let bar: String = format!("{}{}", "█".repeat(filled), "░".repeat(empty));

        let text = format!(" {}{}/{} ", bar, used, total);
        Span::styled(text, Style::default().fg(color).add_modifier(Modifier::BOLD))
    }

    /// Detailed summary string
    pub fn summary(&self) -> String {
        let frac = self.usage_fraction();
        format!(
            "Context: {}/{} ({:.0}%)\n  Input: {}\n  Output: {}\n  Cache create: {}\n  Cache read: {}",
            Self::format_tokens(self.total_used()),
            Self::format_tokens(self.context_window),
            frac * 100.0,
            Self::format_tokens(self.input_tokens),
            Self::format_tokens(self.output_tokens),
            Self::format_tokens(self.cache_creation_tokens),
            Self::format_tokens(self.cache_read_tokens),
        )
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_context_window_claude() {
        let (cw, _) = model_context_window("claude-sonnet-4-5");
        assert_eq!(cw, 200_000);
    }

    #[test]
    fn test_model_context_window_gpt4o() {
        let (cw, _) = model_context_window("gpt-4o");
        assert_eq!(cw, 128_000);
    }

    #[test]
    fn test_usage_fraction() {
        let mut g = ContextGauge::new("claude-sonnet-4-5");
        g.update(50_000, 10_000, 0, 0);
        let frac = g.usage_fraction();
        assert!((frac - 0.3).abs() < 0.01);
    }

    #[test]
    fn test_usage_color_green() {
        let mut g = ContextGauge::new("claude-sonnet-4-5");
        g.update(10_000, 0, 0, 0);
        assert_eq!(g.usage_color(), Color::Green);
    }

    #[test]
    fn test_usage_color_red() {
        let mut g = ContextGauge::new("claude-sonnet-4-5");
        g.update(190_000, 5_000, 0, 0);
        assert_eq!(g.usage_color(), Color::Red);
    }

    #[test]
    fn test_format_tokens() {
        assert_eq!(ContextGauge::format_tokens(500), "500");
        assert_eq!(ContextGauge::format_tokens(1_500), "1.5k");
        assert_eq!(ContextGauge::format_tokens(200_000), "200.0k");
        assert_eq!(ContextGauge::format_tokens(1_500_000), "1.5M");
    }

    #[test]
    fn test_set_model_updates_limits() {
        let mut g = ContextGauge::new("claude-sonnet-4-5");
        assert_eq!(g.context_window, 200_000);
        g.set_model("gpt-4o");
        assert_eq!(g.context_window, 128_000);
    }

    #[test]
    fn test_summary() {
        let mut g = ContextGauge::new("claude-sonnet-4-5");
        g.update(50_000, 10_000, 5_000, 20_000);
        let s = g.summary();
        assert!(s.contains("60.0k/200.0k"));
        assert!(s.contains("30%"));
    }
}
