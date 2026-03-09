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

/// A snapshot-based completion source that caches all completions.
///
/// Created from a `CompletionSource` to decouple the TUI from the registry.
/// The TUI stores this; the main crate rebuilds it when the registry changes.
#[derive(Debug, Clone)]
pub struct CompletionSnapshot {
    /// All available completions.
    pub items: Vec<CompletionItem>,
    /// All slash command infos (for leader menu).
    pub commands: Vec<SlashCommandInfo>,
}

impl CompletionSnapshot {
    /// Create a snapshot from any completion source.
    pub fn from_source(source: &dyn CompletionSource) -> Self {
        Self {
            // Complete with empty string to get ALL completions
            items: source.completions("/"),
            commands: source.slash_commands(),
        }
    }
}

impl CompletionSource for CompletionSnapshot {
    fn completions(&self, input: &str) -> Vec<CompletionItem> {
        let query = input.trim_start_matches('/').to_lowercase();
        if query.is_empty() {
            return self.items.clone();
        }
        self.items
            .iter()
            .filter(|item| {
                item.display.to_lowercase().contains(&query) || item.insert_text.to_lowercase().contains(&query)
            })
            .cloned()
            .collect()
    }

    fn slash_commands(&self) -> Vec<SlashCommandInfo> {
        self.commands.clone()
    }
}
