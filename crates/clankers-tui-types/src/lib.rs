//! Thin compatibility wrapper for the extracted `clanker-tui-types` crate.
//!
//! Existing workspace crates still import `clankers_tui_types`; this wrapper
//! keeps those paths working while callers migrate to `clanker_tui_types`.

pub use clanker_tui_types::*;
