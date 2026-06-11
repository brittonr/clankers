//! Terminal UI (ratatui + crossterm)
//!
//! Terminal coordinates (row/col) and widget dimensions are bounded by
//! physical terminal size (typically ≤500). All `as usize` / `as u16`
//! conversions in this crate operate on display coordinates that fit
//! in any integer type on any supported platform.
#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", feature(register_tool), register_tool(tigerstyle))]
#![cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        tigerstyle::platform_dependent_cast,
        tigerstyle::explicit_defaults,
        tigerstyle::usize_in_public_api,
        tigerstyle::ambiguous_params,
        tigerstyle::raw_arithmetic_overflow,
        tigerstyle::numeric_units,
        tigerstyle::unbounded_collection_growth,
        tigerstyle::ambient_clock,
        tigerstyle::too_many_parameters,
        tigerstyle::compound_condition,
        tigerstyle::bool_naming,
        tigerstyle::no_panic,
        reason = "TUI layout/rendering shell uses terminal-bounded coordinate formulas, ratatui builder defaults, display-only costs/timestamps, and visual snapshot coverage"
    )
)]
pub mod app;
pub mod clipboard;
pub mod components;
pub mod event;
pub mod keymap;
pub mod mouse;
pub mod panel;
pub mod panes;
pub mod render;
pub mod scrollbar_registry;
pub mod selection;
pub mod selectors;
pub mod theme;
pub mod widget_host;
