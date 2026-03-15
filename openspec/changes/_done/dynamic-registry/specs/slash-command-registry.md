# Spec: Slash Command Registry

## Overview

Replace the `SlashAction` enum and 1,831-line match block with a
`SlashRegistry` that maps command names to handler trait objects. Plugins and
user config can register commands at runtime.

## Current Pain

Adding `/fork` required:
1. `SlashAction::Fork` variant in `src/slash_commands/mod.rs`
2. `SlashCommand { name: "fork", ... }` entry in `builtin_commands()`
3. `SlashAction::Fork => { ... }` match arm in `handle_slash_command()` in
   `src/modes/interactive.rs` (1,831-line function)

The match block in `handle_slash_command()` is the single largest function in
the codebase.

## Trait Definition

```rust
// src/slash_commands/mod.rs

/// Handler for a slash command.
pub trait SlashHandler: Send + Sync {
    fn handle(&self, args: &str, ctx: &mut SlashContext) -> SlashResult;
}

/// Context available to handlers.
pub struct SlashContext<'a> {
    pub app: &'a mut App,
    pub cmd_tx: &'a tokio::sync::mpsc::UnboundedSender<AgentCommand>,
    pub plugin_manager: Option<&'a Arc<std::sync::Mutex<PluginManager>>>,
    pub panel_tx: &'a tokio::sync::mpsc::UnboundedSender<SubagentEvent>,
    pub db: &'a Option<Db>,
    pub session_manager: &'a mut Option<SessionManager>,
}

/// Result from a slash command handler.
pub enum SlashResult {
    /// Handled, no further action.
    Ok,
    /// Show a message to the user.
    Message(String),
    /// Send text to the agent as user input.
    SendToAgent(String),
    /// Show an error message.
    Error(String),
}
```

## Registration

```rust
/// Full definition of a registered slash command.
pub struct SlashCommandDef {
    pub name: String,
    pub description: String,
    pub help: String,
    pub accepts_args: bool,
    pub subcommands: Vec<(String, String)>,
    pub handler: Box<dyn SlashHandler>,
    pub priority: u16,
    pub source: String,
    /// Optional leader menu binding (replaces separate leader_key field).
    pub leader_key: Option<LeaderBinding>,
}

/// Contributor trait.
pub trait SlashContributor {
    fn slash_commands(&self) -> Vec<SlashCommandDef>;
}
```

## Registry

```rust
pub struct SlashRegistry {
    commands: IndexMap<String, SlashCommandDef>,
}

impl SlashRegistry {
    pub fn build(contributors: &[&dyn SlashContributor]) -> (Self, Vec<Conflict>) {
        // Collect, deduplicate by name (priority wins), report conflicts.
    }

    pub fn dispatch(&self, name: &str, args: &str, ctx: &mut SlashContext) -> Option<SlashResult> {
        self.commands.get(name).map(|def| def.handler.handle(args, ctx))
    }

    pub fn completions(&self, prefix: &str) -> Vec<&SlashCommandDef> {
        self.commands.values()
            .filter(|d| d.name.starts_with(prefix))
            .collect()
    }

    /// All registered commands (for /help, slash menu, etc.)
    pub fn all(&self) -> impl Iterator<Item = &SlashCommandDef> {
        self.commands.values()
    }
}
```

## Builtin Migration Strategy

Each match arm in `handle_slash_command()` becomes a struct:

```rust
// src/slash_commands/handlers/session.rs
pub struct SessionHandler;

impl SlashHandler for SessionHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext) -> SlashResult {
        // Move the ~80 lines from the SlashAction::Session match arm here
    }
}
```

Group handlers by domain:

```
src/slash_commands/
  mod.rs            — traits, registry, SlashContext, SlashResult
  handlers/
    mod.rs          — builtin_slash_contributor()
    session.rs      — SessionHandler, NewHandler, ResumeHandler
    model.rs        — ModelHandler, RoleHandler, ThinkHandler
    navigation.rs   — CdHandler, ShellHandler
    context.rs      — ClearHandler, ResetHandler, CompactHandler, UndoHandler
    info.rs         — HelpHandler, StatusHandler, UsageHandler, VersionHandler
    tools.rs        — ToolsHandler, PluginHandler
    swarm.rs        — WorkerHandler, ShareHandler, SubagentsHandler, PeersHandler
    tui.rs          — LayoutHandler, PreviewHandler, EditorHandler, TodoHandler
    auth.rs         — LoginHandler, AccountHandler
    memory.rs       — MemoryHandler, SystemPromptHandler
    branching.rs    — ForkHandler, RewindHandler, BranchesHandler, SwitchHandler, LabelHandler
    export.rs       — ExportHandler
```

## Plugin Slash Commands

Plugins already declare `commands: Vec<String>` in `plugin.json`. Currently
these are dead metadata. With the registry:

```rust
impl SlashContributor for PluginManager {
    fn slash_commands(&self) -> Vec<SlashCommandDef> {
        self.loaded_plugins()
            .flat_map(|plugin| {
                plugin.manifest.commands.iter().map(move |cmd_name| {
                    let name = cmd_name.trim_start_matches('/').to_string();
                    SlashCommandDef {
                        name: name.clone(),
                        description: format!("{} plugin command", plugin.manifest.name),
                        help: String::new(),
                        accepts_args: true,
                        subcommands: vec![],
                        handler: Box::new(PluginCommandHandler {
                            plugin_name: plugin.manifest.name.clone(),
                            command: name,
                        }),
                        priority: PRIORITY_PLUGIN,
                        source: plugin.manifest.name.clone(),
                        leader_key: None,  // from plugin's leader_menu field
                    }
                })
            })
            .collect()
    }
}

struct PluginCommandHandler {
    plugin_name: String,
    command: String,
}

impl SlashHandler for PluginCommandHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext) -> SlashResult {
        // Call the plugin's handle_command WASM export
    }
}
```

## Integration with Leader Menu

`SlashCommandDef` includes `leader_key: Option<LeaderBinding>`. The
`SlashRegistry` implements `MenuContributor`:

```rust
impl MenuContributor for SlashRegistry {
    fn menu_items(&self) -> Vec<MenuContribution> {
        self.commands.values()
            .filter_map(|def| {
                let b = def.leader_key.as_ref()?;
                Some(MenuContribution {
                    key: b.key,
                    label: b.label.unwrap_or(&def.description).to_string(),
                    action: LeaderAction::SlashCommand(format!("/{}", def.name)),
                    placement: b.placement.clone(),
                    priority: def.priority,
                    source: def.source.clone(),
                })
            })
            .collect()
    }
}
```

This means the leader menu and slash command system share a single source of
truth. No more manual duplication.

## Invariants

- Command names are unique within the registry (deduplication by priority).
- Command names are case-insensitive (lowercased on registration).
- The `SlashContext` borrow structure must satisfy the borrow checker — the
  `App` mutable ref means handlers can't hold references to app state across
  calls. This matches the current match-arm behavior.
- Plugin command handlers must not block the main thread — WASM calls should
  be bounded in time (existing Extism timeout).
