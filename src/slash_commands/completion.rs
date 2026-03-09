//! Slash command completion engine.
//!
//! Provides autocomplete for `/` commands including subcommands
//! and prompt templates.

use super::PROMPT_TEMPLATE_CACHE;
use super::SlashRegistry;
use super::builtin_commands;

/// A completion item returned by the autocomplete system.
#[derive(Debug, Clone)]
pub struct CompletionItem {
    /// Display name (e.g. "account" or "switch <name>")
    pub display: String,
    /// Description shown next to it
    pub description: &'static str,
    /// Full text to insert when accepted (without leading `/`), e.g. "account switch "
    pub insert_text: String,
    /// Whether accepting this should add a trailing space
    pub trailing_space: bool,
}

/// Get completions for a partial slash command input from a registry.
/// The input should include the leading `/`.
pub fn completions_from_registry(registry: &SlashRegistry, input: &str) -> Vec<CompletionItem> {
    let input = input.trim_start();
    if !input.starts_with('/') {
        return Vec::new();
    }

    let partial = &input[1..];

    // If there's a space, the command name is complete — show subcommands
    if let Some((cmd_name, sub_partial)) = partial.split_once(char::is_whitespace) {
        let sub_partial = sub_partial.trim_start();
        if let Some(cmd) = registry.get(cmd_name)
            && !cmd.subcommands.is_empty()
        {
            // Only show subcommands if user hasn't typed past the subcommand keyword
            // (i.e., don't keep showing menu when typing arguments)
            let sub_word = sub_partial.split_whitespace().next().unwrap_or("");
            let has_more_words = sub_partial.contains(char::is_whitespace);

            // If the user has typed more than just the subcommand keyword, hide menu
            if has_more_words {
                return Vec::new();
            }

            return cmd
                .subcommands
                .iter()
                .filter(|(name, _)| {
                    // Match against the first word of the subcommand name
                    let first_word = name.split_whitespace().next().unwrap_or(name);
                    sub_word.is_empty() || first_word.starts_with(sub_word)
                })
                .map(|(name, desc)| {
                    let first_word = name.split_whitespace().next().unwrap_or(name);
                    CompletionItem {
                        display: name.clone(),
                        description: Box::leak(desc.clone().into_boxed_str()),
                        insert_text: format!("{} {} ", cmd_name, first_word),
                        trailing_space: false, // already included
                    }
                })
                .collect();
        }
        return Vec::new();
    }

    // Top-level command completion
    let mut items: Vec<CompletionItem> = registry
        .completions(partial)
        .into_iter()
        .map(|c| CompletionItem {
            display: c.name.clone(),
            description: Box::leak(c.description.clone().into_boxed_str()),
            insert_text: c.name.clone(),
            trailing_space: c.accepts_args,
        })
        .collect();

    // Also include prompt templates from the thread-local cache
    PROMPT_TEMPLATE_CACHE.with(|cache| {
        for (name, desc) in cache.borrow().iter() {
            if name.starts_with(partial) && !items.iter().any(|i| i.display == *name) {
                items.push(CompletionItem {
                    display: name.clone(),
                    // Leak the description so we get a &'static str.
                    // These are cached for the lifetime of the process anyway.
                    description: Box::leak(desc.clone().into_boxed_str()),
                    insert_text: name.clone(),
                    trailing_space: true,
                });
            }
        }
    });

    items
}

/// Get completions for a partial slash command input.
/// The input should include the leading `/`.
///
/// NOTE: This is a legacy compatibility function that uses builtin_commands().
/// Prefer `completions_from_registry()` when you have access to a registry.
pub fn completions(input: &str) -> Vec<CompletionItem> {
    let input = input.trim_start();
    if !input.starts_with('/') {
        return Vec::new();
    }

    let partial = &input[1..];

    // If there's a space, the command name is complete — show subcommands
    if let Some((cmd_name, sub_partial)) = partial.split_once(char::is_whitespace) {
        let sub_partial = sub_partial.trim_start();
        let commands = builtin_commands();
        if let Some(cmd) = commands.iter().find(|c| c.name == cmd_name)
            && !cmd.subcommands.is_empty()
        {
            // Only show subcommands if user hasn't typed past the subcommand keyword
            // (i.e., don't keep showing menu when typing arguments)
            let sub_word = sub_partial.split_whitespace().next().unwrap_or("");
            let has_more_words = sub_partial.contains(char::is_whitespace);

            // If the user has typed more than just the subcommand keyword, hide menu
            if has_more_words {
                return Vec::new();
            }

            return cmd
                .subcommands
                .iter()
                .filter(|(name, _)| {
                    // Match against the first word of the subcommand name
                    let first_word = name.split_whitespace().next().unwrap_or(name);
                    sub_word.is_empty() || first_word.starts_with(sub_word)
                })
                .map(|(name, desc)| {
                    let first_word = name.split_whitespace().next().unwrap_or(name);
                    CompletionItem {
                        display: name.to_string(),
                        description: desc,
                        insert_text: format!("{} {} ", cmd_name, first_word),
                        trailing_space: false, // already included
                    }
                })
                .collect();
        }
        return Vec::new();
    }

    // Top-level command completion
    let mut items: Vec<CompletionItem> = builtin_commands()
        .into_iter()
        .filter(|c| c.name.starts_with(partial))
        .map(|c| CompletionItem {
            display: c.name.to_string(),
            description: c.description,
            insert_text: c.name.to_string(),
            trailing_space: c.accepts_args,
        })
        .collect();

    // Also include prompt templates from the thread-local cache
    PROMPT_TEMPLATE_CACHE.with(|cache| {
        for (name, desc) in cache.borrow().iter() {
            if name.starts_with(partial) && !items.iter().any(|i| i.display == *name) {
                items.push(CompletionItem {
                    display: name.clone(),
                    // Leak the description so we get a &'static str.
                    // These are cached for the lifetime of the process anyway.
                    description: Box::leak(desc.clone().into_boxed_str()),
                    insert_text: name.clone(),
                    trailing_space: true,
                });
            }
        }
    });

    items
}

/// Format help text listing all commands
pub fn help_text() -> String {
    let commands = builtin_commands();
    let mut out = String::from("Available slash commands:\n\n");
    let max_name_len = commands.iter().map(|c| c.name.len()).max().unwrap_or(0);
    for cmd in &commands {
        out.push_str(&format!("  /{:<width$}  {}\n", cmd.name, cmd.description, width = max_name_len));
    }
    out
}
