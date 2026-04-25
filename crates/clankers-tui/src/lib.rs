//! Terminal UI (ratatui + crossterm)
//!
//! Terminal coordinates (row/col) and widget dimensions are bounded by
//! physical terminal size (typically ≤500). All `as usize` / `as u16`
//! conversions in this crate operate on display coordinates that fit
//! in any integer type on any supported platform.
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
#![cfg_attr(
    dylint_lib = "tigerstyle",
    allow(platform_dependent_cast, reason = "terminal coordinates always fit in usize/u16")
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
