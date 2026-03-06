# Design: Dynamic Registry Pattern

## Shared Architecture

Every subsystem follows the same shape:

```
┌──────────┐   ┌──────────┐   ┌──────────┐
│ Builtins │   │ Plugins  │   │  User    │
│ (compile)│   │ (runtime)│   │  Config  │
└────┬─────┘   └────┬─────┘   └────┬─────┘
     │              │              │
     │ impl Trait   │ impl Trait   │ impl Trait
     │              │              │
     ▼              ▼              ▼
  ┌──────────────────────────────────────┐
  │    Registry::build(contributors)     │
  │                                      │
  │  collect → deduplicate → resolve     │
  │  conflicts → produce runtime struct  │
  └──────────────────┬───────────────────┘
                     │
                     ▼
              ┌──────────────┐
              │   Runtime    │
              │   Registry   │
              └──────────────┘
```

### Priority Convention

All subsystems use the same priority scale:

```rust
pub const PRIORITY_BUILTIN: u16 = 0;
pub const PRIORITY_PLUGIN: u16 = 100;
pub const PRIORITY_USER: u16 = 200;
```

These constants live in a shared `src/registry.rs` module.

### Shared `src/registry.rs`

```rust
//! Common types for the dynamic registry pattern.

/// Priority constants for conflict resolution across all registries.
pub const PRIORITY_BUILTIN: u16 = 0;
pub const PRIORITY_PLUGIN: u16 = 100;
pub const PRIORITY_USER: u16 = 200;

/// A conflict detected during registry build.
#[derive(Debug, Clone)]
pub struct Conflict {
    pub registry: &'static str,   // "leader_menu", "slash_command", etc.
    pub key: String,              // what conflicted (key char, command name, etc.)
    pub winner: String,           // source that won
    pub loser: String,            // source that lost
}
```

## Per-Subsystem Design

### 1. Leader Menu (`MenuContributor`)

See `../leader-menu-auto/design.md` for full detail.

**Trait:** `MenuContributor { fn menu_items(&self) -> Vec<MenuContribution> }`

**Builder:** `LeaderMenu::build(contributors, hidden) -> (LeaderMenu, Vec<Conflict>)`

**Rebuild trigger:** after plugin load/unload, at init.

### 2. Slash Commands (`SlashCommandHandler`)

**Current state:** 37-variant `SlashAction` enum, 1,831-line match block in
`handle_slash_command()`, `builtin_commands()` returns `Vec<SlashCommand>` with
static metadata.

**Target state:** Commands register a handler that receives the parsed input
and returns a result. The giant match block becomes a HashMap lookup.

```rust
/// A registered slash command with its handler.
pub struct SlashCommandDef {
    pub name: String,
    pub description: String,
    pub help: String,
    pub accepts_args: bool,
    pub subcommands: Vec<(String, String)>,
    pub handler: Box<dyn SlashHandler>,
    pub priority: u16,
    pub source: String,
}

/// Handler for a slash command.
pub trait SlashHandler: Send + Sync {
    /// Execute the command. Returns a SlashResult indicating what to do.
    fn handle(&self, args: &str, ctx: &mut SlashContext) -> SlashResult;
}

/// Context passed to slash command handlers.
pub struct SlashContext<'a> {
    pub app: &'a mut App,
    pub cmd_tx: &'a UnboundedSender<AgentCommand>,
    pub plugin_manager: Option<&'a Arc<Mutex<PluginManager>>>,
    pub panel_tx: &'a UnboundedSender<SubagentEvent>,
    pub db: &'a Option<Db>,
    pub session_manager: &'a mut Option<SessionManager>,
}

/// What the handler wants to happen after execution.
pub enum SlashResult {
    /// Command handled, nothing else to do.
    Ok,
    /// Display a message to the user.
    Message(String),
    /// Send input to the agent as if the user typed it.
    SendToAgent(String),
    /// Error message.
    Error(String),
}
```

**Registry:**

```rust
pub struct SlashRegistry {
    commands: HashMap<String, SlashCommandDef>,
}

impl SlashRegistry {
    pub fn build(contributors: &[&dyn SlashContributor]) -> (Self, Vec<Conflict>) { ... }
    pub fn dispatch(&self, name: &str, args: &str, ctx: &mut SlashContext) -> SlashResult { ... }
    pub fn completions(&self, prefix: &str) -> Vec<&SlashCommandDef> { ... }
}
```

**Migration:** Each match arm in `handle_slash_command()` becomes a struct
implementing `SlashHandler`. Builtins are registered via
`builtin_slash_contributor()`. Plugins register via their manifest `commands`
field + a WASM call bridge.

**File changes:**

| File | Change |
|------|--------|
| `src/slash_commands/mod.rs` | Add `SlashHandler` trait, `SlashRegistry`, break out handler structs |
| `src/slash_commands/handlers/` | New directory, one file per handler group |
| `src/modes/interactive.rs` | Replace 1,831-line match with `registry.dispatch()` |
| `src/plugin/mod.rs` | Implement `SlashContributor` for `PluginManager` |

### 3. Panel Registry

**Current state:** `PanelId` enum (6 variants) + `PanelTab` enum (5 variants,
doesn't match). `App` has named fields per panel. `interactive.rs` has a
250-line nested match for panel-focused key dispatch. `Panel` trait exists but
is bypassed.

**Target state:** Panels register dynamically. `App` holds a panel map. The
`Panel` trait's `handle_key_event()` is the dispatch mechanism.

```rust
/// A unique panel identifier (string, not enum).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PanelId(pub String);

impl PanelId {
    pub const TODO: &str = "todo";
    pub const FILES: &str = "files";
    pub const SUBAGENTS: &str = "subagents";
    pub const PEERS: &str = "peers";
    pub const PROCESSES: &str = "processes";
}

/// Extended Panel trait — adds metadata for registration.
pub trait Panel: Send {
    fn id(&self) -> &str;
    fn label(&self) -> &str;
    /// Which column this panel prefers (left or right).
    fn default_column(&self) -> PanelColumn;
    fn handle_key_event(&mut self, key: &KeyEvent) -> PanelAction;
    fn render(&self, frame: &mut Frame, area: Rect);
}

pub enum PanelColumn { Left, Right }

pub enum PanelAction {
    Consumed,
    NotHandled,
    /// Request an app-level action (e.g. send agent command).
    AppAction(Action),
}
```

**In `App`:**

```rust
pub struct App {
    // Replace individual panel fields:
    pub panels: IndexMap<String, Box<dyn Panel>>,
    pub active_panel: Option<String>,
    // ...
}
```

**Migration:** Delete `PanelTab` enum entirely. Convert `PanelId` enum to
string newtype. Move panel-cycling logic out of `handle_action()` into a
generic `next_panel()` / `prev_panel()` on the `App` or panel manager.

### 4. Tool Collision List (trivial)

**Current:** manually maintained `builtin_names` HashSet in `build_plugin_tools()`.

**Target:** derive from the actual tool vec.

```rust
// Before (manual, out of sync):
let builtin_names: HashSet<&str> = [
    "read", "write", "edit", "bash", ...
].into_iter().collect();

// After (derived, always correct):
let builtin_names: HashSet<&str> = tools.iter().map(|t| t.name()).collect();
```

No trait needed. Just fix the one call site.

### 5. Model Roles

**Current:** 6-variant `ModelRole` enum with `parse()`, `all()`,
`description()`, `infer_role_for_task()`.

**Target:** String-keyed map with defaults.

```rust
pub struct ModelRoleDef {
    pub name: String,
    pub description: String,
    pub model: Option<String>,
    /// Keywords for auto-inference.
    pub keywords: Vec<String>,
}

pub struct ModelRoles {
    roles: IndexMap<String, ModelRoleDef>,
}

impl ModelRoles {
    pub fn with_defaults() -> Self { ... }  // seeds builtin 6
    pub fn add(&mut self, role: ModelRoleDef) { ... }
    pub fn get(&self, name: &str) -> Option<&ModelRoleDef> { ... }
    pub fn infer(&self, task: &str) -> &str { ... }
}
```

User config adds roles in settings:

```toml
[[model_roles]]
name = "debug"
description = "Debugging and tracing"
model = "claude-sonnet-4-5-20250514"
keywords = ["debug", "trace", "backtrace", "panic"]
```

### 6. Keybinding Actions (split)

**Current:** 52-variant `Action` enum. Stable core actions (scroll, mode
switch, cursor movement) mixed with feature-specific actions
(`ToggleBlockIds`, `ToggleSessionPopup`).

**Target:** Split into two layers:

```rust
/// Core actions — stable, hardcoded, exhaustive match is fine.
pub enum CoreAction {
    ScrollUp, ScrollDown, PageUp, PageDown,
    MoveLeft, MoveRight, MoveToStart, MoveToEnd,
    InsertMode, NormalMode, SubmitInput,
    Cancel, Quit, Yank, Paste,
    // ~20 stable actions
}

/// Extended actions — string-keyed, extensible by plugins/config.
/// Feature-specific actions register here instead of adding enum variants.
pub struct ExtendedAction {
    pub name: String,
    pub handler: Box<dyn Fn(&mut App) + Send>,
}

/// Unified action type.
pub enum Action {
    Core(CoreAction),
    Extended(String),  // looked up in ExtendedActionRegistry
}
```

`parse_action()` tries `CoreAction` first (exhaustive match), then falls back
to extended action lookup. Plugins can register extended actions. Feature code
registers its own actions at init instead of polluting the core enum.

## Dependency Graph

The phases have minimal dependencies but should ship in order:

```
Phase 1: Leader Menu ─────────────────────────────────┐
    (proves the pattern, shared priority constants)    │
                                                       │
Phase 2: Slash Commands ──────────────────────────┐    │
    (highest LOC reduction, unblocks plugin cmds)  │    │
                                                   │    │
Phase 3: Panel Registry ──────────────────────┐    │    │
    (eliminates dual-enum, uses Panel trait)   │    │    │
                                               │    │    │
Phase 4: Tool Collision ──── (trivial, any time)    │    │
                                               │    │    │
Phase 5: Model Roles ─────── (independent)     │    │    │
                                               │    │    │
Phase 6: Keybinding Actions ──────────────────────────┘
    (depends on slash + panel being stable first)
```

Phase 4 can ship at any point — it's a one-line fix.
