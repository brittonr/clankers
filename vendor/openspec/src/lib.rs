//! Spec-driven development (OpenSpec)
#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", feature(register_tool), register_tool(tigerstyle))]
#![cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        tigerstyle::assertion_density,
        tigerstyle::nested_conditionals,
        tigerstyle::unbounded_collection_growth,
        tigerstyle::bool_naming,
        tigerstyle::unbounded_loop,
        tigerstyle::ambiguous_params,
        tigerstyle::ambient_clock,
        tigerstyle::raw_arithmetic_overflow,
        tigerstyle::unchecked_division,
        tigerstyle::ignored_result,
        tigerstyle::no_recursion,
        tigerstyle::too_many_parameters,
        tigerstyle::explicit_defaults,
        tigerstyle::numeric_units,
        reason = "vendored OpenSpec snapshot is third-party compatibility code; Clankers gates track local integration separately"
    )
)]

pub mod core;

#[cfg(feature = "fs")]
pub mod config;

#[cfg(feature = "fs")]
pub mod engine;

// Re-export core types at the top level
pub use core::*;

#[cfg(feature = "fs")]
pub use config::SpecConfig;
// Re-export SpecEngine and config behind fs feature
#[cfg(feature = "fs")]
pub use engine::SpecEngine;
