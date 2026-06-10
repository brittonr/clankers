//! Shared utility functions for clankers.
#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", feature(register_tool), register_tool(tigerstyle))]
#![cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        tigerstyle::assertion_density,
        tigerstyle::function_length,
        tigerstyle::unbounded_loop,
        tigerstyle::raw_arithmetic_overflow,
        tigerstyle::ambiguous_params,
        tigerstyle::usize_in_public_api,
        reason = "utility APIs preserve existing parser/truncation contracts and have focused tests"
    )
)]

pub mod ansi;
pub mod at_file;
pub mod direnv;
pub mod fs;
pub mod id;
pub mod logging;
pub mod parsing;
pub mod path_policy;
pub mod syntax;
pub mod token;
pub mod truncation;
