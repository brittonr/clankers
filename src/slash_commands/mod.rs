//! Slash command system
//!
//! Slash commands are prefixed with `/` in the input editor and provide
//! quick access to common operations like clearing context, switching models,
//! showing help, etc.

use std::cell::RefCell;
use std::fmt;

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

/// A registered slash command
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
    /// The action this command performs
    pub action: SlashAction,
    /// Subcommands shown in the autocomplete menu (name, description)
    pub subcommands: Vec<(&'static str, &'static str)>,
}

/// What happens when a slash command is executed
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlashAction {
    /// Clear the conversation history
    Clear,
    /// Show help / list commands
    Help,
    /// Switch to a different model
    Model,
    /// Show current model and settings
    Status,
    /// Reset the conversation (clear history + context)
    Reset,
    /// Compact/summarize the conversation
    Compact,
    /// Toggle thinking mode on/off
    Think,
    /// Quit the application
    Quit,
    /// Show version information
    Version,
    /// Show token usage statistics
    Usage,
    /// Change working directory
    Cd,
    /// Show session info
    Session,
    /// Undo the last user message (remove last turn)
    Undo,
    /// Export conversation to file
    Export,
    /// Run a raw shell command (bypass the agent)
    Shell,
    /// Authenticate with a provider (OAuth login)
    Login,
    /// Show plugin information
    Plugin,
    /// List available tools
    Tools,
    /// Spawn a swarm worker in a Zellij pane
    Worker,
    /// Share the current Zellij session
    Share,
    /// List/manage subagents
    Subagents,
    /// Manage todo list
    Todo,
    /// Switch/list accounts
    Account,
    /// Preview markdown rendering (debug/test)
    Preview,
    /// Toggle plan mode
    Plan,
    /// Code review mode
    Review,
    /// Switch model role
    Role,
    /// View or modify the system prompt
    SystemPrompt,
    /// Open $EDITOR to compose a multi-line prompt
    Editor,
    /// Manage cross-session memory
    Memory,
    /// Manage swarm peers
    Peers,
    /// Switch panel layout
    Layout,
    /// Fork conversation to explore alternatives
    Fork,
    /// Jump back to an earlier message
    Rewind,
    /// List conversation branches
    Branches,
    /// Switch to a different branch
    Switch,
    /// Label the current message
    Label,
    /// Run a user-defined prompt template
    PromptTemplate(String),
}

impl fmt::Display for SlashAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// All built-in slash commands
pub fn builtin_commands() -> Vec<SlashCommand> {
    vec![
        SlashCommand {
            name: "help",
            description: "Show available commands",
            help: "Lists all available slash commands with descriptions.",
            accepts_args: false,
            action: SlashAction::Help,
            subcommands: vec![],
        },
        SlashCommand {
            name: "clear",
            description: "Clear conversation history",
            help: "Clears the visible message history. Does not affect the agent's context window.",
            accepts_args: false,
            action: SlashAction::Clear,
            subcommands: vec![],
        },
        SlashCommand {
            name: "reset",
            description: "Reset conversation and context",
            help: "Clears conversation history and resets the agent context, starting fresh.",
            accepts_args: false,
            action: SlashAction::Reset,
            subcommands: vec![],
        },
        SlashCommand {
            name: "compact",
            description: "Summarize conversation to save tokens",
            help: "Asks the model to create a compact summary of the conversation so far, \
                   replacing the full history to reduce token usage.",
            accepts_args: false,
            action: SlashAction::Compact,
            subcommands: vec![],
        },
        SlashCommand {
            name: "model",
            description: "Switch model (e.g. /model claude-3-5-sonnet)",
            help: "Switch to a different model. Usage: /model <model-name>",
            accepts_args: true,
            action: SlashAction::Model,
            subcommands: vec![],
        },
        SlashCommand {
            name: "think",
            description: "Set thinking level (off/low/medium/high/max)",
            help: "Cycle or set extended thinking level.\n\n\
                   Usage:\n  \
                   /think              — cycle to next level\n  \
                   /think off          — disable thinking\n  \
                   /think low          — light reasoning (~5k tokens)\n  \
                   /think medium       — moderate reasoning (~10k tokens)\n  \
                   /think high         — deep reasoning (~32k tokens)\n  \
                   /think max          — maximum reasoning (~128k tokens)\n  \
                   /think <number>     — set budget directly (maps to nearest level)\n\n\
                   Keybinding: Ctrl+T cycles through levels.",
            accepts_args: true,
            action: SlashAction::Think,
            subcommands: vec![
                ("off", "disable thinking"),
                ("low", "light reasoning (~5k tokens)"),
                ("medium", "moderate reasoning (~10k tokens)"),
                ("high", "deep reasoning (~32k tokens)"),
                ("max", "maximum reasoning (~128k tokens)"),
            ],
        },
        SlashCommand {
            name: "status",
            description: "Show current settings",
            help: "Displays the current model, token usage, and session information.",
            accepts_args: false,
            action: SlashAction::Status,
            subcommands: vec![],
        },
        SlashCommand {
            name: "usage",
            description: "Show token usage statistics",
            help: "Shows detailed token usage and estimated cost for this session.",
            accepts_args: false,
            action: SlashAction::Usage,
            subcommands: vec![],
        },
        SlashCommand {
            name: "undo",
            description: "Remove last conversation turn",
            help: "Removes the last user message and assistant response from the conversation.",
            accepts_args: false,
            action: SlashAction::Undo,
            subcommands: vec![],
        },
        SlashCommand {
            name: "session",
            description: "Manage sessions",
            help: "Session management:\n  \
                   /session                — show current session info\n  \
                   /session list [n]       — list recent sessions (default: 10)\n  \
                   /session resume [id]    — resume a previous session (opens menu if no id)\n  \
                   /session delete <id>    — delete a session\n  \
                   /session purge          — delete all sessions for this directory",
            accepts_args: true,
            action: SlashAction::Session,
            subcommands: vec![
                ("list [n]", "list recent sessions"),
                ("resume [id]", "resume a session (menu if no id)"),
                ("delete <id>", "delete a session"),
                ("purge", "delete all sessions for this directory"),
            ],
        },
        SlashCommand {
            name: "export",
            description: "Export conversation to file",
            help: "Exports the conversation to a file. Usage: /export [filename]",
            accepts_args: true,
            action: SlashAction::Export,
            subcommands: vec![],
        },
        SlashCommand {
            name: "cd",
            description: "Change working directory",
            help: "Change the working directory. Usage: /cd <path>",
            accepts_args: true,
            action: SlashAction::Cd,
            subcommands: vec![],
        },
        SlashCommand {
            name: "shell",
            description: "Run a shell command directly",
            help: "Execute a shell command without going through the agent. Usage: /shell <command>",
            accepts_args: true,
            action: SlashAction::Shell,
            subcommands: vec![],
        },
        SlashCommand {
            name: "version",
            description: "Show version information",
            help: "Displays the clankers version and build information.",
            accepts_args: false,
            action: SlashAction::Version,
            subcommands: vec![],
        },
        SlashCommand {
            name: "login",
            description: "Authenticate with Anthropic (OAuth)",
            help: "Start the OAuth login flow.\n\n\
                   Usage:\n  \
                   /login                  — generate an auth URL and display it\n  \
                   /login <code#state>     — complete login with code from browser\n  \
                   /login <callback URL>   — complete login with the full callback URL\n  \
                   /login --account <name> — login to a specific account\n\n\
                   See also: /account (list, switch, logout, status)",
            accepts_args: true,
            action: SlashAction::Login,
            subcommands: vec![],
        },
        SlashCommand {
            name: "tools",
            description: "List available tools",
            help: "Lists all tools available to the agent, including built-in tools \
                   and any tools provided by loaded plugins.",
            accepts_args: false,
            action: SlashAction::Tools,
            subcommands: vec![],
        },
        SlashCommand {
            name: "plugin",
            description: "Show loaded plugins",
            help: "Lists all discovered and loaded plugins with their status.\n\n\
                   Usage: /plugin [name]  — show details for a specific plugin",
            accepts_args: true,
            action: SlashAction::Plugin,
            subcommands: vec![],
        },
        SlashCommand {
            name: "worker",
            description: "Spawn or list swarm workers",
            help: "Spawn a named worker in a Zellij pane, or list active workers.\n\n\
                   Usage:\n  \
                   /worker                   — list active workers\n  \
                   /worker <name> <task>      — spawn worker with a task\n  \
                   /worker <name>             — spawn an idle worker\n\n\
                   Requires running inside a Zellij session (clankers --zellij or clankers --swarm).",
            accepts_args: true,
            action: SlashAction::Worker,
            subcommands: vec![],
        },
        SlashCommand {
            name: "share",
            description: "Share this Zellij session remotely",
            help: "Share the current Zellij session over the network via iroh.\n\n\
                   Usage:\n  \
                   /share              — share read-write\n  \
                   /share --read-only  — share read-only\n\n\
                   Requires running inside a Zellij session.",
            accepts_args: true,
            action: SlashAction::Share,
            subcommands: vec![],
        },
        SlashCommand {
            name: "subagents",
            description: "List and manage subagents",
            help: "List running and completed subagents, or manage them.\n\n\
                   Usage:\n  \
                   /subagents             — list all subagents\n  \
                   /subagents kill <id>   — kill a running subagent\n  \
                   /subagents kill all    — kill all running subagents\n  \
                   /subagents remove <id> — remove a subagent entry from the panel\n  \
                   /subagents clear       — remove all completed/failed subagents",
            accepts_args: true,
            action: SlashAction::Subagents,
            subcommands: vec![
                ("kill <id>", "kill a running subagent"),
                ("kill all", "kill all running subagents"),
                ("remove <id>", "remove a subagent entry"),
                ("clear", "remove all completed/failed subagents"),
            ],
        },
        SlashCommand {
            name: "account",
            description: "Switch or list accounts",
            help: "Manage multiple authenticated accounts.\n\n\
                   Usage:\n  \
                   /account                — list all accounts & status\n  \
                   /account switch <name>  — switch active account\n  \
                   /account login [name]   — login to an account (default: active)\n  \
                   /account logout [name]  — logout an account\n  \
                   /account remove <name>  — remove an account\n  \
                   /account list           — list all accounts",
            accepts_args: true,
            action: SlashAction::Account,
            subcommands: vec![
                ("switch <name>", "switch active account"),
                ("login [name]", "login to an account"),
                ("logout [name]", "logout an account"),
                ("remove <name>", "remove an account"),
                ("status [name]", "show account status"),
                ("list", "list all accounts"),
            ],
        },
        SlashCommand {
            name: "todo",
            description: "Manage todo list",
            help: "Track tasks in the right-side panel.\n\n\
                   Usage:\n  \
                   /todo                   — list all items\n  \
                   /todo add <text>        — add a new item\n  \
                   /todo done <id|text>    — mark item as done\n  \
                   /todo wip <id|text>     — mark item as in-progress\n  \
                   /todo remove <id>       — remove an item\n  \
                   /todo clear             — remove all completed items",
            accepts_args: true,
            action: SlashAction::Todo,
            subcommands: vec![
                ("add <text>", "add a new item"),
                ("done <id|text>", "mark item as done"),
                ("wip <id|text>", "mark item as in-progress"),
                ("remove <id>", "remove an item"),
                ("clear", "remove all completed items"),
            ],
        },
        SlashCommand {
            name: "preview",
            description: "Preview markdown rendering (debug)",
            help: "Injects a fake assistant block with sample markdown content.\n\n\
                   Usage:\n  \
                   /preview              — show default markdown sample\n  \
                   /preview <markdown>   — render the provided markdown text",
            accepts_args: true,
            action: SlashAction::Preview,
            subcommands: vec![],
        },
        SlashCommand {
            name: "plan",
            description: "Toggle plan mode (architecture-first)",
            help: "Toggle plan mode on or off. In plan mode, the agent reads and analyzes \
                   the codebase first, proposes an implementation plan, and waits for approval \
                   before making any edits.\n\n\
                   Usage:\n  \
                   /plan        — toggle plan mode\n  \
                   /plan on     — enable plan mode\n  \
                   /plan off    — disable plan mode",
            accepts_args: true,
            action: SlashAction::Plan,
            subcommands: vec![("on", "enable plan mode"), ("off", "disable plan mode")],
        },
        SlashCommand {
            name: "review",
            description: "Start an interactive code review",
            help: "Start a structured code review of recent changes. The agent will \
                   examine the diff, identify issues, and produce a prioritized report.\n\n\
                   Usage:\n  \
                   /review             — review changes vs main/master\n  \
                   /review <base>      — review changes vs a specific base ref\n  \
                   /review staged      — review only staged changes",
            accepts_args: true,
            action: SlashAction::Review,
            subcommands: vec![],
        },
        SlashCommand {
            name: "role",
            description: "Switch or list model roles",
            help: "Manage model roles for different task types.\n\n\
                   Usage:\n  \
                   /role                    — list all role assignments\n  \
                   /role <name>             — switch to a role's model\n  \
                   /role <name> <model>     — set a role's model\n  \
                   /role reset              — clear all role overrides\n\n\
                   Roles: default, smol, slow, plan, commit, review",
            accepts_args: true,
            action: SlashAction::Role,
            subcommands: vec![
                ("<name>", "switch to a role's model"),
                ("<name> <model>", "set a role's model"),
                ("reset", "clear all role overrides"),
            ],
        },
        SlashCommand {
            name: "system",
            description: "View or modify the system prompt",
            help: "View, replace, append to, or reset the system prompt.\n\n\
                   Usage:\n  \
                   /system              — show the current system prompt (truncated)\n  \
                   /system show         — show the full system prompt\n  \
                   /system set <text>   — replace the system prompt entirely\n  \
                   /system append <text>— append text to the system prompt\n  \
                   /system prepend <text>— prepend text to the system prompt\n  \
                   /system reset        — restore the original system prompt\n  \
                   /system file <path>  — load system prompt from a file",
            accepts_args: true,
            action: SlashAction::SystemPrompt,
            subcommands: vec![
                ("show", "show the full system prompt"),
                ("set <text>", "replace the system prompt"),
                ("append <text>", "append to the system prompt"),
                ("prepend <text>", "prepend to the system prompt"),
                ("reset", "restore the original system prompt"),
                ("file <path>", "load system prompt from a file"),
            ],
        },
        SlashCommand {
            name: "editor",
            description: "Open $EDITOR to compose input",
            help: "Opens your $EDITOR (or $VISUAL, falls back to vi) with the current \
                   editor content. When you save and quit, the content is loaded back \
                   into the clankers input. Useful for composing long multi-line prompts.\n\n\
                   Keybindings: Ctrl+O (insert mode), o (normal mode)",
            accepts_args: false,
            action: SlashAction::Editor,
            subcommands: vec![],
        },
        SlashCommand {
            name: "memory",
            description: "Manage cross-session memory",
            help: "View, add, edit, remove, and search persistent memories.\n\n\
                   Usage:\n  \
                   /memory                   — list all memories\n  \
                   /memory add <text>         — add a global memory\n  \
                   /memory add --project <text> — add a project-scoped memory\n  \
                   /memory edit <id> <text>   — replace memory text by ID\n  \
                   /memory remove <id>        — remove a memory by ID\n  \
                   /memory search <query>     — search memories by text/tags\n  \
                   /memory clear              — remove all memories",
            accepts_args: true,
            action: SlashAction::Memory,
            subcommands: vec![
                ("add <text>", "add a global memory"),
                ("add --project <text>", "add a project-scoped memory"),
                ("edit <id> <text>", "replace memory text by ID"),
                ("remove <id>", "remove a memory by ID"),
                ("search <query>", "search memories"),
                ("clear", "remove all memories"),
            ],
        },
        SlashCommand {
            name: "peers",
            description: "Manage swarm peers",
            help: "View and manage P2P swarm peers.\n\n\
                   Usage:\n  \
                   /peers                      — list all peers (switches to peers panel)\n  \
                   /peers add <node-id> <name>  — add a peer to the registry\n  \
                   /peers remove <name-or-id>   — remove a peer\n  \
                   /peers probe [name-or-id]    — probe a peer (or all peers)\n  \
                   /peers discover              — scan LAN via mDNS for new peers\n  \
                   /peers allow <node-id>       — add to allowlist\n  \
                   /peers deny <node-id>        — remove from allowlist\n  \
                   /peers server [on|off]       — start/stop embedded RPC server",
            accepts_args: true,
            action: SlashAction::Peers,
            subcommands: vec![
                ("add <node-id> <name>", "add a peer"),
                ("remove <name-or-id>", "remove a peer"),
                ("probe [name-or-id]", "probe a peer or all peers"),
                ("discover", "scan LAN via mDNS"),
                ("allow <node-id>", "add to allowlist"),
                ("deny <node-id>", "remove from allowlist"),
                ("server [on|off]", "start/stop RPC server"),
            ],
        },
        SlashCommand {
            name: "layout",
            description: "Switch panel layout",
            help: "Usage: /layout <preset>|toggle <panel>\n  \
                   /layout default              — 3-column (todo+files | chat | subagents+peers)\n  \
                   /layout wide                 — wide chat with left sidebar\n  \
                   /layout focused              — chat only (no panels)\n  \
                   /layout right                — all panels on the right\n  \
                   /layout toggle <panel>       — show/hide a panel (todo|files|subagents|peers)",
            accepts_args: true,
            action: SlashAction::Layout,
            subcommands: vec![
                ("default", "3-column layout"),
                ("wide", "wide chat with left sidebar"),
                ("focused", "chat only (no panels)"),
                ("right", "all panels on the right"),
                ("toggle <panel>", "show/hide a panel"),
            ],
        },
        SlashCommand {
            name: "fork",
            description: "Fork conversation to explore alternatives",
            help: "Create a new branch from the current message.\n\n\
                   Usage:\n  \
                   /fork                — fork with auto-generated name\n  \
                   /fork <reason>       — fork with a descriptive name",
            accepts_args: true,
            action: SlashAction::Fork,
            subcommands: vec![],
        },
        SlashCommand {
            name: "rewind",
            description: "Jump back to an earlier message",
            help: "Rewind the conversation to an earlier point.\n\n\
                   Usage:\n  \
                   /rewind <N>            — go back N messages\n  \
                   /rewind <message-id>   — jump to specific message\n  \
                   /rewind <label>        — jump to a labeled message",
            accepts_args: true,
            action: SlashAction::Rewind,
            subcommands: vec![],
        },
        SlashCommand {
            name: "branches",
            description: "List conversation branches",
            help: "List all branches in the current session.\n\n\
                   Usage:\n  \
                   /branches              — list all branches\n  \
                   /branches --verbose    — show detailed branch tree",
            accepts_args: true,
            action: SlashAction::Branches,
            subcommands: vec![],
        },
        SlashCommand {
            name: "switch",
            description: "Switch to a different branch",
            help: "Switch to a different conversation branch.\n\n\
                   Usage:\n  \
                   /switch <branch-name>  — switch by branch name\n  \
                   /switch <message-id>   — switch to specific message",
            accepts_args: true,
            action: SlashAction::Switch,
            subcommands: vec![],
        },
        SlashCommand {
            name: "label",
            description: "Label the current message",
            help: "Add a human-readable label to the current message.\n\n\
                   Usage: /label <name>\n\n\
                   Labels can be used with /rewind and /switch for easy navigation.",
            accepts_args: true,
            action: SlashAction::Label,
            subcommands: vec![],
        },
        SlashCommand {
            name: "quit",
            description: "Quit clankers",
            help: "Exit the application.",
            accepts_args: false,
            action: SlashAction::Quit,
            subcommands: vec![],
        },
    ]
}

/// Parse a slash command from input text.
/// Returns `Some((action, args))` if the text starts with `/` and matches a command.
/// Returns `None` if it's not a slash command or doesn't match.
pub fn parse_command(input: &str) -> Option<(SlashAction, String)> {
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
    if let Some(c) = commands.iter().find(|c| c.name == cmd_name) {
        return Some((c.action.clone(), args));
    }

    // Fall back to user-defined prompt templates: /fix, /test, etc.
    // The template name is the command name without leading slash.
    // Any remaining text after the command becomes the {{input}} variable.
    if !cmd_name.is_empty() && cmd_name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return Some((SlashAction::PromptTemplate(cmd_name.to_string()), args));
    }

    None
}

/// A completion item returned by the autocomplete system.
/// Can represent either a top-level command or a subcommand.
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

/// Get completions for a partial slash command input.
/// The input should include the leading `/`.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_command_basic() {
        let (action, args) = parse_command("/help").unwrap();
        assert_eq!(action, SlashAction::Help);
        assert_eq!(args, "");
    }

    #[test]
    fn test_parse_command_with_args() {
        let (action, args) = parse_command("/model claude-3-5-sonnet").unwrap();
        assert_eq!(action, SlashAction::Model);
        assert_eq!(args, "claude-3-5-sonnet");
    }

    #[test]
    fn test_parse_command_unknown_falls_through_to_prompt_template() {
        // Unknown commands now fall through to the prompt template system
        let result = parse_command("/nonexistent");
        assert!(result.is_some());
        let (action, _args) = result.unwrap();
        assert!(matches!(action, SlashAction::PromptTemplate(ref name) if name == "nonexistent"));
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
        let (action, args) = parse_command("/login").unwrap();
        assert_eq!(action, SlashAction::Login);
        assert_eq!(args, "");
    }

    #[test]
    fn test_parse_login_with_code() {
        let (action, args) = parse_command("/login abc123#state456").unwrap();
        assert_eq!(action, SlashAction::Login);
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
        let (action, args) = parse_command("/worker").unwrap();
        assert_eq!(action, SlashAction::Worker);
        assert_eq!(args, "");
    }

    #[test]
    fn test_parse_worker_with_name_and_task() {
        let (action, args) = parse_command("/worker builder fix the tests").unwrap();
        assert_eq!(action, SlashAction::Worker);
        assert_eq!(args, "builder fix the tests");
    }

    #[test]
    fn test_parse_share() {
        let (action, args) = parse_command("/share").unwrap();
        assert_eq!(action, SlashAction::Share);
        assert_eq!(args, "");
    }

    #[test]
    fn test_parse_share_read_only() {
        let (action, args) = parse_command("/share --read-only").unwrap();
        assert_eq!(action, SlashAction::Share);
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
        let (action, args) = parse_command("/system").unwrap();
        assert_eq!(action, SlashAction::SystemPrompt);
        assert_eq!(args, "");
    }

    #[test]
    fn test_parse_system_show() {
        let (action, args) = parse_command("/system show").unwrap();
        assert_eq!(action, SlashAction::SystemPrompt);
        assert_eq!(args, "show");
    }

    #[test]
    fn test_parse_system_set() {
        let (action, args) = parse_command("/system set You are a helpful assistant.").unwrap();
        assert_eq!(action, SlashAction::SystemPrompt);
        assert_eq!(args, "set You are a helpful assistant.");
    }

    #[test]
    fn test_parse_system_append() {
        let (action, args) = parse_command("/system append Always be concise.").unwrap();
        assert_eq!(action, SlashAction::SystemPrompt);
        assert_eq!(args, "append Always be concise.");
    }

    #[test]
    fn test_parse_system_reset() {
        let (action, args) = parse_command("/system reset").unwrap();
        assert_eq!(action, SlashAction::SystemPrompt);
        assert_eq!(args, "reset");
    }

    #[test]
    fn test_parse_system_file() {
        let (action, args) = parse_command("/system file /tmp/prompt.md").unwrap();
        assert_eq!(action, SlashAction::SystemPrompt);
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
        let (action, args) = parse_command("/editor").unwrap();
        assert_eq!(action, SlashAction::Editor);
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
        let (action, args) = parse_command("/fork").unwrap();
        assert_eq!(action, SlashAction::Fork);
        assert_eq!(args, "");
    }

    #[test]
    fn test_parse_fork_with_args() {
        let (action, args) = parse_command("/fork try different approach").unwrap();
        assert_eq!(action, SlashAction::Fork);
        assert_eq!(args, "try different approach");
    }

    #[test]
    fn test_parse_rewind() {
        let (action, args) = parse_command("/rewind 5").unwrap();
        assert_eq!(action, SlashAction::Rewind);
        assert_eq!(args, "5");
    }

    #[test]
    fn test_parse_branches() {
        let (action, args) = parse_command("/branches").unwrap();
        assert_eq!(action, SlashAction::Branches);
        assert_eq!(args, "");
    }

    #[test]
    fn test_parse_switch() {
        let (action, args) = parse_command("/switch main").unwrap();
        assert_eq!(action, SlashAction::Switch);
        assert_eq!(args, "main");
    }

    #[test]
    fn test_parse_label() {
        let (action, args) = parse_command("/label checkpoint").unwrap();
        assert_eq!(action, SlashAction::Label);
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
}
