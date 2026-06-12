//! Shared utility functions for clankers.
#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", feature(register_tool), register_tool(tigerstyle))]
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
