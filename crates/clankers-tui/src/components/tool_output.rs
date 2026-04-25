//! Tool output display (collapsible)

#![allow(unexpected_cfgs)]
#![cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        compound_assertion,
        ignored_result,
        no_unwrap,
        no_panic,
        no_todo,
        unjustified_no_todo_allow,
        no_recursion,
        unchecked_narrowing,
        unchecked_division,
        unbounded_loop,
        catch_all_on_enum,
        explicit_defaults,
        unbounded_channel,
        unbounded_collection_growth,
        assertion_density,
        raw_arithmetic_overflow,
        sentinel_fallback,
        acronym_style,
        bool_naming,
        negated_predicate,
        numeric_units,
        float_for_currency,
        function_length,
        nested_conditionals,
        platform_dependent_cast,
        usize_in_public_api,
        too_many_parameters,
        compound_condition,
        unjustified_allow,
        ambiguous_params,
        ambient_clock,
        verified_purity,
        contradictory_time,
        multi_lock_ordering,
        reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"
    )
)]

use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;

/// A tool output block in the chat
pub struct ToolOutputBlock {
    pub tool_name: String,
    pub output: String,
    pub is_error: bool,
    pub collapsed: bool,
}

impl ToolOutputBlock {
    pub fn new(tool_name: String, output: String, is_error: bool) -> Self {
        // Auto-collapse if output is long
        let is_collapsed = output.lines().count() > 10;
        Self {
            tool_name,
            output,
            is_error,
            collapsed: is_collapsed,
        }
    }

    pub fn toggle(&mut self) {
        self.collapsed = !self.collapsed;
    }

    /// Render to lines
    pub fn render(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        let icon = if self.collapsed { "▶" } else { "▼" };
        let status = if self.is_error { "✗" } else { "✓" };
        let color = if self.is_error { Color::Red } else { Color::Green };

        lines.push(Line::from(vec![
            Span::styled(format!("{} ", icon), Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{} ", status), Style::default().fg(color)),
            Span::styled(self.tool_name.clone(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]));

        if !self.collapsed {
            let max_lines = 50;
            for (i, line) in self.output.lines().enumerate() {
                if i >= max_lines {
                    lines.push(Line::from(Span::styled(
                        format!("  ... ({} more lines)", self.output.lines().count() - max_lines),
                        Style::default().fg(Color::DarkGray),
                    )));
                    break;
                }
                lines.push(Line::from(Span::styled(format!("  {}", line), Style::default().fg(Color::DarkGray))));
            }
        }
        lines
    }
}
