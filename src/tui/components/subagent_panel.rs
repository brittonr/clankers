//! Subagent panel — shows live output from running subagents
//!
//! Two modes:
//! - **List view**: all subagents stacked, each showing tail of output
//! - **Detail view**: full scrollable log for one subagent (Enter to open, Esc to close)

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
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

use crate::tui::panel::DrawContext;
use crate::tui::panel::Panel;
use crate::tui::panel::PanelAction;
use crate::tui::panel::PanelId;
use crate::tui::theme::Theme;

/// Status of a subagent
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubagentStatus {
    Running,
    Done,
    Error,
}

/// A tracked subagent with its output
#[derive(Debug, Clone)]
pub struct SubagentEntry {
    pub id: String,
    pub name: String,
    pub task: String,
    pub status: SubagentStatus,
    pub output_lines: Vec<String>,
    /// Process ID (for kill support)
    pub pid: Option<u32>,
}

/// Panel view mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelView {
    /// Show all subagents stacked with tail output
    List,
    /// Full scrollable log for the selected subagent
    Detail,
}

/// State for the subagent panel
#[derive(Debug)]
pub struct SubagentPanel {
    /// All tracked subagents (active + completed)
    pub entries: Vec<SubagentEntry>,
    /// Index of the currently selected subagent
    pub selected: usize,
    /// Current view mode
    pub view: PanelView,
    /// Scroll offset for detail view (Cell so draw(&self) can clamp it)
    pub scroll_offset: std::cell::Cell<u16>,
}

impl Default for SubagentPanel {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            selected: 0,
            view: PanelView::List,
            scroll_offset: std::cell::Cell::new(0),
        }
    }
}

impl SubagentPanel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_visible(&self) -> bool {
        !self.entries.is_empty()
    }

    // ── Scroll helpers (abstract over Cell) ─────────────────────────

    pub fn scroll_up(&self, n: u16) {
        self.scroll_offset.set(self.scroll_offset.get().saturating_sub(n));
    }

    pub fn scroll_down(&self, n: u16) {
        self.scroll_offset.set(self.scroll_offset.get().saturating_add(n));
    }

    pub fn scroll_to_top(&self) {
        self.scroll_offset.set(0);
    }

    pub fn scroll_to_bottom(&self) {
        self.scroll_offset.set(u16::MAX);
    }

    pub fn add(&mut self, id: String, name: String, task: String, pid: Option<u32>) {
        self.entries.push(SubagentEntry {
            id,
            name,
            task,
            status: SubagentStatus::Running,
            output_lines: Vec::new(),
            pid,
        });
        self.selected = self.entries.len() - 1;
    }

    pub fn append_output(&mut self, id: &str, line: &str) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.output_lines.push(line.to_string());
        }
        // Auto-scroll in detail view if viewing this entry
        if self.view == PanelView::Detail
            && let Some(idx) = self.entries.iter().position(|e| e.id == id)
            && idx == self.selected
        {
            self.scroll_offset.set(u16::MAX);
        }
    }

    pub fn mark_done(&mut self, id: &str) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.status = SubagentStatus::Done;
        }
    }

    pub fn mark_error(&mut self, id: &str) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.status = SubagentStatus::Error;
        }
    }

    pub fn clear_done(&mut self) {
        self.entries.retain(|e| e.status == SubagentStatus::Running);
        if self.selected >= self.entries.len() {
            self.selected = self.entries.len().saturating_sub(1);
        }
    }

    pub fn next_tab(&mut self) {
        if !self.entries.is_empty() {
            self.selected = (self.selected + 1) % self.entries.len();
            self.scroll_offset.set(u16::MAX);
        }
    }

    pub fn prev_tab(&mut self) {
        if !self.entries.is_empty() {
            self.selected = if self.selected == 0 {
                self.entries.len() - 1
            } else {
                self.selected - 1
            };
            self.scroll_offset.set(u16::MAX);
        }
    }

    /// Open detail view for the selected subagent
    pub fn open_detail(&mut self) {
        if !self.entries.is_empty() {
            self.view = PanelView::Detail;
            self.scroll_offset.set(u16::MAX); // start at bottom
        }
    }

    /// Return to list view
    pub fn close_detail(&mut self) {
        self.view = PanelView::List;
    }

    pub fn running_count(&self) -> usize {
        self.entries.iter().filter(|e| e.status == SubagentStatus::Running).count()
    }

    /// Get the selected entry (if any)
    pub fn selected_entry(&self) -> Option<&SubagentEntry> {
        self.entries.get(self.selected)
    }

    /// Get the selected entry's ID (if any)
    pub fn selected_id(&self) -> Option<String> {
        self.entries.get(self.selected).map(|e| e.id.clone())
    }

    /// Remove the selected entry from the panel (dismiss it regardless of status)
    pub fn remove_selected(&mut self) {
        if !self.entries.is_empty() {
            self.entries.remove(self.selected);
            if self.selected >= self.entries.len() && !self.entries.is_empty() {
                self.selected = self.entries.len() - 1;
            }
            if self.entries.is_empty() {
                self.view = PanelView::List;
            }
        }
    }

    /// Remove an entry by ID
    pub fn remove_by_id(&mut self, id: &str) {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            self.entries.remove(pos);
            if self.selected >= self.entries.len() && !self.entries.is_empty() {
                self.selected = self.entries.len() - 1;
            }
            if self.entries.is_empty() {
                self.view = PanelView::List;
            }
        }
    }

    /// Get an entry by ID
    pub fn get_by_id(&self, id: &str) -> Option<&SubagentEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Get a summary of all subagents for display
    pub fn summary(&self) -> String {
        if self.entries.is_empty() {
            return "No subagents.".to_string();
        }
        let mut out = format!("{} subagent(s):\n", self.entries.len());
        for entry in &self.entries {
            let (icon, _) = status_icon_color(&entry.status);
            let pid_str = entry.pid.map(|p| format!(" (pid {})", p)).unwrap_or_default();
            out.push_str(&format!(
                "  {} [{}] {}{} — {}\n",
                icon,
                entry.id,
                entry.name,
                pid_str,
                entry.task.chars().take(50).collect::<String>()
            ));
        }
        out
    }
}

// ── Rendering ───────────────────────────────────────────────────────────────

fn status_icon_color(status: &SubagentStatus) -> (&'static str, Color) {
    match status {
        SubagentStatus::Running => ("⏳", Color::Yellow),
        SubagentStatus::Done => ("✓", Color::Green),
        SubagentStatus::Error => ("✗", Color::Red),
    }
}

// ── Panel trait impl ────────────────────────────────────────────────────────

impl Panel for SubagentPanel {
    fn id(&self) -> PanelId {
        PanelId::Subagents
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn close_detail_view(&mut self) {
        self.close_detail();
    }

    fn title(&self) -> String {
        let running = self.running_count();
        let total = self.entries.len();
        format!("Subagents ({}/{})", running, total)
    }

    fn focus_hints(&self) -> &'static str {
        " j/k Tab Enter:open "
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn empty_text(&self) -> &'static str {
        "No subagents running"
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> Option<PanelAction> {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.next_tab();
                Some(PanelAction::Consumed)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.prev_tab();
                Some(PanelAction::Consumed)
            }
            KeyCode::Enter => {
                // Focus the subagent's dedicated BSP pane (if it has one)
                if let Some(entry) = self.entries.get(self.selected) {
                    Some(PanelAction::FocusSubagent(entry.id.clone()))
                } else {
                    self.open_detail();
                    Some(PanelAction::Consumed)
                }
            }
            KeyCode::Esc if self.view == PanelView::Detail => {
                self.close_detail();
                Some(PanelAction::Consumed)
            }
            KeyCode::Esc => Some(PanelAction::Unfocus),
            KeyCode::Char('X') if self.view == PanelView::Detail => {
                self.remove_selected();
                Some(PanelAction::Consumed)
            }
            _ => None,
        }
    }

    fn handle_scroll(&mut self, up: bool, lines: u16) {
        if up {
            self.scroll_up(lines);
        } else {
            self.scroll_down(lines);
        }
    }

    fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
        if area.width < 10 || area.height < 4 {
            return;
        }
        // SubagentPanel's detail view needs &mut for scroll_offset,
        // but draw takes &self. We delegate to the legacy render for now.
        // The legacy render_subagent_panel handles both views.
        match self.view {
            PanelView::List => render_list_view_immut(frame, self, ctx.theme, area, ctx.focused),
            PanelView::Detail => render_detail_view_immut(frame, self, ctx.theme, area, ctx.focused),
        }
    }
}

/// Render the subagent panel
pub fn render_subagent_panel(frame: &mut Frame, panel: &mut SubagentPanel, theme: &Theme, area: Rect, focused: bool) {
    if area.width < 10 || area.height < 4 {
        return;
    }

    match panel.view {
        PanelView::List => render_list_view(frame, panel, theme, area, focused),
        PanelView::Detail => render_detail_view(frame, panel, theme, area, focused),
    }
}

// ── List view ───────────────────────────────────────────────────────────────

fn render_list_view(frame: &mut Frame, panel: &mut SubagentPanel, theme: &Theme, area: Rect, focused: bool) {
    use ratatui::layout::Constraint;
    use ratatui::layout::Direction;
    use ratatui::layout::Layout;

    let running = panel.running_count();
    let total = panel.entries.len();
    let focus_hint = if focused { " j/k Tab Enter:open " } else { "" };
    let title = format!(" Subagents ({}/{}) {}", running, total, focus_hint);

    let border_color = if focused { Color::Cyan } else { theme.border };
    let outer = Block::default().borders(Borders::ALL).border_style(Style::default().fg(border_color)).title(title);

    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    if panel.entries.is_empty() {
        let empty =
            Paragraph::new(Line::from(Span::styled("No subagents running", Style::default().fg(Color::DarkGray))));
        frame.render_widget(empty, inner);
        return;
    }

    let n = panel.entries.len();
    let available = inner.height;

    // Distribute space evenly, minimum 2 lines per entry (header + 1 line output)
    let min_per = 2u16;
    let constraints: Vec<Constraint> = if n as u16 * min_per <= available {
        let base = available / n as u16;
        let remainder = available % n as u16;
        (0..n).map(|i| Constraint::Length(if i == 0 { base + remainder } else { base })).collect()
    } else {
        // Too many — give each the minimum
        (0..n).map(|_| Constraint::Length(min_per)).collect()
    };

    let chunks = Layout::default().direction(Direction::Vertical).constraints(constraints).split(inner);

    for (i, entry) in panel.entries.iter().enumerate() {
        let chunk = chunks[i];
        if chunk.height == 0 {
            continue;
        }
        let is_selected = focused && i == panel.selected;
        render_list_entry(frame, entry, chunk, is_selected, theme);
    }
}

/// Render one entry in list view: header + tail of output
fn render_list_entry(frame: &mut Frame, entry: &SubagentEntry, area: Rect, is_selected: bool, _theme: &Theme) {
    let (icon, color) = status_icon_color(&entry.status);

    let header_style = if is_selected {
        Style::default().fg(Color::Black).bg(color).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(color)
    };

    let task_preview: String = entry.task.chars().take(40).collect();
    let header_text = format!(" {} {} — {} ", icon, entry.name, task_preview);
    let header = Line::from(Span::styled(header_text, header_style));

    if area.height == 1 {
        frame.render_widget(Paragraph::new(header), area);
        return;
    }

    // Header on first line
    let header_area = Rect { height: 1, ..area };
    frame.render_widget(Paragraph::new(header), header_area);

    // Output tail on remaining lines
    let output_area = Rect {
        y: area.y + 1,
        height: area.height - 1,
        ..area
    };

    let visible = output_area.height as usize;
    let lines: Vec<Line> = if entry.output_lines.is_empty() {
        let msg = if entry.status == SubagentStatus::Running {
            "  starting..."
        } else {
            "  (no output)"
        };
        vec![Line::from(Span::styled(msg, Style::default().fg(Color::DarkGray)))]
    } else {
        let skip = entry.output_lines.len().saturating_sub(visible);
        entry
            .output_lines
            .iter()
            .skip(skip)
            .take(visible)
            .map(|l| Line::from(Span::styled(format!("  {}", l), Style::default().fg(Color::DarkGray))))
            .collect()
    };

    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), output_area);
}

// ── Detail view ─────────────────────────────────────────────────────────────

fn render_detail_view(frame: &mut Frame, panel: &mut SubagentPanel, theme: &Theme, area: Rect, focused: bool) {
    let entry = match panel.entries.get(panel.selected) {
        Some(e) => e,
        None => {
            panel.view = PanelView::List;
            return;
        }
    };

    let (icon, color) = status_icon_color(&entry.status);
    let total_lines = entry.output_lines.len();
    let title = format!(
        " {} {} — {} [{} lines] Esc:back x:kill X:remove ",
        icon,
        entry.name,
        entry.task.chars().take(20).collect::<String>(),
        total_lines,
    );

    let border_color = if focused { Color::Cyan } else { theme.border };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(title, Style::default().fg(color).add_modifier(Modifier::BOLD)));

    let content_area = block.inner(area);
    frame.render_widget(block, area);

    if content_area.height == 0 {
        return;
    }

    let lines: Vec<Line> = if entry.output_lines.is_empty() {
        vec![Line::from(Span::styled(
            format!("Task: {}\n\nWaiting for output...", entry.task),
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        entry.output_lines.iter().map(|l| Line::from(l.as_str())).collect()
    };

    let total = lines.len() as u16;
    let max_scroll = total.saturating_sub(content_area.height);
    let scroll = panel.scroll_offset.get().min(max_scroll);
    panel.scroll_offset.set(scroll);

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((scroll, 0));

    frame.render_widget(paragraph, content_area);
}

// ── Immutable render functions (for Panel::draw) ────────────────────────────

/// List view — immutable (no &mut needed)
fn render_list_view_immut(frame: &mut Frame, panel: &SubagentPanel, theme: &Theme, area: Rect, focused: bool) {
    use ratatui::layout::Constraint;
    use ratatui::layout::Direction;
    use ratatui::layout::Layout;

    if panel.entries.is_empty() {
        let empty =
            Paragraph::new(Line::from(Span::styled("No subagents running", Style::default().fg(Color::DarkGray))));
        frame.render_widget(empty, area);
        return;
    }

    let n = panel.entries.len();
    let available = area.height;
    let min_per = 2u16;
    let constraints: Vec<Constraint> = if n as u16 * min_per <= available {
        let base = available / n as u16;
        let remainder = available % n as u16;
        (0..n).map(|i| Constraint::Length(if i == 0 { base + remainder } else { base })).collect()
    } else {
        (0..n).map(|_| Constraint::Length(min_per)).collect()
    };

    let chunks = Layout::default().direction(Direction::Vertical).constraints(constraints).split(area);

    for (i, entry) in panel.entries.iter().enumerate() {
        let chunk = chunks[i];
        if chunk.height == 0 {
            continue;
        }
        let is_selected = focused && i == panel.selected;
        render_list_entry(frame, entry, chunk, is_selected, theme);
    }
}

/// Detail view — immutable (computes scroll without mutation)
fn render_detail_view_immut(frame: &mut Frame, panel: &SubagentPanel, _theme: &Theme, area: Rect, _focused: bool) {
    let entry = match panel.entries.get(panel.selected) {
        Some(e) => e,
        None => return,
    };

    let lines: Vec<Line> = if entry.output_lines.is_empty() {
        vec![Line::from(Span::styled(
            format!("Task: {}\n\nWaiting for output...", entry.task),
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        entry.output_lines.iter().map(|l| Line::from(l.as_str())).collect()
    };

    let total = lines.len() as u16;
    let max_scroll = total.saturating_sub(area.height);
    let scroll = panel.scroll_offset.get().min(max_scroll);
    panel.scroll_offset.set(scroll);

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((scroll, 0));
    frame.render_widget(paragraph, area);
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_panel_starts_invisible() {
        let panel = SubagentPanel::new();
        assert!(!panel.is_visible());
        assert_eq!(panel.running_count(), 0);
        assert_eq!(panel.view, PanelView::List);
    }

    #[test]
    fn test_add_makes_visible() {
        let mut panel = SubagentPanel::new();
        panel.add("1".into(), "sub1".into(), "do stuff".into(), None);
        assert!(panel.is_visible());
        assert_eq!(panel.running_count(), 1);
        assert_eq!(panel.selected, 0);
    }

    #[test]
    fn test_append_output() {
        let mut panel = SubagentPanel::new();
        panel.add("1".into(), "sub1".into(), "task".into(), None);
        panel.append_output("1", "line 1");
        panel.append_output("1", "line 2");
        assert_eq!(panel.entries[0].output_lines.len(), 2);
    }

    #[test]
    fn test_mark_done() {
        let mut panel = SubagentPanel::new();
        panel.add("1".into(), "sub1".into(), "task".into(), None);
        panel.mark_done("1");
        assert_eq!(panel.entries[0].status, SubagentStatus::Done);
        assert_eq!(panel.running_count(), 0);
    }

    #[test]
    fn test_clear_done() {
        let mut panel = SubagentPanel::new();
        panel.add("1".into(), "sub1".into(), "task".into(), None);
        panel.add("2".into(), "sub2".into(), "task".into(), None);
        panel.mark_done("1");
        panel.clear_done();
        assert_eq!(panel.entries.len(), 1);
        assert_eq!(panel.entries[0].id, "2");
    }

    #[test]
    fn test_tab_navigation() {
        let mut panel = SubagentPanel::new();
        panel.add("1".into(), "sub1".into(), "t".into(), None);
        panel.add("2".into(), "sub2".into(), "t".into(), None);
        panel.add("3".into(), "sub3".into(), "t".into(), None);
        assert_eq!(panel.selected, 2);
        panel.next_tab();
        assert_eq!(panel.selected, 0);
        panel.prev_tab();
        assert_eq!(panel.selected, 2);
    }

    #[test]
    fn test_detail_view_toggle() {
        let mut panel = SubagentPanel::new();
        panel.add("1".into(), "sub1".into(), "task".into(), None);
        assert_eq!(panel.view, PanelView::List);
        panel.open_detail();
        assert_eq!(panel.view, PanelView::Detail);
        panel.close_detail();
        assert_eq!(panel.view, PanelView::List);
    }

    #[test]
    fn test_append_output_unknown_id_noop() {
        let mut panel = SubagentPanel::new();
        panel.add("1".into(), "sub1".into(), "task".into(), None);
        panel.append_output("unknown", "line");
        assert!(panel.entries[0].output_lines.is_empty());
    }
}
