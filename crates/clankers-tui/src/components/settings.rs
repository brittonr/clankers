//! Settings panel overlay

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
use ratatui::widgets::Wrap;

pub struct SettingsPanel {
    pub visible: bool,
    pub entries: Vec<SettingEntry>,
    pub selected: usize,
}

pub struct SettingEntry {
    pub key: String,
    pub value: String,
    pub description: String,
}

impl SettingsPanel {
    pub fn new(entries: Vec<SettingEntry>) -> Self {
        Self {
            visible: false,
            entries,
            selected: 0,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        self.selected = (self.selected + 1).min(self.entries.len().saturating_sub(1));
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if !self.visible {
            return;
        }

        let width = 60.min(area.width.saturating_sub(4));
        let height = (self.entries.len() as u16 * 2 + 2).min(area.height.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let popup = Rect::new(x, y, width, height);

        frame.render_widget(Clear, popup);
        let block = Block::default()
            .title(" Settings ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue));

        let inner = block.inner(popup);
        frame.render_widget(block, popup);

        let lines: Vec<Line> = self
            .entries
            .iter()
            .enumerate()
            .flat_map(|(i, entry)| {
                let style = if i == self.selected {
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                vec![
                    Line::from(vec![
                        Span::styled(&entry.key, style),
                        Span::styled(" = ", Style::default().fg(Color::DarkGray)),
                        Span::styled(&entry.value, Style::default().fg(Color::Yellow)),
                    ]),
                    Line::from(Span::styled(format!("  {}", entry.description), Style::default().fg(Color::DarkGray))),
                ]
            })
            .collect();

        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
    }
}
