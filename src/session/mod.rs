//! Main-crate session display adapters.
//!
//! Session persistence types are imported from `clankers-session` directly.

// `to_merge_view` stays here because it depends on `clanker_tui_types`
// which is a TUI-layer dependency not suitable for the session crate.
pub mod merge_view;
