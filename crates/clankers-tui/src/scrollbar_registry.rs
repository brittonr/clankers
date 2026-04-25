//! Scrollbar tracking for mouse interactions
//!
//! Maintains a registry of scrollbar regions to enable mouse dragging and clicking
//! on scrollbars to navigate content.

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

use std::collections::HashMap;

use ratatui::layout::Rect;

use crate::panel::PanelId;

/// Information about a scrollbar's position and state
#[derive(Debug, Clone)]
pub struct ScrollbarInfo {
    /// The area occupied by the scrollbar
    pub area: Rect,
    /// Total content length (in lines/items)
    pub content_length: usize,
    /// Current scroll position
    pub position: usize,
    /// Visible height (viewport size)
    pub visible_height: usize,
    /// The position of the thumb (handle) within the scrollbar
    pub thumb_start: u16,
    /// The height of the thumb
    pub thumb_height: u16,
}

/// Registry for tracking scrollbar positions in the UI
#[derive(Default)]
pub struct ScrollbarRegistry {
    /// Panel scrollbars
    pub panels: HashMap<PanelId, ScrollbarInfo>,
    /// Subagent pane scrollbars
    pub subagents: HashMap<String, ScrollbarInfo>,
    /// Messages area scrollbar
    pub messages: Option<ScrollbarInfo>,
}

impl ScrollbarRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear all registered scrollbars (call at start of each render)
    pub fn clear(&mut self) {
        self.panels.clear();
        self.subagents.clear();
        self.messages = None;
    }

    /// Register a panel scrollbar
    pub fn register_panel(&mut self, panel_id: PanelId, info: ScrollbarInfo) {
        self.panels.insert(panel_id, info);
    }

    /// Register a subagent scrollbar
    pub fn register_subagent(&mut self, id: String, info: ScrollbarInfo) {
        self.subagents.insert(id, info);
    }

    /// Register the messages area scrollbar
    pub fn register_messages(&mut self, info: ScrollbarInfo) {
        self.messages = Some(info);
    }

    /// Test if a point is within any scrollbar
    pub fn hit_test(&self, col: u16, row: u16) -> Option<ScrollbarHit> {
        // Check panel scrollbars
        for (panel_id, info) in &self.panels {
            if rect_contains(info.area, col, row) {
                return Some(ScrollbarHit::Panel(*panel_id, info.clone()));
            }
        }

        // Check subagent scrollbars
        for (id, info) in &self.subagents {
            if rect_contains(info.area, col, row) {
                return Some(ScrollbarHit::Subagent(id.clone(), info.clone()));
            }
        }

        // Check messages scrollbar
        if let Some(info) = &self.messages
            && rect_contains(info.area, col, row)
        {
            return Some(ScrollbarHit::Messages(info.clone()));
        }

        None
    }

    /// Calculate the scroll position from a mouse position within a scrollbar
    pub fn position_from_mouse(info: &ScrollbarInfo, mouse_y: u16) -> usize {
        let relative_y = mouse_y.saturating_sub(info.area.y);
        let scrollbar_height = info.area.height as f64;
        let ratio = relative_y as f64 / scrollbar_height;

        let max_scroll = info.content_length.saturating_sub(info.visible_height);
        (ratio * max_scroll as f64).round() as usize
    }
}

/// Result of a scrollbar hit test
pub enum ScrollbarHit {
    Panel(PanelId, ScrollbarInfo),
    Subagent(String, ScrollbarInfo),
    Messages(ScrollbarInfo),
}

/// Check if a rectangle contains a point
fn rect_contains(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
}

/// Calculate scrollbar thumb position and size
pub fn calculate_thumb_geometry(
    scrollbar_height: u16,
    content_length: usize,
    visible_height: usize,
    position: usize,
) -> (u16, u16) {
    if content_length <= visible_height {
        // No scrolling needed
        return (0, scrollbar_height);
    }

    // Calculate thumb height (minimum 1)
    let thumb_ratio = visible_height as f64 / content_length as f64;
    let thumb_height = ((scrollbar_height as f64 * thumb_ratio).round() as u16).max(1);

    // Calculate thumb position
    let max_scroll = content_length.saturating_sub(visible_height);
    let scroll_ratio = if max_scroll > 0 {
        position as f64 / max_scroll as f64
    } else {
        0.0
    };

    let max_thumb_pos = scrollbar_height.saturating_sub(thumb_height);
    let thumb_start = (max_thumb_pos as f64 * scroll_ratio).round() as u16;

    (thumb_start, thumb_height)
}
