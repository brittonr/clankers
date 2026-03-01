//! Todo panel — tracks a task list for the current session
//!
//! Items can be added by the agent (via the `todo` tool) or the user
//! (via `/todo` slash command). Implements [`Panel`] for unified layout,
//! key handling, and rendering.

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Modifier;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Wrap;

use crate::tui::panel::DrawContext;
use crate::tui::panel::ListNav;
use crate::tui::panel::Panel;
use crate::tui::panel::PanelAction;
use crate::tui::panel::PanelId;

// ── Data types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TodoStatus {
    Pending,
    InProgress,
    Done,
    Blocked,
}

impl TodoStatus {
    pub fn icon(&self) -> &'static str {
        match self {
            TodoStatus::Pending => "○",
            TodoStatus::InProgress => "◐",
            TodoStatus::Done => "●",
            TodoStatus::Blocked => "✕",
        }
    }

    pub fn color(&self) -> Color {
        match self {
            TodoStatus::Pending => Color::DarkGray,
            TodoStatus::InProgress => Color::Yellow,
            TodoStatus::Done => Color::Green,
            TodoStatus::Blocked => Color::Red,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            TodoStatus::Pending => "pending",
            TodoStatus::InProgress => "in-progress",
            TodoStatus::Done => "done",
            TodoStatus::Blocked => "blocked",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "pending" | "todo" => Some(TodoStatus::Pending),
            "in-progress" | "inprogress" | "active" | "wip" => Some(TodoStatus::InProgress),
            "done" | "complete" | "completed" => Some(TodoStatus::Done),
            "blocked" | "stuck" => Some(TodoStatus::Blocked),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TodoItem {
    pub id: usize,
    pub text: String,
    pub status: TodoStatus,
    pub note: Option<String>,
}

// ── Panel state ─────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct TodoPanel {
    pub items: Vec<TodoItem>,
    pub nav: ListNav,
    next_id: usize,
}

impl Default for TodoPanel {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            nav: ListNav::new(),
            next_id: 1,
        }
    }
}

impl TodoPanel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a new todo item, returns the assigned ID
    pub fn add(&mut self, text: String) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.items.push(TodoItem {
            id,
            text,
            status: TodoStatus::Pending,
            note: None,
        });
        id
    }

    /// Set status by ID. Returns true if found.
    pub fn set_status(&mut self, id: usize, status: TodoStatus) -> bool {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.status = status;
            true
        } else {
            false
        }
    }

    /// Set status by matching text (partial, case-insensitive). Returns the ID if found.
    pub fn set_status_by_text(&mut self, query: &str, status: TodoStatus) -> Option<usize> {
        let q = query.to_lowercase();
        if let Some(item) = self.items.iter_mut().find(|i| i.text.to_lowercase().contains(&q)) {
            item.status = status;
            Some(item.id)
        } else {
            None
        }
    }

    /// Add or update a note on a todo item
    pub fn set_note(&mut self, id: usize, note: String) -> bool {
        if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
            item.note = Some(note);
            true
        } else {
            false
        }
    }

    /// Remove a todo item by ID
    pub fn remove(&mut self, id: usize) -> bool {
        let before = self.items.len();
        self.items.retain(|i| i.id != id);
        self.nav.clamp(self.items.len());
        self.items.len() < before
    }

    /// Remove the currently selected item
    pub fn remove_selected(&mut self) {
        if !self.items.is_empty() {
            self.items.remove(self.nav.selected);
            self.nav.clamp(self.items.len());
        }
    }

    /// Clear all done items
    pub fn clear_done(&mut self) {
        self.items.retain(|i| i.status != TodoStatus::Done);
        self.nav.clamp(self.items.len());
    }

    /// Toggle selected item between Pending ↔ Done
    pub fn toggle_selected(&mut self) {
        if let Some(item) = self.items.get_mut(self.nav.selected) {
            item.status = match item.status {
                TodoStatus::Done => TodoStatus::Pending,
                _ => TodoStatus::Done,
            };
        }
    }

    /// Cycle selected item: Pending → InProgress → Done → Pending
    pub fn cycle_selected(&mut self) {
        if let Some(item) = self.items.get_mut(self.nav.selected) {
            item.status = match item.status {
                TodoStatus::Pending => TodoStatus::InProgress,
                TodoStatus::InProgress => TodoStatus::Done,
                TodoStatus::Done => TodoStatus::Pending,
                TodoStatus::Blocked => TodoStatus::Pending,
            };
        }
    }

    pub fn select_next(&mut self) {
        self.nav.next(self.items.len());
    }

    pub fn select_prev(&mut self) {
        self.nav.prev(self.items.len());
    }

    /// Summary string for /todo list
    pub fn summary(&self) -> String {
        if self.items.is_empty() {
            return "No todo items.".to_string();
        }
        let pending = self.items.iter().filter(|i| i.status == TodoStatus::Pending).count();
        let active = self.items.iter().filter(|i| i.status == TodoStatus::InProgress).count();
        let done = self.items.iter().filter(|i| i.status == TodoStatus::Done).count();
        let blocked = self.items.iter().filter(|i| i.status == TodoStatus::Blocked).count();

        let mut out = format!(
            "{} item(s): {} pending, {} active, {} done, {} blocked\n",
            self.items.len(),
            pending,
            active,
            done,
            blocked,
        );
        for item in &self.items {
            let note_str = item.note.as_ref().map(|n| format!(" ({})", n)).unwrap_or_default();
            out.push_str(&format!("  {} [#{}] {}{}\n", item.status.icon(), item.id, item.text, note_str));
        }
        out
    }
}

// ── Panel trait impl ────────────────────────────────────────────────────────

impl Panel for TodoPanel {
    fn id(&self) -> PanelId {
        PanelId::Todo
    }

    fn title(&self) -> String {
        let pending = self.items.iter().filter(|i| i.status != TodoStatus::Done).count();
        let total = self.items.len();
        format!("Todo ({}/{})", pending, total)
    }

    fn focus_hints(&self) -> &'static str {
        " j/k Tab ⎵:cycle "
    }

    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn empty_text(&self) -> &'static str {
        "No items. Use /todo add <text> or the todo tool."
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> Option<PanelAction> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.select_next();
                Some(PanelAction::Consumed)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.select_prev();
                Some(PanelAction::Consumed)
            }
            KeyCode::Char(' ') => {
                self.cycle_selected();
                Some(PanelAction::Consumed)
            }
            KeyCode::Char('x') | KeyCode::Delete => {
                self.remove_selected();
                Some(PanelAction::Consumed)
            }
            KeyCode::Char('D') => {
                self.clear_done();
                Some(PanelAction::Consumed)
            }
            KeyCode::Enter => {
                self.toggle_selected();
                Some(PanelAction::Consumed)
            }
            KeyCode::Esc => Some(PanelAction::Unfocus),
            _ => None, // not handled — bubble up
        }
    }

    fn handle_scroll(&mut self, up: bool, lines: u16) {
        let len = self.items.len();
        for _ in 0..lines {
            if up {
                self.nav.prev(len);
            } else {
                self.nav.next(len);
            }
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
        let mut lines = Vec::new();
        for (i, item) in self.items.iter().enumerate() {
            let icon = item.status.icon();
            let color = item.status.color();

            let mut spans = vec![
                self.nav.prefix_span(i, ctx.focused),
                Span::styled(format!("{} ", icon), Style::default().fg(color)),
            ];

            let text_style = if item.status == TodoStatus::Done {
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::CROSSED_OUT)
            } else {
                self.nav.item_style(i, ctx.focused, Style::default().fg(ctx.theme.fg))
            };
            spans.push(Span::styled(&item.text, text_style));

            // Show ID dimmed
            spans.push(Span::styled(format!(" #{}", item.id), Style::default().fg(Color::DarkGray)));

            lines.push(Line::from(spans));

            // Show note indented if present
            if let Some(ref note) = item.note {
                lines.push(Line::from(vec![
                    Span::raw("     "),
                    Span::styled(format!("↳ {}", note), Style::default().fg(Color::DarkGray)),
                ]));
            }
        }

        let scroll = self.nav.scroll_offset(area.height as usize, 1);
        let para = Paragraph::new(lines).scroll((scroll, 0)).wrap(Wrap { trim: false });
        frame.render_widget(para, area);
    }
}

// ── Legacy render function (bridge to Panel trait) ──────────────────────────

/// Render the todo panel using the Panel trait infrastructure.
pub fn render_todo_panel(
    frame: &mut Frame,
    panel: &TodoPanel,
    theme: &crate::tui::theme::Theme,
    area: Rect,
    focused: bool,
) {
    use crate::tui::panel::draw_panel;
    let ctx = DrawContext { theme, focused };
    draw_panel(frame, panel, area, &ctx);
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crossterm::event::KeyModifiers;

    use super::*;

    #[test]
    fn test_add_and_summary() {
        let mut panel = TodoPanel::new();
        let id = panel.add("Write tests".into());
        assert_eq!(id, 1);
        assert!(!panel.is_empty());
        assert!(panel.summary().contains("Write tests"));
    }

    #[test]
    fn test_set_status() {
        let mut panel = TodoPanel::new();
        let id = panel.add("Task 1".into());
        assert!(panel.set_status(id, TodoStatus::Done));
        assert_eq!(panel.items[0].status, TodoStatus::Done);
    }

    #[test]
    fn test_set_status_by_text() {
        let mut panel = TodoPanel::new();
        panel.add("Fix the bug".into());
        panel.add("Write docs".into());
        let found = panel.set_status_by_text("bug", TodoStatus::InProgress);
        assert_eq!(found, Some(1));
        assert_eq!(panel.items[0].status, TodoStatus::InProgress);
    }

    #[test]
    fn test_toggle_and_cycle() {
        let mut panel = TodoPanel::new();
        panel.add("Task".into());
        panel.toggle_selected();
        assert_eq!(panel.items[0].status, TodoStatus::Done);
        panel.toggle_selected();
        assert_eq!(panel.items[0].status, TodoStatus::Pending);

        panel.cycle_selected();
        assert_eq!(panel.items[0].status, TodoStatus::InProgress);
        panel.cycle_selected();
        assert_eq!(panel.items[0].status, TodoStatus::Done);
        panel.cycle_selected();
        assert_eq!(panel.items[0].status, TodoStatus::Pending);
    }

    #[test]
    fn test_remove_and_clear_done() {
        let mut panel = TodoPanel::new();
        panel.add("A".into());
        panel.add("B".into());
        panel.add("C".into());
        panel.set_status(2, TodoStatus::Done);
        panel.clear_done();
        assert_eq!(panel.items.len(), 2);
        assert!(panel.items.iter().all(|i| i.status != TodoStatus::Done));

        panel.remove(1);
        assert_eq!(panel.items.len(), 1);
        assert_eq!(panel.items[0].text, "C");
    }

    #[test]
    fn test_navigation() {
        let mut panel = TodoPanel::new();
        panel.add("A".into());
        panel.add("B".into());
        panel.add("C".into());
        panel.nav.selected = 0;
        panel.select_next();
        assert_eq!(panel.nav.selected, 1);
        panel.select_prev();
        assert_eq!(panel.nav.selected, 0);
        panel.select_prev(); // wraps
        assert_eq!(panel.nav.selected, 2);
    }

    #[test]
    fn test_panel_trait_title() {
        let mut panel = TodoPanel::new();
        assert_eq!(panel.title(), "Todo (0/0)");
        panel.add("Task".into());
        assert_eq!(panel.title(), "Todo (1/1)");
        panel.set_status(1, TodoStatus::Done);
        assert_eq!(panel.title(), "Todo (0/1)");
    }

    #[test]
    fn test_handle_key_event() {
        let mut panel = TodoPanel::new();
        panel.add("A".into());
        panel.add("B".into());

        let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(panel.handle_key_event(key), Some(PanelAction::Consumed));
        assert_eq!(panel.nav.selected, 1);

        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(panel.handle_key_event(key), Some(PanelAction::Unfocus));

        // Unknown key returns None (not handled)
        let key = KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE);
        assert_eq!(panel.handle_key_event(key), None);
    }
}
