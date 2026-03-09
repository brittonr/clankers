//! Per-subagent pane state — each subagent gets its own BSP pane.
//!
//! Unlike the old approach of cramming all subagent output into one panel,
//! each subagent now gets a first-class pane in the tiling layout with
//! independent scrolling, focus, and lifecycle.

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use indexmap::IndexMap;
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
use ratatui_hypertile::PaneId;

use crate::components::subagent_panel::SubagentStatus;
use crate::panel::DrawContext;
use crate::panel::PanelAction;
use crate::panel::PanelScroll;

// ── Per-subagent pane state ─────────────────────────────────────────────────

/// State for a single subagent's dedicated BSP pane.
#[derive(Debug)]
pub struct SubagentPaneState {
    /// Subagent ID (matches SubagentEvent ids)
    pub id: String,
    /// Short display name
    pub name: String,
    /// Task description
    pub task: String,
    /// Current status
    pub status: SubagentStatus,
    /// All output lines
    pub output_lines: Vec<String>,
    /// Process ID (for kill support)
    pub pid: Option<u32>,
    /// Scroll state
    pub scroll: PanelScroll,
    /// The hypertile pane ID assigned to this subagent
    pub pane_id: PaneId,
    /// Whether to auto-scroll to bottom on new output
    pub auto_scroll: bool,
}

impl SubagentPaneState {
    pub fn new(id: String, name: String, task: String, pid: Option<u32>, pane_id: PaneId) -> Self {
        Self {
            id,
            name,
            task,
            status: SubagentStatus::Running,
            output_lines: Vec::new(),
            pid,
            scroll: PanelScroll::new(),
            pane_id,
            auto_scroll: true,
        }
    }
}

// ── Manager for all subagent panes ──────────────────────────────────────────

/// Owns all per-subagent pane states, keyed by subagent ID.
#[derive(Debug, Default)]
pub struct SubagentPaneManager {
    panes: IndexMap<String, SubagentPaneState>,
}

impl SubagentPaneManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new subagent pane, allocating a PaneId from hypertile.
    /// Returns the allocated PaneId (caller must register it in PaneRegistry
    /// and insert it into the BSP tree).
    pub fn create(
        &mut self,
        id: String,
        name: String,
        task: String,
        pid: Option<u32>,
        tiling: &mut ratatui_hypertile::Hypertile,
    ) -> PaneId {
        let pane_id = tiling.state_mut().allocate_pane_id();
        let state = SubagentPaneState::new(id.clone(), name, task, pid, pane_id);
        self.panes.insert(id, state);
        pane_id
    }

    /// Remove a subagent pane. Returns the PaneId if it existed
    /// (caller must unregister from PaneRegistry and remove from BSP tree).
    pub fn remove(&mut self, id: &str) -> Option<PaneId> {
        self.panes.shift_remove(id).map(|s| s.pane_id)
    }

    /// Get a pane state by subagent ID.
    pub fn get(&self, id: &str) -> Option<&SubagentPaneState> {
        self.panes.get(id)
    }

    /// Get a mutable pane state by subagent ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut SubagentPaneState> {
        self.panes.get_mut(id)
    }

    /// Append an output line to a subagent's log.
    pub fn append_output(&mut self, id: &str, line: &str) {
        if let Some(state) = self.panes.get_mut(id) {
            state.output_lines.push(line.to_string());
            if state.auto_scroll {
                // Push scroll to bottom on new content
                let total = state.output_lines.len();
                let visible = state.scroll.visible_height;
                state.scroll.offset = total.saturating_sub(visible);
            }
        }
    }

    /// Mark a subagent as done.
    pub fn mark_done(&mut self, id: &str) {
        if let Some(state) = self.panes.get_mut(id) {
            state.status = SubagentStatus::Done;
        }
    }

    /// Mark a subagent as errored.
    pub fn mark_error(&mut self, id: &str) {
        if let Some(state) = self.panes.get_mut(id) {
            state.status = SubagentStatus::Error;
        }
    }

    /// Look up a subagent ID by its pane ID.
    pub fn id_for_pane(&self, pane_id: PaneId) -> Option<&str> {
        self.panes.values().find(|s| s.pane_id == pane_id).map(|s| s.id.as_str())
    }

    /// Handle a key event for a focused subagent pane.
    pub fn handle_key_event(&mut self, id: &str, key: KeyEvent) -> Option<PanelAction> {
        let state = self.panes.get_mut(id)?;
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                state.auto_scroll = false;
                state.scroll.scroll_down(1);
                Some(PanelAction::Consumed)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                state.auto_scroll = false;
                state.scroll.scroll_up(1);
                Some(PanelAction::Consumed)
            }
            KeyCode::Char('g') => {
                state.auto_scroll = false;
                state.scroll.offset = 0;
                Some(PanelAction::Consumed)
            }
            KeyCode::Char('G') => {
                state.auto_scroll = true;
                let total = state.output_lines.len();
                let visible = state.scroll.visible_height;
                state.scroll.offset = total.saturating_sub(visible);
                Some(PanelAction::Consumed)
            }
            KeyCode::PageDown => {
                state.auto_scroll = false;
                let page = state.scroll.visible_height.max(1);
                state.scroll.scroll_down(page);
                Some(PanelAction::Consumed)
            }
            KeyCode::PageUp => {
                state.auto_scroll = false;
                let page = state.scroll.visible_height.max(1);
                state.scroll.scroll_up(page);
                Some(PanelAction::Consumed)
            }
            KeyCode::Esc => Some(PanelAction::Unfocus),
            // x = kill, q/X = dismiss — handled by caller (needs channel access)
            _ => None,
        }
    }

    /// Handle mouse scroll for a subagent pane.
    pub fn handle_scroll(&mut self, id: &str, up: bool, lines: u16) {
        if let Some(state) = self.panes.get_mut(id) {
            state.auto_scroll = false;
            if up {
                state.scroll.scroll_up(lines as usize);
            } else {
                state.scroll.scroll_down(lines as usize);
            }
        }
    }

    /// Render a single subagent pane.
    pub fn draw(&mut self, id: &str, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
        let Some(state) = self.panes.get_mut(id) else {
            return;
        };

        if area.width < 4 || area.height < 3 {
            return;
        }

        let (icon, color) = status_icon_color(&state.status);
        let border_color = if ctx.focused { Color::Cyan } else { ctx.theme.border };

        let focus_hint = if ctx.focused {
            match state.status {
                SubagentStatus::Running => " j/k:scroll x:kill q:close ",
                _ => " j/k:scroll q:close ",
            }
        } else {
            ""
        };

        let title =
            format!(" {} {} — {}{}", icon, state.name, state.task.chars().take(30).collect::<String>(), focus_hint,);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(Span::styled(title, Style::default().fg(color).add_modifier(Modifier::BOLD)));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.height == 0 || inner.width == 0 {
            return;
        }

        // Build content lines
        let lines: Vec<Line> = if state.output_lines.is_empty() {
            let msg = if state.status == SubagentStatus::Running {
                "Waiting for output..."
            } else {
                "(no output)"
            };
            vec![Line::from(Span::styled(msg, Style::default().fg(Color::DarkGray)))]
        } else {
            state.output_lines.iter().map(|l| Line::from(l.as_str())).collect()
        };

        // Update scroll dimensions
        let total = lines.len();
        let visible = inner.height as usize;
        state.scroll.set_dimensions(total, visible);

        // Auto-scroll: snap to bottom if enabled
        if state.auto_scroll {
            state.scroll.offset = total.saturating_sub(visible);
        }

        let offset = state.scroll.offset_u16();
        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false }).scroll((offset, 0));
        frame.render_widget(paragraph, inner);

        // Scroll indicator
        if state.scroll.can_scroll_up() || state.scroll.can_scroll_down() {
            let indicator = format!(" {}/{} ", state.scroll.offset + 1, total.saturating_sub(visible).max(1));
            let ind_len = indicator.len() as u16;
            if area.width > ind_len + 2 {
                let ind_area = Rect {
                    x: area.x + area.width.saturating_sub(ind_len + 1),
                    y: area.y + area.height.saturating_sub(1),
                    width: ind_len,
                    height: 1,
                };
                frame.render_widget(
                    Paragraph::new(Span::styled(indicator, Style::default().fg(Color::DarkGray))),
                    ind_area,
                );
            }
        }
    }

    /// Whether any subagent panes exist.
    pub fn is_empty(&self) -> bool {
        self.panes.is_empty()
    }

    /// Number of subagent panes.
    pub fn len(&self) -> usize {
        self.panes.len()
    }

    /// All subagent IDs.
    pub fn ids(&self) -> Vec<String> {
        self.panes.keys().cloned().collect()
    }

    /// Iterate all pane states.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &SubagentPaneState)> {
        self.panes.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// Find the PaneId for a subagent by its string ID.
    pub fn pane_id_for(&self, id: &str) -> Option<PaneId> {
        self.panes.get(id).map(|s| s.pane_id)
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn status_icon_color(status: &SubagentStatus) -> (&'static str, Color) {
    match status {
        SubagentStatus::Running => ("⏳", Color::Yellow),
        SubagentStatus::Done => ("✓", Color::Green),
        SubagentStatus::Error => ("✗", Color::Red),
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tiling() -> ratatui_hypertile::Hypertile {
        ratatui_hypertile::Hypertile::new()
    }

    #[test]
    fn test_create_and_get() {
        let mut mgr = SubagentPaneManager::new();
        let mut tiling = make_tiling();
        let pane_id = mgr.create("sub1".into(), "worker".into(), "do stuff".into(), Some(1234), &mut tiling);
        assert!(pane_id.get() > 0); // not ROOT
        assert!(mgr.get("sub1").is_some());
        assert_eq!(mgr.get("sub1").unwrap().name, "worker");
        assert_eq!(mgr.len(), 1);
    }

    #[test]
    fn test_remove() {
        let mut mgr = SubagentPaneManager::new();
        let mut tiling = make_tiling();
        let pane_id = mgr.create("sub1".into(), "w1".into(), "t".into(), None, &mut tiling);
        assert_eq!(mgr.remove("sub1"), Some(pane_id));
        assert!(mgr.is_empty());
        assert_eq!(mgr.remove("sub1"), None);
    }

    #[test]
    fn test_append_output() {
        let mut mgr = SubagentPaneManager::new();
        let mut tiling = make_tiling();
        mgr.create("sub1".into(), "w1".into(), "t".into(), None, &mut tiling);
        mgr.append_output("sub1", "line 1");
        mgr.append_output("sub1", "line 2");
        assert_eq!(mgr.get("sub1").unwrap().output_lines.len(), 2);
        // Unknown ID is a no-op
        mgr.append_output("unknown", "nope");
    }

    #[test]
    fn test_mark_done_error() {
        let mut mgr = SubagentPaneManager::new();
        let mut tiling = make_tiling();
        mgr.create("sub1".into(), "w1".into(), "t".into(), None, &mut tiling);
        mgr.create("sub2".into(), "w2".into(), "t".into(), None, &mut tiling);
        mgr.mark_done("sub1");
        mgr.mark_error("sub2");
        assert_eq!(mgr.get("sub1").unwrap().status, SubagentStatus::Done);
        assert_eq!(mgr.get("sub2").unwrap().status, SubagentStatus::Error);
    }

    #[test]
    fn test_id_for_pane() {
        let mut mgr = SubagentPaneManager::new();
        let mut tiling = make_tiling();
        let pane_id = mgr.create("sub1".into(), "w1".into(), "t".into(), None, &mut tiling);
        assert_eq!(mgr.id_for_pane(pane_id), Some("sub1"));
        assert_eq!(mgr.id_for_pane(PaneId::ROOT), None);
    }
}
