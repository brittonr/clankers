//! Tool toggle popup — enable/disable tools per session.
//!
//! Renders a centered popup with a filterable, scrollable list of tools.
//! Each tool has a checkbox that can be toggled with Enter or Space.
//! Changes take effect immediately (caller rebuilds the agent tool set).

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

use std::collections::HashSet;

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
use ratatui::widgets::List;
use ratatui::widgets::ListItem;
use ratatui::widgets::Paragraph;

/// A single tool entry in the toggle list.
#[derive(Debug, Clone)]
pub struct ToolEntry {
    pub name: String,
    pub description: String,
    pub source: String,
    pub enabled: bool,
}

/// Tool toggle popup state.
pub struct ToolToggle {
    pub visible: bool,
    pub entries: Vec<ToolEntry>,
    pub filter: String,
    pub selected: usize,
    /// Tool names that were toggled (added or removed from disabled set)
    /// since the popup opened. Caller checks this to know if a rebuild is needed.
    pub dirty: bool,
    /// Scope indicator shown in the title bar.
    pub scope: ToolToggleScope,
}

/// Where the toggle state will be saved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolToggleScope {
    /// Session-only (no persistence)
    Session,
    /// Save to .clankers/settings.json (per-repo)
    Project,
    /// Save to ~/.clankers/agent/settings.json (global)
    Global,
}

impl std::fmt::Display for ToolToggleScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Session => write!(f, "session"),
            Self::Project => write!(f, "project"),
            Self::Global => write!(f, "global"),
        }
    }
}

impl ToolToggle {
    pub fn new() -> Self {
        Self {
            visible: false,
            entries: Vec::new(),
            filter: String::new(),
            selected: 0,
            dirty: false,
            scope: ToolToggleScope::Session,
        }
    }

    /// Open the toggle popup with the given tools and disabled set.
    pub fn open(&mut self, tools: Vec<(String, String, String)>, disabled: &HashSet<String>) {
        self.entries = tools
            .into_iter()
            .map(|(name, description, source)| ToolEntry {
                enabled: !disabled.contains(&name),
                name,
                description,
                source,
            })
            .collect();
        self.filter.clear();
        self.selected = 0;
        self.dirty = false;
        self.visible = true;
    }

    pub fn close(&mut self) {
        self.visible = false;
    }

    /// Get the indices of filtered entries (matching the current filter).
    fn filtered_indices(&self) -> Vec<usize> {
        let filter_lower = self.filter.to_lowercase();
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, e)| filter_lower.is_empty() || e.name.to_lowercase().contains(&filter_lower))
            .map(|(i, _)| i)
            .collect()
    }

    /// Toggle the currently selected tool.
    pub fn toggle_selected(&mut self) {
        let filtered = self.filtered_indices();
        if let Some(&idx) = filtered.get(self.selected) {
            self.entries[idx].enabled = !self.entries[idx].enabled;
            self.dirty = true;
        }
    }

    /// Enable all visible (filtered) tools.
    pub fn enable_all(&mut self) {
        for &idx in &self.filtered_indices() {
            if !self.entries[idx].enabled {
                self.entries[idx].enabled = true;
                self.dirty = true;
            }
        }
    }

    /// Disable all visible (filtered) tools.
    pub fn disable_all(&mut self) {
        for &idx in &self.filtered_indices() {
            if self.entries[idx].enabled {
                self.entries[idx].enabled = false;
                self.dirty = true;
            }
        }
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn move_down(&mut self) {
        let max = self.filtered_indices().len().saturating_sub(1);
        self.selected = (self.selected + 1).min(max);
    }

    pub fn type_char(&mut self, c: char) {
        self.filter.push(c);
        self.selected = 0;
    }

    pub fn backspace(&mut self) {
        self.filter.pop();
        self.selected = 0;
    }

    /// Cycle the scope: session → project → global → session.
    pub fn cycle_scope(&mut self) {
        self.scope = match self.scope {
            ToolToggleScope::Session => ToolToggleScope::Project,
            ToolToggleScope::Project => ToolToggleScope::Global,
            ToolToggleScope::Global => ToolToggleScope::Session,
        };
    }

    /// Get the current set of disabled tool names.
    pub fn disabled_set(&self) -> HashSet<String> {
        self.entries.iter().filter(|e| !e.enabled).map(|e| e.name.clone()).collect()
    }

    /// Summary line: "N/M tools enabled"
    fn summary(&self) -> String {
        let enabled = self.entries.iter().filter(|e| e.enabled).count();
        let total = self.entries.len();
        format!("{}/{} enabled", enabled, total)
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if !self.visible {
            return;
        }

        let width = 70.min(area.width.saturating_sub(4));
        let height = 24.min(area.height.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let popup_area = Rect::new(x, y, width, height);

        frame.render_widget(Clear, popup_area);

        let title = format!(" Tools [{}] — {} ", self.scope, self.summary());
        let block = Block::default().title(title).borders(Borders::ALL).border_style(Style::default().fg(Color::Blue));

        let inner = block.inner(popup_area);
        frame.render_widget(block, popup_area);

        if inner.height < 3 {
            return;
        }

        // Filter input (row 0)
        let filter_line = Line::from(vec![
            Span::styled("Filter: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&self.filter, Style::default().fg(Color::White)),
            Span::styled("_", Style::default().fg(Color::White).add_modifier(Modifier::SLOW_BLINK)),
        ]);
        let filter_area = Rect::new(inner.x, inner.y, inner.width, 1);
        frame.render_widget(Paragraph::new(filter_line), filter_area);

        // Tool list (rows 1..height-1)
        let filtered = self.filtered_indices();
        let list_height = inner.height.saturating_sub(2) as usize;

        // Scroll offset to keep selected visible
        let scroll_offset = if self.selected >= list_height {
            self.selected - list_height + 1
        } else {
            0
        };

        let items: Vec<ListItem> = filtered
            .iter()
            .skip(scroll_offset)
            .take(list_height)
            .enumerate()
            .map(|(display_idx, &entry_idx)| {
                let entry = &self.entries[entry_idx];
                let is_selected = display_idx + scroll_offset == self.selected;

                let checkbox = if entry.enabled { "[✓]" } else { "[ ]" };
                let check_style = if entry.enabled {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                let name_style = if is_selected {
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
                } else if entry.enabled {
                    Style::default().fg(Color::White)
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                let desc = if entry.description.len() > 40 {
                    format!("{}…", &entry.description[..39])
                } else {
                    entry.description.clone()
                };

                let bg = if is_selected {
                    Style::default().bg(Color::DarkGray)
                } else {
                    Style::default()
                };

                let line = Line::from(vec![
                    Span::styled(format!(" {} ", checkbox), check_style.patch(bg)),
                    Span::styled(format!("{:<20}", entry.name), name_style.patch(bg)),
                    Span::styled(format!(" {}", desc), Style::default().fg(Color::DarkGray).patch(bg)),
                ]);

                ListItem::new(line)
            })
            .collect();

        let list_area = Rect::new(inner.x, inner.y + 1, inner.width, inner.height.saturating_sub(2));
        frame.render_widget(List::new(items), list_area);

        // Footer hint (last row)
        let footer = Line::from(vec![
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::styled(":toggle  ", Style::default().fg(Color::DarkGray)),
            Span::styled("a", Style::default().fg(Color::Yellow)),
            Span::styled(":all on  ", Style::default().fg(Color::DarkGray)),
            Span::styled("n", Style::default().fg(Color::Yellow)),
            Span::styled(":all off  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Tab", Style::default().fg(Color::Yellow)),
            Span::styled(":scope  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::styled(":close", Style::default().fg(Color::DarkGray)),
        ]);
        let footer_area = Rect::new(inner.x, inner.y + inner.height - 1, inner.width, 1);
        frame.render_widget(Paragraph::new(footer), footer_area);
    }
}

impl Default for ToolToggle {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn sample_tools() -> Vec<(String, String, String)> {
        vec![
            ("read".into(), "Read files".into(), "built-in".into()),
            ("write".into(), "Write files".into(), "built-in".into()),
            ("bash".into(), "Run commands".into(), "built-in".into()),
            ("grep".into(), "Search files".into(), "built-in".into()),
            ("calendar".into(), "CalDAV tool".into(), "plugin".into()),
        ]
    }

    #[test]
    fn open_populates_entries() {
        let mut toggle = ToolToggle::new();
        let disabled: HashSet<String> = ["bash".to_string()].into_iter().collect();

        toggle.open(sample_tools(), &disabled);

        assert!(toggle.visible);
        assert_eq!(toggle.entries.len(), 5);
        assert!(toggle.entries.iter().find(|e| e.name == "read").unwrap().enabled);
        assert!(!toggle.entries.iter().find(|e| e.name == "bash").unwrap().enabled);
    }

    #[test]
    fn toggle_selected_flips_state() {
        let mut toggle = ToolToggle::new();
        toggle.open(sample_tools(), &HashSet::new());

        assert!(toggle.entries[0].enabled);
        toggle.toggle_selected();
        assert!(!toggle.entries[0].enabled);
        assert!(toggle.dirty);

        toggle.toggle_selected();
        assert!(toggle.entries[0].enabled);
    }

    #[test]
    fn filter_narrows_list() {
        let mut toggle = ToolToggle::new();
        toggle.open(sample_tools(), &HashSet::new());

        toggle.type_char('c');
        toggle.type_char('a');
        toggle.type_char('l');
        let filtered = toggle.filtered_indices();
        // Only "calendar" contains "cal"
        assert_eq!(filtered.len(), 1);
        assert_eq!(toggle.entries[filtered[0]].name, "calendar");
    }

    #[test]
    fn disabled_set_reflects_state() {
        let mut toggle = ToolToggle::new();
        toggle.open(sample_tools(), &HashSet::new());

        // Disable first two
        toggle.toggle_selected();
        toggle.move_down();
        toggle.toggle_selected();

        let disabled = toggle.disabled_set();
        assert_eq!(disabled.len(), 2);
        assert!(disabled.contains("read"));
        assert!(disabled.contains("write"));
    }

    #[test]
    fn enable_all_enables_everything() {
        let mut toggle = ToolToggle::new();
        let disabled: HashSet<String> = ["bash".into(), "grep".into()].into_iter().collect();
        toggle.open(sample_tools(), &disabled);

        assert_eq!(toggle.disabled_set().len(), 2);
        toggle.enable_all();
        assert_eq!(toggle.disabled_set().len(), 0);
        assert!(toggle.dirty);
    }

    #[test]
    fn disable_all_disables_everything() {
        let mut toggle = ToolToggle::new();
        toggle.open(sample_tools(), &HashSet::new());

        toggle.disable_all();
        assert_eq!(toggle.disabled_set().len(), 5);
    }

    #[test]
    fn cycle_scope_rotates() {
        let mut toggle = ToolToggle::new();
        assert_eq!(toggle.scope, ToolToggleScope::Session);

        toggle.cycle_scope();
        assert_eq!(toggle.scope, ToolToggleScope::Project);

        toggle.cycle_scope();
        assert_eq!(toggle.scope, ToolToggleScope::Global);

        toggle.cycle_scope();
        assert_eq!(toggle.scope, ToolToggleScope::Session);
    }

    #[test]
    fn navigation_clamps() {
        let mut toggle = ToolToggle::new();
        toggle.open(sample_tools(), &HashSet::new());

        toggle.move_up();
        assert_eq!(toggle.selected, 0);

        for _ in 0..20 {
            toggle.move_down();
        }
        assert_eq!(toggle.selected, 4);
    }

    #[test]
    fn close_hides() {
        let mut toggle = ToolToggle::new();
        toggle.open(sample_tools(), &HashSet::new());
        assert!(toggle.visible);
        toggle.close();
        assert!(!toggle.visible);
    }
}
