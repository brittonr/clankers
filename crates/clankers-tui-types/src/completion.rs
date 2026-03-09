//! Slash command completion types.

use crate::menu::MenuPlacement;

/// A completion item returned by the autocomplete system.
#[derive(Debug, Clone)]
pub struct CompletionItem {
    /// Display name (e.g. "account" or "switch <name>").
    pub display: String,
    /// Description shown next to it.
    pub description: String,
    /// Full text to insert when accepted (without leading `/`), e.g. "account switch ".
    pub insert_text: String,
    /// Whether accepting this should add a trailing space.
    pub trailing_space: bool,
}

/// Info about a slash command (for leader menu building).
#[derive(Debug, Clone)]
pub struct SlashCommandInfo {
    /// Command name (e.g. "new", "compact").
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Optional leader key binding.
    pub leader_key: Option<LeaderBinding>,
}

/// A leader key binding for a slash command.
#[derive(Debug, Clone)]
pub struct LeaderBinding {
    /// Key to press in the leader menu.
    pub key: char,
    /// Where in the menu this appears.
    pub placement: MenuPlacement,
    /// Override label (defaults to command description if None).
    pub label: Option<String>,
}

/// Trait for providing completions to the TUI without depending on `SlashRegistry`.
pub trait CompletionSource {
    /// Get completions for the given input text (e.g., "/com" → matching commands).
    fn completions(&self, input: &str) -> Vec<CompletionItem>;
    /// Get all available slash command info (for leader menu building).
    fn slash_commands(&self) -> Vec<SlashCommandInfo>;
}
