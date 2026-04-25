//! Slash command completion engine.
//!
//! Provides autocomplete for `/` commands including subcommands
//! and prompt templates.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use std::collections::HashSet;

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

fn build_subcommand_completions<'a, D, I, F>(
    cmd_name: &str,
    subcommands: I,
    sub_word: &str,
    mut description: F,
) -> Vec<CompletionItem>
where
    I: IntoIterator<Item = (&'a str, D)>,
    F: FnMut(D) -> &'static str,
{
    let mut seen_first_words = HashSet::new();

    subcommands
        .into_iter()
        .filter_map(|(name, desc)| {
            let first_word = name.split_whitespace().next().unwrap_or(name);
            if !sub_word.is_empty() && !first_word.starts_with(sub_word) {
                return None;
            }
            if !seen_first_words.insert(first_word.to_string()) {
                return None;
            }

            Some(CompletionItem {
                display: name.to_string(),
                description: description(desc),
                insert_text: format!("{} {} ", cmd_name, first_word),
                trailing_space: false,
            })
        })
        .collect()
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

            return build_subcommand_completions(
                cmd_name,
                cmd.subcommands.iter().map(|(name, desc)| (name.as_str(), desc.as_str())),
                sub_word,
                |desc| Box::leak(desc.to_string().into_boxed_str()),
            );
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

            return build_subcommand_completions(
                cmd_name,
                cmd.subcommands.iter().map(|(name, desc)| (*name, *desc)),
                sub_word,
                |desc| desc,
            );
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
    use std::fmt::Write;
    let commands = builtin_commands();
    let mut out = String::from("Available slash commands:\n\n");
    let max_name_len = commands.iter().map(|c| c.name.len()).max().unwrap_or(0);
    for cmd in &commands {
        writeln!(out, "  /{:<width$}  {}", cmd.name, cmd.description, width = max_name_len).ok();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completions_prefix_match() {
        // Test that "/m" returns commands starting with 'm'
        let results = completions("/m");

        assert!(!results.is_empty(), "Should find commands starting with 'm'");

        // All results should start with 'm'
        for item in &results {
            assert!(item.display.starts_with('m'), "Command '{}' doesn't start with 'm'", item.display);
        }

        // Should find common 'm' commands (model, memory, merge, etc.)
        let names: Vec<&str> = results.iter().map(|i| i.display.as_str()).collect();
        assert!(names.contains(&"model"), "Should include 'model' command");
        assert!(names.contains(&"memory"), "Should include 'memory' command");
        assert!(names.contains(&"merge"), "Should include 'merge' command");
    }

    #[test]
    fn test_completions_all_commands() {
        // Test that "/" returns all commands
        let results = completions("/");

        assert!(!results.is_empty(), "Should return all commands");

        // Should include some well-known commands
        let names: Vec<&str> = results.iter().map(|i| i.display.as_str()).collect();
        assert!(names.contains(&"help"), "Should include 'help'");
        assert!(names.contains(&"clear"), "Should include 'clear'");
        assert!(names.contains(&"model"), "Should include 'model'");
        assert!(names.contains(&"quit"), "Should include 'quit'");

        // Should have a reasonable number of commands (at least 20 builtins)
        assert!(results.len() >= 20, "Should have at least 20 builtin commands");
    }

    #[test]
    fn test_completions_no_slash() {
        // Test that input without slash returns empty
        let results = completions("no-slash");

        assert!(results.is_empty(), "Should return empty for non-slash input");
    }

    #[test]
    fn test_completions_nonexistent() {
        // Test that "/nonexistent" returns empty
        let results = completions("/nonexistent");

        assert!(results.is_empty(), "Should return empty for nonexistent command");
    }

    #[test]
    fn test_completions_subcommands() {
        // Test that "/account " returns subcommands
        let results = completions("/account ");

        assert!(!results.is_empty(), "Should return subcommands for 'account'");

        // Should include known account subcommands exactly once per keyword
        let displays: Vec<&str> = results.iter().map(|i| i.display.as_str()).collect();
        assert!(displays.iter().any(|d| d.contains("switch")), "Should include 'switch' subcommand");
        assert!(displays.iter().any(|d| d.contains("login")), "Should include 'login' subcommand");
        assert!(displays.iter().any(|d| d.contains("logout")), "Should include 'logout' subcommand");
        assert_eq!(results.iter().filter(|item| item.insert_text == "account switch ").count(), 1);
        assert_eq!(results.iter().filter(|item| item.insert_text == "account login ").count(), 1);
        assert_eq!(results.iter().filter(|item| item.insert_text == "account logout ").count(), 1);

        // Verify insert_text format includes command name
        for item in &results {
            assert!(
                item.insert_text.starts_with("account "),
                "Insert text should start with 'account ', got: '{}'",
                item.insert_text
            );
        }
    }

    #[test]
    fn test_completions_subcommands_filtered() {
        // Test that "/account sw" filters to subcommands starting with 'sw'
        let results = completions("/account sw");

        assert_eq!(results.len(), 1, "Should collapse duplicate 'switch' variants to one completion");

        // Should only include 'switch' subcommand
        let displays: Vec<&str> = results.iter().map(|i| i.display.as_str()).collect();
        assert!(displays.iter().any(|d| d.contains("switch")), "Should include 'switch' subcommand");
        assert_eq!(results[0].insert_text, "account switch ");

        // Should NOT include other subcommands like login, logout, etc.
        assert!(!displays.iter().any(|d| d.contains("login")), "Should NOT include 'login' when filtering by 'sw'");
    }

    #[test]
    fn test_completions_past_subcommand() {
        // Test that "/account switch foo" returns empty (past subcommand)
        let results = completions("/account switch foo");

        assert!(results.is_empty(), "Should return empty when typing past subcommand arguments");
    }

    #[test]
    fn test_help_text_format() {
        // Test that help_text() contains all builtin commands and proper format
        let help = help_text();

        // Should start with "Available"
        assert!(help.starts_with("Available"), "Help text should start with 'Available'");

        // Should contain well-known commands
        assert!(help.contains("/help"), "Should list 'help' command");
        assert!(help.contains("/clear"), "Should list 'clear' command");
        assert!(help.contains("/model"), "Should list 'model' command");
        assert!(help.contains("/quit"), "Should list 'quit' command");
        assert!(help.contains("/account"), "Should list 'account' command");

        // Should have descriptions
        assert!(
            help.contains("Show available commands") || help.contains("Clear"),
            "Should include command descriptions"
        );
    }

    #[test]
    fn test_trailing_space_flag() {
        // Test that completion items have correct trailing_space
        let results = completions("/");

        // Commands that accept arguments should have trailing_space = true
        let model_cmd = results.iter().find(|i| i.display == "model");
        assert!(model_cmd.is_some(), "Should find 'model' command");
        assert!(model_cmd.unwrap().trailing_space, "'model' should have trailing_space=true (accepts args)");

        // Commands that don't accept arguments should have trailing_space = false
        let help_cmd = results.iter().find(|i| i.display == "help");
        assert!(help_cmd.is_some(), "Should find 'help' command");
        assert!(!help_cmd.unwrap().trailing_space, "'help' should have trailing_space=false (no args)");

        let quit_cmd = results.iter().find(|i| i.display == "quit");
        assert!(quit_cmd.is_some(), "Should find 'quit' command");
        assert!(!quit_cmd.unwrap().trailing_space, "'quit' should have trailing_space=false (no args)");
    }

    #[test]
    fn test_completions_from_registry() {
        // Test that completions_from_registry() with a fresh SlashRegistry works
        use crate::slash_commands::BuiltinSlashContributor;
        use crate::slash_commands::SlashContributor;
        use crate::slash_commands::SlashRegistry;

        let contributor = BuiltinSlashContributor;
        let (registry, _conflicts) = SlashRegistry::build(&[&contributor as &dyn SlashContributor]);

        // Test prefix matching
        let results = completions_from_registry(&registry, "/m");
        assert!(!results.is_empty(), "Registry should find commands starting with 'm'");
        for item in &results {
            assert!(item.display.starts_with('m'), "Command '{}' doesn't start with 'm'", item.display);
        }

        // Test all commands
        let all_results = completions_from_registry(&registry, "/");
        assert!(!all_results.is_empty(), "Registry should return all commands");
        assert!(all_results.len() >= 20, "Should have at least 20 commands");

        // Test no slash
        let no_slash = completions_from_registry(&registry, "no-slash");
        assert!(no_slash.is_empty(), "Should return empty for non-slash input");

        // Test subcommands
        let subcommands = completions_from_registry(&registry, "/account ");
        assert!(!subcommands.is_empty(), "Should return account subcommands");
        assert_eq!(subcommands.iter().filter(|item| item.insert_text == "account switch ").count(), 1);
        assert_eq!(subcommands.iter().filter(|item| item.insert_text == "account login ").count(), 1);
        assert_eq!(subcommands.iter().filter(|item| item.insert_text == "account logout ").count(), 1);

        // Test subcommand filtering
        let filtered = completions_from_registry(&registry, "/account sw");
        assert_eq!(filtered.len(), 1, "Should collapse duplicate 'switch' variants in registry completions");
        assert_eq!(filtered[0].insert_text, "account switch ");

        // Test past subcommand
        let past_sub = completions_from_registry(&registry, "/account switch foo");
        assert!(past_sub.is_empty(), "Should return empty past subcommand");
    }
}
