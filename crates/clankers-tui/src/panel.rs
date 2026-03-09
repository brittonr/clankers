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

use crate::theme::Theme;

// Panel identifier and actions re-exported from clankers-tui-types.
pub use clankers_tui_types::PanelAction;
pub use clankers_tui_types::PanelId;

// ── PanelManager ────────────────────────────────────────────────────────────

/// Manages the collection of panels, keyed by `PanelId`.
/// Replaces the old pattern of having a named field per panel on `App`.
pub struct PanelManager {
    panels: IndexMap<PanelId, Box<dyn Panel>>,
}

impl PanelManager {
    pub fn new() -> Self {
        Self {
            panels: IndexMap::new(),
        }
    }

    /// Register a panel (inserts by its `id()`).
    pub fn register(&mut self, panel: Box<dyn Panel>) {
        let id = panel.id();
        self.panels.insert(id, panel);
    }

    /// Get a panel by ID (immutable).
    pub fn get(&self, id: PanelId) -> Option<&dyn Panel> {
        self.panels.get(&id).map(|p| p.as_ref())
    }

    /// Get a panel by ID (mutable).
    pub fn get_mut(&mut self, id: PanelId) -> Option<&mut dyn Panel> {
        Some(self.panels.get_mut(&id)?.as_mut())
    }

    /// Iterate over all panels.
    pub fn iter(&self) -> impl Iterator<Item = (PanelId, &dyn Panel)> {
        self.panels.iter().map(|(id, p)| (*id, p.as_ref()))
    }

    /// All registered panel IDs.
    pub fn ids(&self) -> Vec<PanelId> {
        self.panels.keys().copied().collect()
    }

    /// Downcast a panel to a concrete type (immutable).
    pub fn downcast_ref<T: Panel + 'static>(&self, id: PanelId) -> Option<&T> {
        self.panels.get(&id)?.as_any().downcast_ref()
    }

    /// Downcast a panel to a concrete type (mutable).
    pub fn downcast_mut<T: Panel + 'static>(&mut self, id: PanelId) -> Option<&mut T> {
        self.panels.get_mut(&id)?.as_any_mut().downcast_mut()
    }
}

impl Default for PanelManager {
    fn default() -> Self {
        Self::new()
    }
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

    /// Downcast to `&dyn Any` for type-safe downcasting via `PanelManager`.
    fn as_any(&self) -> &dyn std::any::Any;

    /// Downcast to `&mut dyn Any` for type-safe downcasting via `PanelManager`.
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

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

    /// Close any detail/diff views this panel might have open.
    /// Default: no-op. Override for panels like Subagents (detail view) or Files (diff view).
    fn close_detail_view(&mut self) {}

    /// Return a reference to the panel's scroll state (if it uses `PanelScroll`).
    /// Implementing this enables the default `handle_scroll` for mouse wheel.
    fn panel_scroll(&self) -> Option<&PanelScroll> {
        None
    }

    /// Mutable reference to the panel's scroll state.
    fn panel_scroll_mut(&mut self) -> Option<&mut PanelScroll> {
        None
    }

    /// Handle a mouse scroll event.
    ///
    /// Default implementation delegates to `panel_scroll_mut()` if available.
    /// Panels using `ListNav` should override this to call `nav.next()`/`nav.prev()`.
    fn handle_scroll(&mut self, up: bool, lines: u16) {
        if let Some(scroll) = self.panel_scroll_mut() {
            if up {
                scroll.scroll_up(lines as usize);
            } else {
                scroll.scroll_down(lines as usize);
            }
        }
    }

    /// Return the panel's content as lines. If implemented, the default
    /// `draw()` implementation handles scrolling automatically via `panel_scroll()`.
    ///
    /// Panels that need custom rendering (split panes, stateful widgets)
    /// should override `draw()` instead.
    fn content(&self, _width: usize, _ctx: &DrawContext) -> Option<Vec<Line<'static>>> {
        None
    }

    /// Render the panel's content into `area` (inside the border).
    /// The caller has already drawn the outer Block with title/border.
    ///
    /// Default implementation calls `content()` and renders with auto-scroll.
    /// Override this for panels that need custom rendering.
    fn draw(&self, frame: &mut Frame, area: Rect, ctx: &DrawContext) {
        if let Some(lines) = self.content(area.width as usize, ctx) {
            let scroll_offset = self.panel_scroll().map(|s| s.offset_u16()).unwrap_or(0);
            let para = Paragraph::new(lines).scroll((scroll_offset, 0)).wrap(Wrap { trim: false });
            frame.render_widget(para, area);
        }
    }
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

/// Mutable variant: draw frame + auto-scroll content.
///
/// Calls `content()` to get lines, updates `panel_scroll_mut()` dimensions,
/// then renders with scroll offset. Falls back to immutable `draw()` if the
/// panel doesn't implement `content()`.
pub fn draw_panel_scrolled(frame: &mut Frame, panel: &mut dyn Panel, area: Rect, ctx: &DrawContext) {
    if let Some(inner) = draw_panel_frame(frame, panel, area, ctx) {
        let width = inner.width as usize;
        let visible = inner.height as usize;

        if let Some(lines) = panel.content(width, ctx) {
            let total = lines.len();
            // Update scroll dimensions
            if let Some(scroll) = panel.panel_scroll_mut() {
                scroll.set_dimensions(total, visible);
            }
            let offset = panel.panel_scroll().map(|s| s.offset_u16()).unwrap_or(0);
            let para = Paragraph::new(lines).scroll((offset, 0)).wrap(Wrap { trim: false });
            frame.render_widget(para, inner);
        } else {
            panel.draw(frame, inner, ctx);
        }
    }
}

// ── PanelScroll — generic scroll state for any panel ────────────────────────

/// Tracks scroll offset for panel content that overflows the visible area.
///
/// Embed this in your panel struct, implement `panel_scroll()` /
/// `panel_scroll_mut()` on the Panel trait, and you get:
/// - Default `handle_scroll` for mouse wheel
/// - `scroll_to_fit()` in draw to auto-clamp
/// - `offset_u16()` for `Paragraph::scroll()`
///
/// Panels using `ListNav` for item selection should keep using that instead;
/// `PanelScroll` is for free-form text or when you want raw pixel scrolling.
#[derive(Debug, Clone, Default)]
pub struct PanelScroll {
    /// Current scroll offset (in lines from top).
    pub offset: usize,
    /// Total content height (set each frame in draw).
    pub content_height: usize,
    /// Visible height (set each frame in draw).
    pub visible_height: usize,
}

impl PanelScroll {
    pub fn new() -> Self {
        Self::default()
    }

    /// Scroll up by `n` lines (clamped to 0).
    pub fn scroll_up(&mut self, n: usize) {
        self.offset = self.offset.saturating_sub(n);
    }

    /// Scroll down by `n` lines (clamped to max).
    pub fn scroll_down(&mut self, n: usize) {
        let max = self.content_height.saturating_sub(self.visible_height);
        self.offset = (self.offset + n).min(max);
    }

    /// Update content/visible dimensions each frame. Clamps offset if content shrank.
    pub fn set_dimensions(&mut self, content_height: usize, visible_height: usize) {
        self.content_height = content_height;
        self.visible_height = visible_height;
        let max = content_height.saturating_sub(visible_height);
        if self.offset > max {
            self.offset = max;
        }
    }

    /// Convenience for `Paragraph::scroll((offset, 0))`.
    pub fn offset_u16(&self) -> u16 {
        self.offset as u16
    }

    /// Whether there is content above the visible area.
    pub fn can_scroll_up(&self) -> bool {
        self.offset > 0
    }

    /// Whether there is content below the visible area.
    pub fn can_scroll_down(&self) -> bool {
        self.offset + self.visible_height < self.content_height
    }

    /// Scroll so that line `target` is visible, preferring to center it.
    pub fn scroll_to_line(&mut self, target: usize) {
        if target < self.offset {
            self.offset = target;
        } else if target >= self.offset + self.visible_height {
            self.offset = target.saturating_sub(self.visible_height / 2);
        }
        let max = self.content_height.saturating_sub(self.visible_height);
        self.offset = self.offset.min(max);
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

    // ── PanelScroll tests ───────────────────────────────────────────

    #[test]
    fn test_panel_scroll_new() {
        let s = PanelScroll::new();
        assert_eq!(s.offset, 0);
        assert_eq!(s.content_height, 0);
        assert_eq!(s.visible_height, 0);
        assert!(!s.can_scroll_up());
        assert!(!s.can_scroll_down());
    }

    #[test]
    fn test_panel_scroll_set_dimensions() {
        let mut s = PanelScroll::new();
        s.set_dimensions(100, 20);
        assert_eq!(s.content_height, 100);
        assert_eq!(s.visible_height, 20);
        assert!(!s.can_scroll_up());
        assert!(s.can_scroll_down());
    }

    #[test]
    fn test_panel_scroll_down_clamps() {
        let mut s = PanelScroll::new();
        s.set_dimensions(30, 20);
        // Max offset = 30 - 20 = 10
        s.scroll_down(5);
        assert_eq!(s.offset, 5);
        s.scroll_down(100);
        assert_eq!(s.offset, 10); // clamped
        assert!(s.can_scroll_up());
        assert!(!s.can_scroll_down());
    }

    #[test]
    fn test_panel_scroll_up_clamps() {
        let mut s = PanelScroll::new();
        s.set_dimensions(30, 20);
        s.scroll_down(5);
        s.scroll_up(3);
        assert_eq!(s.offset, 2);
        s.scroll_up(100);
        assert_eq!(s.offset, 0); // clamped
    }

    #[test]
    fn test_panel_scroll_dimensions_shrink_clamps() {
        let mut s = PanelScroll::new();
        s.set_dimensions(100, 20);
        s.scroll_down(70);
        assert_eq!(s.offset, 70);
        // Content shrinks: max offset is now 10
        s.set_dimensions(30, 20);
        assert_eq!(s.offset, 10); // clamped
    }

    #[test]
    fn test_panel_scroll_content_fits() {
        let mut s = PanelScroll::new();
        s.set_dimensions(10, 20); // content smaller than visible
        s.scroll_down(5);
        assert_eq!(s.offset, 0); // can't scroll
        assert!(!s.can_scroll_up());
        assert!(!s.can_scroll_down());
    }

    #[test]
    fn test_panel_scroll_to_line() {
        let mut s = PanelScroll::new();
        s.set_dimensions(100, 20);

        // Target below visible area
        s.scroll_to_line(50);
        assert!(s.offset > 0);
        assert!(s.offset <= 50);

        // Target above visible area
        s.offset = 50;
        s.scroll_to_line(10);
        assert_eq!(s.offset, 10);

        // Target already visible: no change
        s.offset = 10;
        s.scroll_to_line(15);
        assert_eq!(s.offset, 10);
    }

    #[test]
    fn test_panel_scroll_offset_u16() {
        let mut s = PanelScroll::new();
        s.set_dimensions(100, 20);
        s.scroll_down(42);
        assert_eq!(s.offset_u16(), 42);
    }
}
