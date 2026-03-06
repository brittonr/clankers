//! Panel trait, shared helpers, and panel registry.
//!
//! Every side-panel in the TUI implements [`Panel`] so that layout, focus,
//! key routing, and rendering all go through a single code-path.
//!
//! Follows ratatui's Component architecture pattern:
//! - `init()` for lifecycle setup
//! - `handle_key_event()` → `Option<PanelAction>` for input
//! - `update()` for action-driven state changes
//! - `draw()` for rendering (consistent with ratatui naming)

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

use crate::tui::theme::Theme;

// ── Panel identifier ────────────────────────────────────────────────────────

/// Unique identifier for a panel. Used by the layout engine and focus tracker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PanelId {
    Todo,
    Files,
    Subagents,
    Peers,
    Processes,
    Branches,
}

impl PanelId {
    pub fn label(self) -> &'static str {
        match self {
            PanelId::Todo => "Todo",
            PanelId::Files => "Files",
            PanelId::Subagents => "Subagents",
            PanelId::Peers => "Peers",
            PanelId::Processes => "Processes",
            PanelId::Branches => "Branches",
        }
    }

    /// All known panel IDs (for iteration / config validation).
    pub const ALL: &'static [PanelId] = &[
        PanelId::Todo,
        PanelId::Files,
        PanelId::Subagents,
        PanelId::Peers,
        PanelId::Processes,
        PanelId::Branches,
    ];
}

impl std::fmt::Display for PanelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

// ── Panel actions ───────────────────────────────────────────────────────────

/// Actions that a panel can emit back to the application.
/// Follows the ratatui Component pattern: event handlers return
/// `Option<PanelAction>` rather than mutating app state directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanelAction {
    /// The key was consumed internally — no further handling needed.
    Consumed,
    /// The panel wants to give up focus (e.g. Esc pressed).
    Unfocus,
    /// The panel wants to run a slash command.
    SlashCommand(String),
    /// The panel wants to switch focus to a different panel.
    FocusPanel(PanelId),
    /// Switch to a conversation branch by block ID.
    SwitchBranch(usize),
}

// ── The Panel trait ─────────────────────────────────────────────────────────

/// Common interface for all side-panels.
///
/// Modeled after ratatui's `Component` trait:
/// - `init` for setup
/// - `handle_key_event` for input → optional action
/// - `update` for action-driven state changes (from app-level events)
/// - `draw` for rendering
pub trait Panel {
    /// Unique identifier.
    fn id(&self) -> PanelId;

    /// Lifecycle: called once after construction.
    fn init(&mut self) {}

    /// Dynamic title (may include counts, e.g. "Todo (3/5)").
    fn title(&self) -> String;

    /// Hint text shown in the title when the panel is focused.
    fn focus_hints(&self) -> &'static str {
        " j/k "
    }

    /// Whether the panel currently has no content.
    fn is_empty(&self) -> bool;

    /// Text shown when the panel is empty.
    fn empty_text(&self) -> &'static str {
        "Nothing to show."
    }

    /// Handle a key event while this panel is focused.
    /// Returns `None` if the key was not handled (bubble up),
    /// or `Some(action)` for the app to process.
    fn handle_key_event(&mut self, key: KeyEvent) -> Option<PanelAction>;

    /// Handle a mouse scroll event. Default scrolls `ListNav`-based panels.
    fn handle_scroll(&mut self, _up: bool, _lines: u16) {}

    /// Render the panel's content into `area` (inside the border).
    /// The caller has already drawn the outer Block with title/border.
    fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext);
}

// ── Draw context ────────────────────────────────────────────────────────────

/// Everything a panel needs to render itself, bundled for a clean signature.
pub struct DrawContext<'a> {
    pub theme: &'a Theme,
    pub focused: bool,
}

// ── PanelFrame — shared border / title / empty rendering ────────────────────

/// Draw the standard panel frame (border + title + focus hints) and return
/// the inner Rect. If the panel is empty, renders the empty-state text
/// and returns `None` (caller should skip `draw`).
pub fn draw_panel_frame(frame: &mut Frame, panel: &dyn Panel, area: Rect, ctx: &DrawContext) -> Option<Rect> {
    let border_color = if ctx.focused { Color::Cyan } else { ctx.theme.border };

    let hints = if ctx.focused { panel.focus_hints() } else { "" };
    let title_text = format!(" {}{}", panel.title(), hints);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(title_text, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));

    if panel.is_empty() {
        let empty = Paragraph::new(Line::from(Span::styled(panel.empty_text(), Style::default().fg(Color::DarkGray))))
            .block(block)
            .wrap(Wrap { trim: false });
        frame.render_widget(empty, area);
        return None;
    }

    let inner = block.inner(area);
    frame.render_widget(block, area);
    Some(inner)
}

/// Convenience: draw frame + content in one call.
pub fn draw_panel(frame: &mut Frame, panel: &dyn Panel, area: Rect, ctx: &DrawContext) {
    if let Some(inner) = draw_panel_frame(frame, panel, area, ctx) {
        panel.draw(frame, inner, ctx);
    }
}

// ── ListNav — reusable wrapping-selection helper ────────────────────────────

/// Manages a selected index in a list with wrapping navigation and
/// scroll-to-visible calculation.
#[derive(Debug, Clone, Default)]
pub struct ListNav {
    /// Currently selected index.
    pub selected: usize,
}

impl ListNav {
    pub fn new() -> Self {
        Self { selected: 0 }
    }

    /// Move selection down (wraps around).
    pub fn next(&mut self, len: usize) {
        if len > 0 {
            self.selected = (self.selected + 1) % len;
        }
    }

    /// Move selection up (wraps around).
    pub fn prev(&mut self, len: usize) {
        if len > 0 {
            self.selected = if self.selected == 0 { len - 1 } else { self.selected - 1 };
        }
    }

    /// Clamp selection to valid range after items were removed.
    pub fn clamp(&mut self, len: usize) {
        if len == 0 {
            self.selected = 0;
        } else if self.selected >= len {
            self.selected = len - 1;
        }
    }

    /// Calculate vertical scroll offset so the selected item is visible.
    /// `items_per_entry` accounts for multi-line entries (e.g. 2 for header+content).
    pub fn scroll_offset(&self, visible_height: usize, items_per_entry: usize) -> u16 {
        let visual_pos = self.selected * items_per_entry;
        if visual_pos >= visible_height {
            (visual_pos - visible_height + items_per_entry) as u16
        } else {
            0
        }
    }

    /// Render a selection prefix: "▸ " if selected, "  " otherwise.
    pub fn prefix_span(&self, index: usize, focused: bool) -> Span<'static> {
        if index == self.selected && focused {
            Span::styled("▸ ", Style::default().fg(Color::Cyan))
        } else {
            Span::styled("  ", Style::default().fg(Color::DarkGray))
        }
    }

    /// Style for the text of a selected vs non-selected item.
    pub fn item_style(&self, index: usize, focused: bool, base: Style) -> Style {
        if index == self.selected && focused {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            base
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_nav_wraps() {
        let mut nav = ListNav::new();
        nav.next(3);
        assert_eq!(nav.selected, 1);
        nav.next(3);
        assert_eq!(nav.selected, 2);
        nav.next(3);
        assert_eq!(nav.selected, 0); // wrapped

        nav.prev(3);
        assert_eq!(nav.selected, 2); // wrapped back
        nav.prev(3);
        assert_eq!(nav.selected, 1);
    }

    #[test]
    fn test_list_nav_clamp() {
        let mut nav = ListNav::new();
        nav.selected = 5;
        nav.clamp(3);
        assert_eq!(nav.selected, 2);

        nav.clamp(0);
        assert_eq!(nav.selected, 0);
    }

    #[test]
    fn test_list_nav_empty() {
        let mut nav = ListNav::new();
        nav.next(0); // no-op on empty
        assert_eq!(nav.selected, 0);
        nav.prev(0);
        assert_eq!(nav.selected, 0);
    }

    #[test]
    fn test_scroll_offset() {
        let nav = ListNav { selected: 0 };
        assert_eq!(nav.scroll_offset(10, 1), 0);

        let nav = ListNav { selected: 15 };
        assert_eq!(nav.scroll_offset(10, 1), 6); // 15 - 10 + 1 = 6

        // Multi-line entries (2 lines each)
        let nav = ListNav { selected: 8 };
        assert_eq!(nav.scroll_offset(10, 2), 8); // 8*2=16, 16-10+2=8
    }

    #[test]
    fn test_panel_id_roundtrip() {
        for &id in PanelId::ALL {
            assert!(!id.label().is_empty());
            assert_eq!(format!("{}", id), id.label());
        }
    }
}
