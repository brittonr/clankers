//! Slash command system
//!
//! Slash commands are prefixed with `/` in the input editor and provide
//! quick access to common operations like clearing context, switching models,
//! showing help, etc.

pub mod handlers;

use std::cell::RefCell;
use std::collections::HashMap;

use crate::registry::{Conflict, PRIORITY_BUILTIN};
use crate::tui::components::leader_menu::MenuPlacement;

// ---------------------------------------------------------------------------
// Slash command dispatch — routes command names to handler implementations
// ---------------------------------------------------------------------------

/// Dispatch a slash command by name to its handler.
///
/// This is the single entry point for all slash command execution.
/// Command names map directly to handler structs in
/// `src/slash_commands/handlers/`.
///
/// NOTE: This function is superseded by `SlashRegistry::dispatch()`.
/// It remains as a compatibility fallback for contexts that don't have
/// access to the registry. Prefer using the registry when possible.
pub fn dispatch(
    command: &str,
    args: &str,
    ctx: &mut handlers::SlashContext<'_>,
) {
    use handlers::SlashHandler;

    match command {
        "help" => handlers::info::HelpHandler.handle(args, ctx),
        "clear" => handlers::context::ClearHandler.handle(args, ctx),
        "reset" => handlers::context::ResetHandler.handle(args, ctx),
        "model" => handlers::model::ModelHandler.handle(args, ctx),
        "status" => handlers::info::StatusHandler.handle(args, ctx),
        "usage" => handlers::info::UsageHandler.handle(args, ctx),
        "version" => handlers::info::VersionHandler.handle(args, ctx),
        "quit" => handlers::info::QuitHandler.handle(args, ctx),
        "session" => handlers::session::SessionHandler.handle(args, ctx),
        "undo" => handlers::context::UndoHandler.handle(args, ctx),
        "cd" => handlers::navigation::CdHandler.handle(args, ctx),
        "shell" => handlers::navigation::ShellHandler.handle(args, ctx),
        "export" => handlers::export::ExportHandler.handle(args, ctx),
        "compact" => handlers::context::CompactHandler.handle(args, ctx),
        "think" => handlers::model::ThinkHandler.handle(args, ctx),
        "login" => handlers::auth::LoginHandler.handle(args, ctx),
        "tools" => handlers::tools::ToolsHandler.handle(args, ctx),
        "plugin" => handlers::tools::PluginHandler.handle(args, ctx),
        "subagents" => handlers::swarm::SubagentsHandler.handle(args, ctx),
        "account" => handlers::auth::AccountHandler.handle(args, ctx),
        "todo" => handlers::tui::TodoHandler.handle(args, ctx),
        "worker" => handlers::swarm::WorkerHandler.handle(args, ctx),
        "share" => handlers::swarm::ShareHandler.handle(args, ctx),
        "plan" => handlers::tui::PlanHandler.handle(args, ctx),
        "review" => handlers::tui::ReviewHandler.handle(args, ctx),
        "role" => handlers::model::RoleHandler.handle(args, ctx),
        "system" => handlers::memory::SystemPromptHandler.handle(args, ctx),
        "memory" => handlers::memory::MemoryHandler.handle(args, ctx),
        "peers" => handlers::swarm::PeersHandler.handle(args, ctx),
        "editor" => handlers::tui::EditorHandler.handle(args, ctx),
        "preview" => handlers::tui::PreviewHandler.handle(args, ctx),
        "layout" => handlers::tui::LayoutHandler.handle(args, ctx),
        "fork" => handlers::branching::ForkHandler.handle(args, ctx),
        "rewind" => handlers::branching::RewindHandler.handle(args, ctx),
        "branches" => handlers::branching::BranchesHandler.handle(args, ctx),
        "switch" => handlers::branching::SwitchHandler.handle(args, ctx),
        "compare" => handlers::branching::CompareHandler.handle(args, ctx),
        "label" => handlers::branching::LabelHandler.handle(args, ctx),
        "merge" => handlers::branching::MergeHandler.handle(args, ctx),
        "merge-interactive" => handlers::branching::MergeInteractiveHandler.handle(args, ctx),
        "cherry-pick" => handlers::branching::CherryPickHandler.handle(args, ctx),
        "leader" => handlers::info::LeaderHandler.handle(args, ctx),
        // User-defined prompt templates: any name not matched above
        _ => {
            handlers::prompt_template::PromptTemplateHandler {
                template_name: command.to_string(),
            }
            .handle(args, ctx);
        }
    }
}

// Thread-local cache of prompt template names for slash completion.
// Populated at startup from discovered prompt templates.
thread_local! {
    static PROMPT_TEMPLATE_CACHE: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
}

/// Register prompt template names so they appear in slash command completions.
/// Call this at startup after discovering prompt templates.
pub fn register_prompt_templates(templates: &[(String, String)]) {
    PROMPT_TEMPLATE_CACHE.with(|cache| {
        let mut c = cache.borrow_mut();
        c.clear();
        c.extend(templates.iter().cloned());
    });
}

/// Binding for a slash command in the leader menu.
#[derive(Debug, Clone)]
pub struct LeaderBinding {
    /// Key to press in the leader menu.
    pub key: char,
    /// Where in the menu this appears.
    pub placement: MenuPlacement,
    /// Override label (defaults to SlashCommand.description if None).
    pub label: Option<&'static str>,
}

/// A registered slash command.
#[derive(Debug, Clone)]
pub struct SlashCommand {
    /// The command name (without the leading `/`)
    pub name: &'static str,
    /// Short description shown in the autocomplete menu
    pub description: &'static str,
    /// Longer help text
    pub help: &'static str,
    /// Whether the command accepts arguments
    pub accepts_args: bool,
    /// Subcommands shown in the autocomplete menu (name, description)
    pub subcommands: Vec<(&'static str, &'static str)>,
    /// Optional leader menu binding. When set, this command appears
    /// in the leader menu automatically.
    pub leader_key: Option<LeaderBinding>,
}

/// A fully registered slash command with handler.
pub struct SlashCommandDef {
    /// Command name (without leading `/`)
    pub name: String,
    /// Short description for autocomplete
    pub description: String,
    /// Longer help text
    pub help: String,
    /// Whether the command accepts arguments
    pub accepts_args: bool,
    /// Subcommands for nested autocomplete
    pub subcommands: Vec<(String, String)>,
    /// Handler that executes the command
    pub handler: Box<dyn handlers::SlashHandler>,
    /// Priority for conflict resolution
    pub priority: u16,
    /// Source identifier (e.g. "builtin", "plugin:calendar", "user")
    pub source: String,
    /// Optional leader menu binding
    pub leader_key: Option<LeaderBinding>,
}

// SlashAction enum deleted — dispatch uses command name strings directly.
// See dispatch() above.

/// All built-in slash commands.
/// This function is now a simple collector that asks each handler for its metadata.
pub fn builtin_commands() -> Vec<SlashCommand> {
    builtin_handlers().into_iter().map(|h| h.command()).collect()
}

// The giant 729-line Vec literal has been eliminated!
// Each handler now owns its own metadata via the `command()` method.
//
// To add a new command:
// 1. Create a handler struct in handlers/<domain>.rs
// 2. Implement SlashHandler with both command() and handle()
// 3. Add the handler to builtin_handlers() above
//
// That's it! No more maintaining metadata in two places.


// ---------------------------------------------------------------------------
// Registry system — dynamic command registration with conflict resolution
// ---------------------------------------------------------------------------

/// A source of slash commands (builtins, plugins, user config).
pub trait SlashContributor {
    fn slash_commands(&self) -> Vec<SlashCommandDef>;
}

/// Registry for slash commands with priority-based conflict resolution.
#[derive(Default)]
pub struct SlashRegistry {
    commands: HashMap<String, SlashCommandDef>,
}

impl SlashRegistry {
    /// Build from contributors. Higher priority wins on conflict.
    pub fn build(contributors: &[&dyn SlashContributor]) -> (Self, Vec<Conflict>) {
        let mut commands: HashMap<String, SlashCommandDef> = HashMap::new();
        let mut conflicts = Vec::new();

        // Collect all commands from all contributors
        let mut all_commands: Vec<SlashCommandDef> = contributors
            .iter()
            .flat_map(|c| c.slash_commands())
            .collect();

        // Sort by priority (highest first) so higher priority wins
        all_commands.sort_by_key(|cmd| std::cmp::Reverse(cmd.priority));

        // Register commands, tracking conflicts
        for cmd in all_commands {
            if let Some(existing) = commands.get(&cmd.name) {
                // Conflict: this command is lower priority (lost)
                conflicts.push(Conflict {
                    registry: "slash_commands",
                    key: cmd.name.clone(),
                    winner: existing.source.clone(),
                    loser: cmd.source.clone(),
                });
            } else {
                commands.insert(cmd.name.clone(), cmd);
            }
        }

        (Self { commands }, conflicts)
    }

    /// Dispatch a command by name.
    pub fn dispatch(&self, name: &str, args: &str, ctx: &mut handlers::SlashContext<'_>) {
        use handlers::SlashHandler;
        if let Some(cmd) = self.commands.get(name) {
            cmd.handler.handle(args, ctx);
        } else {
            // Fall through to prompt template handler
            handlers::prompt_template::PromptTemplateHandler {
                template_name: name.to_string(),
            }
            .handle(args, ctx);
        }
    }

    /// Get completions for a partial input.
    pub fn completions(&self, partial: &str) -> Vec<&SlashCommandDef> {
        let mut cmds: Vec<_> = self
            .commands
            .values()
            .filter(|c| c.name.starts_with(partial))
            .collect();
        cmds.sort_by_key(|c| &c.name);
        cmds
    }

    /// Get all registered commands (for help text).
    pub fn all_commands(&self) -> Vec<&SlashCommandDef> {
        let mut cmds: Vec<_> = self.commands.values().collect();
        cmds.sort_by_key(|c| &c.name);
        cmds
    }

    /// Get a single command definition by name.
    pub fn get(&self, name: &str) -> Option<&SlashCommandDef> {
        self.commands.get(name)
    }
}

/// Returns all builtin handler instances.
/// Each handler provides its own metadata via the `command()` method.
fn builtin_handlers() -> Vec<Box<dyn handlers::SlashHandler>> {
    vec![
        Box::new(handlers::info::HelpHandler),
        Box::new(handlers::context::ClearHandler),
        Box::new(handlers::context::ResetHandler),
        Box::new(handlers::context::CompactHandler),
        Box::new(handlers::context::UndoHandler),
        Box::new(handlers::model::ModelHandler),
        Box::new(handlers::model::ThinkHandler),
        Box::new(handlers::model::RoleHandler),
        Box::new(handlers::info::StatusHandler),
        Box::new(handlers::info::UsageHandler),
        Box::new(handlers::info::VersionHandler),
        Box::new(handlers::info::QuitHandler),
        Box::new(handlers::info::LeaderHandler),
        Box::new(handlers::session::SessionHandler),
        Box::new(handlers::navigation::CdHandler),
        Box::new(handlers::navigation::ShellHandler),
        Box::new(handlers::export::ExportHandler),
        Box::new(handlers::auth::LoginHandler),
        Box::new(handlers::auth::AccountHandler),
        Box::new(handlers::tools::ToolsHandler),
        Box::new(handlers::tools::PluginHandler),
        Box::new(handlers::swarm::WorkerHandler),
        Box::new(handlers::swarm::ShareHandler),
        Box::new(handlers::swarm::SubagentsHandler),
        Box::new(handlers::swarm::PeersHandler),
        Box::new(handlers::tui::TodoHandler),
        Box::new(handlers::tui::PreviewHandler),
        Box::new(handlers::tui::EditorHandler),
        Box::new(handlers::tui::LayoutHandler),
        Box::new(handlers::tui::PlanHandler),
        Box::new(handlers::tui::ReviewHandler),
        Box::new(handlers::memory::SystemPromptHandler),
        Box::new(handlers::memory::MemoryHandler),
        Box::new(handlers::branching::ForkHandler),
        Box::new(handlers::branching::RewindHandler),
        Box::new(handlers::branching::BranchesHandler),
        Box::new(handlers::branching::SwitchHandler),
        Box::new(handlers::branching::CompareHandler),
        Box::new(handlers::branching::LabelHandler),
        Box::new(handlers::branching::MergeHandler),
        Box::new(handlers::branching::MergeInteractiveHandler),
        Box::new(handlers::branching::CherryPickHandler),
    ]
}

/// Built-in slash command contributor.
pub struct BuiltinSlashContributor;

impl SlashContributor for BuiltinSlashContributor {
    fn slash_commands(&self) -> Vec<SlashCommandDef> {
        builtin_handlers()
            .into_iter()
            .map(|handler| {
                let cmd = handler.command();
                SlashCommandDef {
                    name: cmd.name.to_string(),
                    description: cmd.description.to_string(),
                    help: cmd.help.to_string(),
                    accepts_args: cmd.accepts_args,
                    subcommands: cmd
                        .subcommands
                        .iter()
                        .map(|(n, d)| (n.to_string(), d.to_string()))
                        .collect(),
                    handler,
                    priority: PRIORITY_BUILTIN,
                    source: "builtin".to_string(),
                    leader_key: cmd.leader_key,
                }
            })
            .collect()
    }
}

/// Parse a slash command from input text.
/// Returns `Some((action, args))` if the text starts with `/` and matches a command.
/// Returns `None` if it's not a slash command or doesn't match.
/// Parse a slash command string into (command_name, args).
/// Returns `None` if the input doesn't start with `/`.
/// Unknown commands are returned as-is (prompt template fallback).
pub fn parse_command(input: &str) -> Option<(String, String)> {
    let input = input.trim();
    if !input.starts_with('/') {
        return None;
    }

    let without_slash = &input[1..];
    let (cmd_name, args) = match without_slash.split_once(char::is_whitespace) {
        Some((name, rest)) => (name, rest.trim().to_string()),
        None => (without_slash, String::new()),
    };

    let commands = builtin_commands();
    if commands.iter().any(|c| c.name == cmd_name) {
        return Some((cmd_name.to_string(), args));
    }

    // Fall back to user-defined prompt templates: /fix, /test, etc.
    if !cmd_name.is_empty() && cmd_name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return Some((cmd_name.to_string(), args));
    }

    None
}


pub mod completion;
pub use completion::{CompletionItem, completions, completions_from_registry, help_text};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_command_basic() {
        let (cmd, args) = parse_command("/help").expect("failed to parse /help");
        assert_eq!(cmd, "help");
        assert_eq!(args, "");
    }

    #[test]
    fn test_parse_command_with_args() {
        let (cmd, args) = parse_command("/model claude-3-5-sonnet").expect("failed to parse /model");
        assert_eq!(cmd, "model");
        assert_eq!(args, "claude-3-5-sonnet");
    }

    #[test]
    fn test_parse_command_unknown_falls_through_to_prompt_template() {
        // Unknown commands now fall through to the prompt template system
        let result = parse_command("/nonexistent");
        assert!(result.is_some());
        let (cmd, _args) = result.expect("should have parsed unknown command");
        assert_eq!(cmd, "nonexistent");
    }

    #[test]
    fn test_parse_command_invalid_chars_returns_none() {
        // Commands with invalid characters should still return None
        assert!(parse_command("/").is_none());
    }

    #[test]
    fn test_parse_not_slash() {
        assert!(parse_command("hello").is_none());
    }

    #[test]
    fn test_completions_partial() {
        let results = completions("/he");
        assert!(results.iter().any(|c| c.display == "help"), "results: {:?}", results);
    }

    #[test]
    fn test_completions_empty_slash() {
        let results = completions("/");
        assert!(results.len() > 5); // Should return all commands
    }

    #[test]
    fn test_completions_with_space() {
        let results = completions("/model ");
        assert!(results.is_empty()); // Command complete, no more suggestions
    }

    #[test]
    fn test_help_text_not_empty() {
        let text = help_text();
        assert!(text.contains("/help"));
        assert!(text.contains("/clear"));
    }

    #[test]
    fn test_parse_login_no_args() {
        let (cmd, args) = parse_command("/login").expect("failed to parse /login");
        assert_eq!(cmd, "login");
        assert_eq!(args, "");
    }

    #[test]
    fn test_parse_login_with_code() {
        let (cmd, args) = parse_command("/login abc123#state456").expect("failed to parse /login with code");
        assert_eq!(cmd, "login");
        assert_eq!(args, "abc123#state456");
    }

    #[test]
    fn test_completions_login() {
        let results = completions("/lo");
        assert!(results.iter().any(|c| c.display == "login"), "results: {:?}", results);
    }

    #[test]
    fn test_help_text_includes_login() {
        let text = help_text();
        assert!(text.contains("/login"));
    }

    #[test]
    fn test_parse_worker_no_args() {
        let (cmd, args) = parse_command("/worker").expect("failed to parse /worker");
        assert_eq!(cmd, "worker");
        assert_eq!(args, "");
    }

    #[test]
    fn test_parse_worker_with_name_and_task() {
        let (cmd, args) = parse_command("/worker builder fix the tests").expect("failed to parse /worker with args");
        assert_eq!(cmd, "worker");
        assert_eq!(args, "builder fix the tests");
    }

    #[test]
    fn test_parse_share() {
        let (cmd, args) = parse_command("/share").expect("failed to parse /share");
        assert_eq!(cmd, "share");
        assert_eq!(args, "");
    }

    #[test]
    fn test_parse_share_read_only() {
        let (cmd, args) = parse_command("/share --read-only").expect("failed to parse /share with flag");
        assert_eq!(cmd, "share");
        assert_eq!(args, "--read-only");
    }

    #[test]
    fn test_completions_worker() {
        let results = completions("/wo");
        assert!(results.iter().any(|c| c.display == "worker"), "results: {:?}", results);
    }

    #[test]
    fn test_completions_share() {
        let results = completions("/sh");
        assert!(
            results.iter().any(|c| c.display == "share") || results.iter().any(|c| c.display == "shell"),
            "results: {:?}",
            results
        );
    }

    #[test]
    fn test_help_text_includes_worker_and_share() {
        let text = help_text();
        assert!(text.contains("/worker"));
        assert!(text.contains("/share"));
    }

    #[test]
    fn test_parse_system_no_args() {
        let (cmd, args) = parse_command("/system").expect("failed to parse /system");
        assert_eq!(cmd, "system");
        assert_eq!(args, "");
    }

    #[test]
    fn test_parse_system_show() {
        let (cmd, args) = parse_command("/system show").expect("failed to parse /system show");
        assert_eq!(cmd, "system");
        assert_eq!(args, "show");
    }

    #[test]
    fn test_parse_system_set() {
        let (cmd, args) = parse_command("/system set You are a helpful assistant.").expect("failed to parse /system set");
        assert_eq!(cmd, "system");
        assert_eq!(args, "set You are a helpful assistant.");
    }

    #[test]
    fn test_parse_system_append() {
        let (cmd, args) = parse_command("/system append Always be concise.").expect("failed to parse /system append");
        assert_eq!(cmd, "system");
        assert_eq!(args, "append Always be concise.");
    }

    #[test]
    fn test_parse_system_reset() {
        let (cmd, args) = parse_command("/system reset").expect("failed to parse /system reset");
        assert_eq!(cmd, "system");
        assert_eq!(args, "reset");
    }

    #[test]
    fn test_parse_system_file() {
        let (cmd, args) = parse_command("/system file /tmp/prompt.md").expect("failed to parse /system file");
        assert_eq!(cmd, "system");
        assert_eq!(args, "file /tmp/prompt.md");
    }

    #[test]
    fn test_completions_system() {
        let results = completions("/sy");
        assert!(results.iter().any(|c| c.display == "system"), "results: {:?}", results);
    }

    #[test]
    fn test_help_text_includes_system() {
        let text = help_text();
        assert!(text.contains("/system"));
    }

    #[test]
    fn test_parse_editor() {
        let (cmd, args) = parse_command("/editor").expect("failed to parse /editor");
        assert_eq!(cmd, "editor");
        assert_eq!(args, "");
    }

    #[test]
    fn test_completions_editor() {
        let results = completions("/ed");
        assert!(results.iter().any(|c| c.display == "editor"), "results: {:?}", results);
    }

    #[test]
    fn test_help_text_includes_editor() {
        let text = help_text();
        assert!(text.contains("/editor"));
    }

    #[test]
    fn test_account_subcommands_shown() {
        let results = completions("/account ");
        assert!(!results.is_empty(), "should show subcommands for /account");
        assert!(results.iter().any(|c| c.display.starts_with("switch")));
        assert!(results.iter().any(|c| c.display.starts_with("login")));
    }

    #[test]
    fn test_account_subcommand_filter() {
        let results = completions("/account sw");
        assert_eq!(results.len(), 1);
        assert!(results[0].display.starts_with("switch"));
    }

    #[test]
    fn test_account_subcommand_after_typing_args_hides() {
        let results = completions("/account switch foo");
        assert!(results.is_empty(), "should hide menu after typing args");
    }

    #[test]
    fn test_think_subcommands() {
        let results = completions("/think ");
        assert!(results.iter().any(|c| c.display == "off"));
        assert!(results.iter().any(|c| c.display == "max"));
    }

    #[test]
    fn test_no_subcommands_for_clear() {
        let results = completions("/clear ");
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_fork() {
        let (cmd, args) = parse_command("/fork").expect("failed to parse /fork");
        assert_eq!(cmd, "fork");
        assert_eq!(args, "");
    }

    #[test]
    fn test_parse_fork_with_args() {
        let (cmd, args) = parse_command("/fork try different approach").expect("failed to parse /fork with args");
        assert_eq!(cmd, "fork");
        assert_eq!(args, "try different approach");
    }

    #[test]
    fn test_parse_rewind() {
        let (cmd, args) = parse_command("/rewind 5").expect("failed to parse /rewind");
        assert_eq!(cmd, "rewind");
        assert_eq!(args, "5");
    }

    #[test]
    fn test_parse_branches() {
        let (cmd, args) = parse_command("/branches").expect("failed to parse /branches");
        assert_eq!(cmd, "branches");
        assert_eq!(args, "");
    }

    #[test]
    fn test_parse_switch() {
        let (cmd, args) = parse_command("/switch main").expect("failed to parse /switch");
        assert_eq!(cmd, "switch");
        assert_eq!(args, "main");
    }

    #[test]
    fn test_parse_label() {
        let (cmd, args) = parse_command("/label checkpoint").expect("failed to parse /label");
        assert_eq!(cmd, "label");
        assert_eq!(args, "checkpoint");
    }

    #[test]
    fn test_completions_fork() {
        let results = completions("/fo");
        assert!(results.iter().any(|c| c.display == "fork"), "results: {:?}", results);
    }

    #[test]
    fn test_completions_branches() {
        let results = completions("/br");
        assert!(results.iter().any(|c| c.display == "branches"), "results: {:?}", results);
    }

    #[test]
    fn test_help_text_includes_branch_commands() {
        let text = help_text();
        assert!(text.contains("/fork"));
        assert!(text.contains("/rewind"));
        assert!(text.contains("/branches"));
        assert!(text.contains("/switch"));
        assert!(text.contains("/label"));
    }

    // Registry tests
    #[test]
    fn test_simple_registry_check() {
        // Very simple test to verify registry basics
        let builtin = BuiltinSlashContributor;
        let cmds = builtin.slash_commands();
        assert!(!cmds.is_empty());
    }

    #[test]
    fn test_registry_build_from_builtins() {
        let builtin = BuiltinSlashContributor;
        let contributors: Vec<&dyn SlashContributor> = vec![&builtin];
        let (registry, conflicts) = SlashRegistry::build(&contributors);

        // Should have no conflicts when building from a single contributor
        assert_eq!(conflicts.len(), 0);

        // Should have all builtin commands
        assert_eq!(registry.all_commands().len(), 42);

        // Verify a few specific commands are present
        assert!(registry.get("help").is_some());
        assert!(registry.get("model").is_some());
        assert!(registry.get("fork").is_some());
        assert!(registry.get("system").is_some());
    }

    #[test]
    fn test_registry_conflict_resolution() {
        use crate::registry::PRIORITY_PLUGIN;

        // Create a mock contributor with a conflicting command
        struct MockContributor;
        impl SlashContributor for MockContributor {
            fn slash_commands(&self) -> Vec<SlashCommandDef> {
                vec![SlashCommandDef {
                    name: "help".to_string(),
                    description: "Plugin help override".to_string(),
                    help: "Overridden help".to_string(),
                    accepts_args: false,
                    subcommands: vec![],
                    handler: Box::new(handlers::info::HelpHandler),
                    priority: PRIORITY_PLUGIN, // Higher than builtin
                    source: "test_plugin".to_string(),
                    leader_key: None,
                }]
            }
        }

        let builtin = BuiltinSlashContributor;
        let mock = MockContributor;
        let contributors: Vec<&dyn SlashContributor> = vec![&builtin, &mock];
        let (registry, conflicts) = SlashRegistry::build(&contributors);

        // Should have one conflict (help)
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].key, "help");
        assert_eq!(conflicts[0].winner, "test_plugin");
        assert_eq!(conflicts[0].loser, "builtin");

        // The plugin version should win
        let help_cmd = registry.get("help").expect("help command should be registered");
        assert_eq!(help_cmd.description, "Plugin help override");
        assert_eq!(help_cmd.source, "test_plugin");
    }

    #[test]
    fn test_registry_completions() {
        let builtin = BuiltinSlashContributor;
        let contributors: Vec<&dyn SlashContributor> = vec![&builtin];
        let (registry, _) = SlashRegistry::build(&contributors);

        // Test prefix matching
        let completions = registry.completions("he");
        assert!(completions.iter().any(|c| c.name == "help"));

        // Test empty partial returns all
        let all_completions = registry.completions("");
        assert_eq!(all_completions.len(), 42);

        // Test no matches
        let no_match = registry.completions("xyz");
        assert_eq!(no_match.len(), 0);
    }

    #[test]
    fn test_registry_completions_from_registry_function() {
        let builtin = BuiltinSlashContributor;
        let contributors: Vec<&dyn SlashContributor> = vec![&builtin];
        let (registry, _) = SlashRegistry::build(&contributors);

        // Test the completions_from_registry function
        let results = completions_from_registry(&registry, "/he");
        assert!(results.iter().any(|c| c.display == "help"));

        // Test with subcommands
        let results = completions_from_registry(&registry, "/account ");
        assert!(!results.is_empty());
        assert!(results.iter().any(|c| c.display.starts_with("switch")));
    }

    #[test]
    fn test_registry_dispatch_unknown_falls_through() {
        let builtin = BuiltinSlashContributor;
        let contributors: Vec<&dyn SlashContributor> = vec![&builtin];
        let (registry, _) = SlashRegistry::build(&contributors);

        // Create a minimal SlashContext for testing
        // We'll use a channel that we can check for messages
        let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::unbounded_channel();
        let (panel_tx, _panel_rx) = tokio::sync::mpsc::unbounded_channel();

        let model = "test-model".to_string();
        let cwd = std::env::current_dir().expect("failed to get current dir").to_string_lossy().to_string();
        let theme = crate::tui::theme::Theme::dark();
        let mut app = crate::tui::app::App::new(model, cwd, theme);

        let mut ctx = handlers::SlashContext {
            app: &mut app,
            cmd_tx: &cmd_tx,
            plugin_manager: None,
            panel_tx: &panel_tx,
            db: &None,
            session_manager: &mut None,
        };

        // Dispatch an unknown command (should fall through to prompt template handler)
        registry.dispatch("unknown_command", "test args", &mut ctx);

        // The test passes if no panic occurred (prompt template handler doesn't fail)
    }

    #[test]
    fn test_registry_help_text_via_all_commands() {
        let builtin = BuiltinSlashContributor;
        let contributors: Vec<&dyn SlashContributor> = vec![&builtin];
        let (registry, _) = SlashRegistry::build(&contributors);

        let all_cmds = registry.all_commands();
        assert_eq!(all_cmds.len(), 42);

        // Commands should be sorted
        let names: Vec<_> = all_cmds.iter().map(|c| &c.name).collect();
        let mut sorted_names = names.clone();
        sorted_names.sort();
        assert_eq!(names, sorted_names);

        // Verify all expected commands are present
        assert!(all_cmds.iter().any(|c| c.name == "help"));
        assert!(all_cmds.iter().any(|c| c.name == "clear"));
        assert!(all_cmds.iter().any(|c| c.name == "fork"));
        assert!(all_cmds.iter().any(|c| c.name == "system"));
    }
}
