//! Terminal UI (ratatui + crossterm)
//!
//! Terminal coordinates (row/col) and widget dimensions are bounded by
//! physical terminal size (typically ≤500). All `as usize` / `as u16`
//! conversions in this crate operate on display coordinates that fit
//! in any integer type on any supported platform.
#![cfg_attr(dylint_lib = "tigerstyle", allow(platform_dependent_cast, reason = "terminal coordinates always fit in usize/u16"))]

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
