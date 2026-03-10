//! Slash command system
//!
//! Slash commands are prefixed with `/` in the input editor and provide
//! quick access to common operations like clearing context, switching models,
//! showing help, etc.

pub mod handlers;

use std::cell::RefCell;
use std::collections::HashMap;

use clankers_tui_types::Conflict;
use clankers_tui_types::MenuPlacement;
use clankers_tui_types::PRIORITY_BUILTIN;

// ---------------------------------------------------------------------------
// Slash command dispatch is handled by `SlashRegistry::dispatch()` below.
// ---------------------------------------------------------------------------

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

/// Get builtin command info (for leader menu building without importing SlashCommand).
pub fn builtin_command_infos() -> Vec<clankers_tui_types::SlashCommandInfo> {
    builtin_commands()
        .iter()
        .map(|cmd| {
            let leader_key = cmd.leader_key.as_ref().map(|b| clankers_tui_types::LeaderBinding {
                key: b.key,
                placement: b.placement.clone(),
                label: b.label.map(|s| s.to_string()),
            });
            clankers_tui_types::SlashCommandInfo {
                name: cmd.name.to_string(),
                description: cmd.description.to_string(),
                leader_key,
            }
        })
        .collect()
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
        let mut all_commands: Vec<SlashCommandDef> = contributors.iter().flat_map(|c| c.slash_commands()).collect();

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
        let mut cmds: Vec<_> = self.commands.values().filter(|c| c.name.starts_with(partial)).collect();
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
        Box::new(handlers::context::CdHandler),
        Box::new(handlers::context::ShellHandler),
        Box::new(handlers::info::ExportHandler),
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
        Box::new(handlers::loop_cmd::LoopHandler),
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
                    subcommands: cmd.subcommands.iter().map(|(n, d)| (n.to_string(), d.to_string())).collect(),
                    handler,
                    priority: PRIORITY_BUILTIN,
                    source: "builtin".to_string(),
                    leader_key: cmd.leader_key,
                }
            })
            .collect()
    }
}

/// Parse a slash command string into (command_name, args).
/// Returns `None` if the input doesn't start with `/`.
/// Unknown commands are returned as-is (prompt template fallback).
///
/// # Tiger Style
///
/// Pure function — no I/O. Command names are bounded to 64 chars and
/// validated for allowed characters (alphanumeric, dash, underscore).
pub fn parse_command(input: &str) -> Option<(String, String)> {
    /// Tiger Style: maximum command name length.
    const MAX_COMMAND_NAME_LEN: usize = 64;

    let input = input.trim();
    if !input.starts_with('/') {
        return None;
    }

    let without_slash = &input[1..];
    let (cmd_name, args) = match without_slash.split_once(char::is_whitespace) {
        Some((name, rest)) => (name, rest.trim().to_string()),
        None => (without_slash, String::new()),
    };

    // Tiger Style: reject absurdly long command names.
    if cmd_name.len() > MAX_COMMAND_NAME_LEN {
        return None;
    }

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
pub use completion::CompletionItem;
pub use completion::completions;
pub use completion::completions_from_registry;
pub use completion::help_text;

impl clankers_tui_types::CompletionSource for SlashRegistry {
    fn completions(&self, input: &str) -> Vec<clankers_tui_types::CompletionItem> {
        completions_from_registry(self, input)
            .into_iter()
            .map(|c| clankers_tui_types::CompletionItem {
                display: c.display,
                description: c.description.to_string(),
                insert_text: c.insert_text,
                trailing_space: c.trailing_space,
            })
            .collect()
    }

    fn slash_commands(&self) -> Vec<clankers_tui_types::SlashCommandInfo> {
        self.all_commands()
            .into_iter()
            .map(|def| {
                let leader_key = def.leader_key.as_ref().map(|binding| clankers_tui_types::LeaderBinding {
                    key: binding.key,
                    placement: binding.placement.clone(),
                    label: binding.label.map(|s| s.to_string()),
                });
                clankers_tui_types::SlashCommandInfo {
                    name: def.name.clone(),
                    description: def.description.clone(),
                    leader_key,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
