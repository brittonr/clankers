//! Session persistence manager — re-exported from `clankers-session`.

// Re-export everything from the extracted crate
pub use clankers_session::*;

// `to_merge_view` stays here because it depends on `clankers_tui_types`
// which is a TUI-layer dependency not suitable for the session crate.
pub mod merge_view;
