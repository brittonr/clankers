//! Shared types for the dynamic registry pattern.
//!
//! All extensible subsystems (leader menu, slash commands, panels, etc.)
//! use the same priority scale and conflict reporting.

/// Priority for built-in (compile-time) registrations.
pub const PRIORITY_BUILTIN: u16 = 0;

/// Priority for plugin registrations (loaded at runtime from WASM).
pub const PRIORITY_PLUGIN: u16 = 100;

/// Priority for user config overrides (highest, always wins).
pub const PRIORITY_USER: u16 = 200;

/// A conflict detected during registry build.
///
/// When two sources register the same key in the same scope, the higher-priority
/// source wins and a `Conflict` is reported for diagnostics.
#[derive(Debug, Clone)]
pub struct Conflict {
    /// Which registry detected the conflict (e.g. "leader_menu").
    pub registry: &'static str,
    /// What conflicted (key char, command name, etc.).
    pub key: String,
    /// Source that won (e.g. "config", plugin name).
    pub winner: String,
    /// Source that lost.
    pub loser: String,
}
