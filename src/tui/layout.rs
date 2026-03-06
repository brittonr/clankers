//! Declarative panel layout engine.
//!
//! The layout describes how side-panels are arranged in columns alongside
//! the main chat area. It is configurable at runtime and serializable to
//! TOML so users can persist their preferred arrangement.

use ratatui::layout::Constraint;
use ratatui::layout::Direction;
use ratatui::layout::Layout;
use ratatui::layout::Rect;

use crate::tui::panel::PanelId;

// ── Layout primitives ───────────────────────────────────────────────────────

/// A slot in a column that holds one panel.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PanelSlot {
    pub id: PanelId,
    /// Height share within the column. Slots with equal weight get equal
    /// space. Use 0 to hide the panel (it still exists, just hidden).
    #[serde(default = "default_weight")]
    pub weight: u16,
}

fn default_weight() -> u16 {
    1
}

impl PanelSlot {
    pub fn new(id: PanelId) -> Self {
        Self { id, weight: 1 }
    }

    pub fn with_weight(id: PanelId, weight: u16) -> Self {
        Self { id, weight }
    }
}

/// A column in the layout. Contains one or more panel slots stacked
/// vertically.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Column {
    /// Width of this column as a percentage of total terminal width.
    /// The main chat column's width is whatever remains after all
    /// side columns are subtracted.
    pub width_pct: u16,
    /// Panel slots stacked top-to-bottom.
    pub slots: Vec<PanelSlot>,
}

impl Column {
    pub fn new(width_pct: u16, slots: Vec<PanelSlot>) -> Self {
        Self { width_pct, slots }
    }

    /// Only visible (weight > 0) slots.
    pub fn visible_slots(&self) -> Vec<&PanelSlot> {
        self.slots.iter().filter(|s| s.weight > 0).collect()
    }
}

/// Which side of the main chat area a column sits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ColumnSide {
    Left,
    Right,
}

/// A side column with its position.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SideColumn {
    pub side: ColumnSide,
    pub column: Column,
}

// ── PanelLayout ─────────────────────────────────────────────────────────────

/// Full layout description: side columns around a central chat area.
///
/// The main column (chat + editor + status) always exists and gets the
/// remaining width after all side columns are accounted for.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PanelLayout {
    /// Side columns (left and/or right of the main area).
    pub columns: Vec<SideColumn>,
}

impl Default for PanelLayout {
    /// The default layout: left column (20%) with Todo + Files,
    /// right column (30%) with Subagents + Peers — matching the
    /// original hardcoded layout.
    fn default() -> Self {
        Self {
            columns: vec![
                SideColumn {
                    side: ColumnSide::Left,
                    column: Column::new(20, vec![PanelSlot::new(PanelId::Todo), PanelSlot::new(PanelId::Files)]),
                },
                SideColumn {
                    side: ColumnSide::Right,
                    column: Column::new(
                        30,
                        vec![
                            PanelSlot::new(PanelId::Subagents),
                            PanelSlot::new(PanelId::Peers),
                            PanelSlot::with_weight(PanelId::Branches, 0),
                        ],
                    ),
                },
            ],
        }
    }
}

impl PanelLayout {
    /// Create a minimal layout with no side panels.
    pub fn chat_only() -> Self {
        Self { columns: vec![] }
    }

    /// Split the terminal area into regions according to this layout.
    /// Returns `(left_regions, main_region, right_regions)` where each
    /// side region is `(PanelId, Rect)`.
    pub fn split(&self, area: Rect) -> LayoutRegions {
        let left_cols: Vec<&SideColumn> = self.columns.iter().filter(|c| c.side == ColumnSide::Left).collect();
        let right_cols: Vec<&SideColumn> = self.columns.iter().filter(|c| c.side == ColumnSide::Right).collect();

        // Build horizontal constraints: [left cols...] [main] [right cols...]
        let mut h_constraints = Vec::new();
        for col in &left_cols {
            h_constraints.push(Constraint::Percentage(col.column.width_pct));
        }
        // Main gets whatever is left
        let side_total: u16 = self.columns.iter().map(|c| c.column.width_pct).sum();
        let main_pct = 100u16.saturating_sub(side_total).max(20);
        h_constraints.push(Constraint::Percentage(main_pct));
        for col in &right_cols {
            h_constraints.push(Constraint::Percentage(col.column.width_pct));
        }

        let h_areas = Layout::default().direction(Direction::Horizontal).constraints(h_constraints).split(area);

        let main_idx = left_cols.len();
        let main_area = h_areas[main_idx];

        // Split each side column vertically into panel slots
        let mut panels = Vec::new();

        for (i, col) in left_cols.iter().enumerate() {
            let col_area = h_areas[i];
            let slot_rects = split_column_vertical(&col.column, col_area);
            for (slot, rect) in slot_rects {
                panels.push((slot.id, rect));
            }
        }

        for (i, col) in right_cols.iter().enumerate() {
            let col_area = h_areas[main_idx + 1 + i];
            let slot_rects = split_column_vertical(&col.column, col_area);
            for (slot, rect) in slot_rects {
                panels.push((slot.id, rect));
            }
        }

        LayoutRegions {
            panels,
            main: main_area,
        }
    }

    /// Get all panel IDs in focus-cycle order (left-to-right, top-to-bottom).
    pub fn focus_order(&self) -> Vec<PanelId> {
        let mut order = Vec::new();
        // Left columns first
        for col in &self.columns {
            if col.side == ColumnSide::Left {
                for slot in col.column.visible_slots() {
                    order.push(slot.id);
                }
            }
        }
        // Right columns
        for col in &self.columns {
            if col.side == ColumnSide::Right {
                for slot in col.column.visible_slots() {
                    order.push(slot.id);
                }
            }
        }
        order
    }

    /// Get the side that a panel is on (for h/l navigation).
    pub fn panel_side(&self, id: PanelId) -> Option<ColumnSide> {
        for col in &self.columns {
            if col.column.slots.iter().any(|s| s.id == id) {
                return Some(col.side);
            }
        }
        None
    }

    /// Get the next panel in the same column (for Tab cycling within a column).
    pub fn next_in_column(&self, current: PanelId) -> Option<PanelId> {
        for col in &self.columns {
            let visible = col.column.visible_slots();
            if let Some(pos) = visible.iter().position(|s| s.id == current) {
                let next = (pos + 1) % visible.len();
                return Some(visible[next].id);
            }
        }
        None
    }

    /// Get panel IDs on a given side.
    pub fn panels_on_side(&self, side: ColumnSide) -> Vec<PanelId> {
        self.columns
            .iter()
            .filter(|c| c.side == side)
            .flat_map(|c| c.column.visible_slots())
            .map(|s| s.id)
            .collect()
    }

    /// Move a panel to a different position. Returns true if successful.
    pub fn move_panel(&mut self, id: PanelId, target_col: usize, target_pos: usize) -> bool {
        // Remove from current position
        let mut removed = None;
        for col in &mut self.columns {
            if let Some(pos) = col.column.slots.iter().position(|s| s.id == id) {
                removed = Some(col.column.slots.remove(pos));
                break;
            }
        }
        let slot = match removed {
            Some(s) => s,
            None => return false,
        };
        // Insert at target
        if target_col >= self.columns.len() {
            return false;
        }
        let col = &mut self.columns[target_col].column;
        let pos = target_pos.min(col.slots.len());
        col.slots.insert(pos, slot);
        true
    }

    /// Toggle a panel's visibility (weight 0 ↔ 1).
    pub fn toggle_panel(&mut self, id: PanelId) {
        for col in &mut self.columns {
            if let Some(slot) = col.column.slots.iter_mut().find(|s| s.id == id) {
                slot.weight = u16::from(slot.weight == 0);
                return;
            }
        }
    }
}

/// Split a column vertically according to slot weights.
fn split_column_vertical(column: &Column, area: Rect) -> Vec<(&PanelSlot, Rect)> {
    let visible = column.visible_slots();
    if visible.is_empty() {
        return vec![];
    }

    let total_weight: u16 = visible.iter().map(|s| s.weight).sum();
    if total_weight == 0 {
        return vec![];
    }

    let constraints: Vec<Constraint> =
        visible.iter().map(|s| Constraint::Ratio(u32::from(s.weight), u32::from(total_weight))).collect();

    let rects = Layout::default().direction(Direction::Vertical).constraints(constraints).split(area);

    visible.into_iter().zip(rects.iter()).map(|(slot, &rect)| (slot, rect)).collect()
}

// ── Layout regions (the split result) ───────────────────────────────────────

/// Result of splitting the terminal area according to a `PanelLayout`.
pub struct LayoutRegions {
    /// Each panel's assigned area.
    pub panels: Vec<(PanelId, Rect)>,
    /// The main chat area.
    pub main: Rect,
}

impl LayoutRegions {
    /// Get the area for a specific panel.
    pub fn panel_area(&self, id: PanelId) -> Option<Rect> {
        self.panels.iter().find(|(pid, _)| *pid == id).map(|(_, r)| *r)
    }
}

// ── Focus tracker ───────────────────────────────────────────────────────────

/// Tracks which panel (if any) currently has focus.
#[derive(Debug, Clone, Default)]
pub struct FocusTracker {
    /// Currently focused panel, or None if the main chat has focus.
    pub focused: Option<PanelId>,
}

impl FocusTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_focused(&self, id: PanelId) -> bool {
        self.focused == Some(id)
    }

    pub fn has_panel_focus(&self) -> bool {
        self.focused.is_some()
    }

    /// Focus the first panel on the given side.
    pub fn focus_side(&mut self, layout: &PanelLayout, side: ColumnSide) {
        let panels = layout.panels_on_side(side);
        self.focused = panels.into_iter().next();
    }

    /// Cycle focus to the next panel in the same column.
    pub fn cycle_in_column(&mut self, layout: &PanelLayout) {
        if let Some(current) = self.focused {
            self.focused = layout.next_in_column(current).or(Some(current));
        }
    }

    /// Unfocus (return to main chat).
    pub fn unfocus(&mut self) {
        self.focused = None;
    }

    /// Focus a specific panel.
    pub fn focus(&mut self, id: PanelId) {
        self.focused = Some(id);
    }
}

// ── Preset layouts ──────────────────────────────────────────────────────────

impl PanelLayout {
    /// Default three-column layout (matches the original hardcoded one).
    pub fn default_three_column() -> Self {
        Self::default()
    }

    /// Wide chat layout: thin left sidebar only.
    pub fn wide_chat() -> Self {
        Self {
            columns: vec![SideColumn {
                side: ColumnSide::Left,
                column: Column::new(20, vec![
                    PanelSlot::new(PanelId::Todo),
                    PanelSlot::new(PanelId::Files),
                    PanelSlot::new(PanelId::Subagents),
                ]),
            }],
        }
    }

    /// Focused layout: no side panels.
    pub fn focused() -> Self {
        Self::chat_only()
    }

    /// Right-heavy layout: everything on the right.
    pub fn right_heavy() -> Self {
        Self {
            columns: vec![SideColumn {
                side: ColumnSide::Right,
                column: Column::new(30, vec![
                    PanelSlot::new(PanelId::Todo),
                    PanelSlot::new(PanelId::Files),
                    PanelSlot::new(PanelId::Subagents),
                    PanelSlot::new(PanelId::Peers),
                ]),
            }],
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_layout_split() {
        let layout = PanelLayout::default();
        let area = Rect::new(0, 0, 200, 50);
        let regions = layout.split(area);

        // Should have 4 panel regions
        assert_eq!(regions.panels.len(), 4);

        // Main area should exist and be non-zero
        assert!(regions.main.width > 0);
        assert!(regions.main.height > 0);

        // All panels should be found
        assert!(regions.panel_area(PanelId::Todo).is_some());
        assert!(regions.panel_area(PanelId::Files).is_some());
        assert!(regions.panel_area(PanelId::Subagents).is_some());
        assert!(regions.panel_area(PanelId::Peers).is_some());
    }

    #[test]
    fn test_chat_only_layout() {
        let layout = PanelLayout::chat_only();
        let area = Rect::new(0, 0, 200, 50);
        let regions = layout.split(area);

        assert!(regions.panels.is_empty());
        assert_eq!(regions.main.width, 200);
    }

    #[test]
    fn test_focus_order() {
        let layout = PanelLayout::default();
        let order = layout.focus_order();
        // Left panels first, then right
        assert_eq!(order, vec![PanelId::Todo, PanelId::Files, PanelId::Subagents, PanelId::Peers]);
    }

    #[test]
    fn test_next_in_column() {
        let layout = PanelLayout::default();
        assert_eq!(layout.next_in_column(PanelId::Todo), Some(PanelId::Files));
        assert_eq!(layout.next_in_column(PanelId::Files), Some(PanelId::Todo)); // wraps
        assert_eq!(layout.next_in_column(PanelId::Subagents), Some(PanelId::Peers));
        assert_eq!(layout.next_in_column(PanelId::Peers), Some(PanelId::Subagents));
    }

    #[test]
    fn test_panel_side() {
        let layout = PanelLayout::default();
        assert_eq!(layout.panel_side(PanelId::Todo), Some(ColumnSide::Left));
        assert_eq!(layout.panel_side(PanelId::Subagents), Some(ColumnSide::Right));
    }

    #[test]
    fn test_focus_tracker() {
        let layout = PanelLayout::default();
        let mut focus = FocusTracker::new();

        assert!(!focus.has_panel_focus());
        focus.focus_side(&layout, ColumnSide::Left);
        assert!(focus.is_focused(PanelId::Todo));

        focus.cycle_in_column(&layout);
        assert!(focus.is_focused(PanelId::Files));

        focus.cycle_in_column(&layout);
        assert!(focus.is_focused(PanelId::Todo)); // wraps

        focus.unfocus();
        assert!(!focus.has_panel_focus());
    }

    #[test]
    fn test_toggle_panel() {
        let mut layout = PanelLayout::default();
        let area = Rect::new(0, 0, 200, 50);

        // Before toggle: 4 panels visible
        let regions = layout.split(area);
        assert_eq!(regions.panels.len(), 4);

        // Hide Peers
        layout.toggle_panel(PanelId::Peers);
        let regions = layout.split(area);
        assert_eq!(regions.panels.len(), 3);
        assert!(regions.panel_area(PanelId::Peers).is_none());

        // Show it again
        layout.toggle_panel(PanelId::Peers);
        let regions = layout.split(area);
        assert_eq!(regions.panels.len(), 4);
    }

    #[test]
    fn test_move_panel() {
        let mut layout = PanelLayout::default();
        // Move Todo to right column
        assert!(layout.move_panel(PanelId::Todo, 1, 0));

        let order = layout.focus_order();
        // Todo should now be in the right column
        assert_eq!(order[0], PanelId::Files); // only left panel remaining
        assert!(order.contains(&PanelId::Todo)); // moved to right
    }

    #[test]
    fn test_preset_layouts() {
        let area = Rect::new(0, 0, 200, 50);

        // Each preset should split without panicking
        let _ = PanelLayout::default_three_column().split(area);
        let _ = PanelLayout::wide_chat().split(area);
        let _ = PanelLayout::focused().split(area);
        let _ = PanelLayout::right_heavy().split(area);
    }
}
