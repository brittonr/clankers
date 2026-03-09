# tui-extraction — Design

## Decisions

### Two crates, not one: `clankers-tui-types` + `clankers-tui`

**Choice:** Extract into two crates. `clankers-tui-types` holds cross-boundary
types (display types, events, enums). `clankers-tui` holds the rendering engine,
components, and application state. The main crate depends on both.

**Rationale:** `SubagentEvent` is defined in `src/tui/` but imported by 5 tool
files and 7 mode files that have zero other TUI dependencies. If it stays in
`clankers-tui`, those files gain an unnecessary dep on ratatui, crossterm, and
the entire rendering stack. A thin types crate (no ratatui dep) breaks this
cycle. This mirrors how `clankers-plugin-sdk` separates plugin wire types from
the plugin host.

**Alternatives considered:**
- Single `clankers-tui` crate with re-exports: Tools would depend on
  `clankers-tui` just for `SubagentEvent`. Pulls in 18K lines of TUI code
  and ratatui/crossterm as transitive deps of every tool.
- Move `SubagentEvent` to the main crate: Then the TUI crate would need to
  import from the main crate, creating a circular dependency.
- Move `SubagentEvent` to `clankers-router` or another existing crate: Wrong
  domain — router knows nothing about subagents.

### `clankers-tui-types` contents

**Choice:** The types crate contains exactly these types (currently scattered
across `src/tui/app/mod.rs`, `src/tui/components/subagent_event.rs`,
`src/tui/components/block.rs`, `src/tui/panel.rs`):

```
SubagentEvent           — lifecycle events for subagent processes
DisplayMessage          — a message for display in the chat view
DisplayImage            — an image attached to a display message
MessageRole             — User | Assistant | ToolCall | ToolResult | Thinking | System
AppState                — Idle | Streaming | Command | Dialog
RouterStatus            — Connected | Local | Disconnected
PendingImage            — clipboard image waiting to be sent
ActiveToolExecution     — in-progress tool tracking (name, start time, line count)
BlockEntry              — System(msg) | Conversation(block)
ConversationBlock       — a prompt+response block in conversation
PanelId                 — Todo | Files | Subagents | Peers | Processes | Branches
PanelAction             — Consumed | Unfocus | SlashCommand | FocusPanel | FocusSubagent
TodoStatus              — enum for todo item states
MenuPlacement           — TopLevel | Submenu(char)
MenuContribution        — a single menu item with key, label, action, priority, placement
MenuContributor (trait) — trait for anything that contributes leader menu items
LeaderAction            — SlashCommand | Submenu | KeymapAction
HitRegion               — mouse hit-test results (Chat, Editor, Panel, Subagent, etc.)
```

**Rationale:** These are the types imported by 28+ files outside `src/tui/`.
They're plain data — no rendering logic, no ratatui types, no crossterm.
Moving them to a types crate lets tools, modes, and slash commands depend on
the types without pulling in the TUI rendering stack.

**Dependencies of `clankers-tui-types`:** `serde` (for derive), `chrono`
(for timestamps in `ConversationBlock`). No ratatui, no crossterm.

### Traits for external dependencies, not generics on App

**Choice:** Define concrete traits in `clankers-tui` that the main crate
implements. The TUI stores trait objects (`Box<dyn CostProvider>`, etc.), not
generic type parameters.

```rust
// In clankers-tui
pub trait CostProvider: Send + Sync {
    fn summary(&self) -> Option<CostSummary>;
    fn budget_status(&self) -> Option<BudgetStatus>;
}

pub trait CompletionSource: Send + Sync {
    fn completions(&self, prefix: &str) -> Vec<CompletionItem>;
    fn commands(&self) -> Vec<SlashCommandInfo>;
}

pub trait PluginWidgetHost: Send + Sync {
    fn widgets(&self) -> Vec<WidgetSpec>;
    fn notifications(&self) -> Vec<NotificationSpec>;
    fn status_segments(&self) -> Vec<StatusSegmentSpec>;
}

pub trait ProcessDataSource: Send + Sync {
    fn processes(&self) -> Vec<ProcessInfo>;
    fn process_detail(&self, pid: u32) -> Option<ProcessDetail>;
}
```

**Rationale:** Generic type parameters on `App` would propagate everywhere
(`App<C, S, P, W>` with 4+ type params through every function that touches
App). Trait objects add one vtable indirection per call but keep the API clean.
These methods are called at most once per frame (16ms) — the vtable cost is
immeasurable.

**Alternatives considered:**
- Generic `App<C: CostProvider, S: CompletionSource, ...>`: Type parameter
  explosion. Every function taking `&App` becomes generic. Compile times
  get worse, not better.
- Duplicate types: Copy `CostSummary`, `BudgetStatus`, etc. into the TUI crate.
  Fragile — types drift when the original changes.
- Direct dependency on `clankers-router` / other crates: The TUI shouldn't
  know about routing, auth, or provider internals. Traits are the right
  abstraction level.

### `CostSummary` and `BudgetStatus` as TUI-owned types

**Choice:** Define `CostSummary` and `BudgetStatus` in `clankers-tui-types`
as simple data structs. The `CostProvider` trait returns these TUI-owned types.
The main crate's `CostTracker` converts its internal representation into these
types when implementing the trait.

```rust
// In clankers-tui-types
pub struct CostSummary {
    pub total_cost: f64,
    pub total_tokens: usize,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub model_breakdown: Vec<(String, f64)>,
}

pub enum BudgetStatus {
    NoBudget,
    Ok { remaining: f64, total: f64 },
    Warning { remaining: f64, total: f64 },
    Exceeded { overage: f64, total: f64 },
}
```

**Rationale:** The TUI needs to render cost data. It shouldn't import
`clankers-router` or `model_selection` just for two display structs. The
conversion is trivial (a few field copies) and happens once per frame.

### `InputMode` moves to `clankers-tui-types`

**Choice:** Move the `InputMode` enum (Normal, Insert, Command) to the types
crate. It's used by the TUI (status bar, key dispatch) and by keybindings
config, but it's a simple 3-variant enum with no dependencies.

**Rationale:** `InputMode` is imported by 2 TUI files and 3 config files. It
has no methods beyond `Display`. It's logically a TUI concept (vi-style modal
editing) that config needs to reference. The types crate is the right home.

### `ThinkingLevel` as a simple enum copy

**Choice:** Define `ThinkingLevel` in `clankers-tui-types` as a standalone enum.
The provider crate keeps its own `ThinkingLevel` for API purposes. The main
crate converts between them (they have the same variants).

```rust
// In clankers-tui-types
pub enum ThinkingLevel {
    Off,
    Brief,
    Full,
}
```

**Rationale:** `ThinkingLevel` is 3 variants with no methods. The TUI uses it
for a status bar indicator and a toggle action. Depending on the provider crate
for a 3-variant enum would pull in `reqwest`, `tokio`, `serde_json`, and the
entire HTTP stack as transitive deps of the TUI crate.

### Agent events translated at the boundary, not passed through

**Choice:** The main crate translates `AgentEvent` into TUI-native `TuiEvent`
variants before forwarding. The TUI crate never imports `AgentEvent`.

```rust
// In clankers-tui
pub enum TuiEvent {
    // Streaming
    StreamStart,
    StreamEnd { messages: Vec<DisplayMessage> },
    TextDelta(String),
    ThinkingDelta(String),
    ContentBlockStart,
    ContentBlockStop,

    // Tools
    ToolCall { tool_name: String, call_id: String, input_json: String },
    ToolOutput { call_id: String, text: String },
    ToolDone { call_id: String, result_text: String, is_error: bool },
    ToolProgress { call_id: String, progress: ToolProgressData },

    // Subagents (forwarded from SubagentEvent)
    Subagent(SubagentEvent),

    // Process monitor
    ProcessSpawn { pid: u32, name: String, command: String },
    ProcessUpdate { pid: u32, cpu: f32, rss: u64 },
    ProcessExit { pid: u32, code: Option<i32> },

    // Session
    SessionStart { session_id: String },
    CostUpdate { total_cost: f64, total_tokens: usize },
    ModelChange { from: String, to: String },

    // Terminal input (from crossterm)
    Key(crossterm::event::KeyEvent),
    Paste(String),
    Resize(u16, u16),
    Mouse { col: u16, row: u16, button: Button },
    Scroll { col: u16, row: u16, direction: ScrollDirection },
}
```

**Rationale:** `AgentEvent` references `AgentMessage`, `Content`, `ContentDelta`,
`ToolResult`, `Usage`, `ProcessMeta`, `AssistantMessage`, `ToolResultMessage` —
12+ types from agent, provider, and tools. If the TUI imported `AgentEvent`
directly, it would transitively depend on the provider crate, the tool system,
and the session system. The translation layer (in `agent_events.rs`, which
stays in the main crate) extracts only the display-relevant data.

**Alternatives considered:**
- TUI depends on agent events crate: Would need to extract `AgentEvent` into
  yet another crate, and it still references provider/tool types.
- Feature-gate the provider types: Complex, error-prone conditional compilation.
- Pass `AgentEvent` as `Box<dyn Any>` and downcast: Type-unsafe, hard to debug.

The translation is ~150 lines (the existing `agent_events.rs`), costs nothing
at runtime, and gives the TUI a clean, stable event API that doesn't change
when provider internals are refactored.

### `App` stays as one struct, external wiring via `AppBridge`

**Choice:** Keep `App` as a single struct in `clankers-tui`. Move the 8 fields
that reference external types out of `App` and into an `AppBridge` trait object
that's stored on `App`.

```rust
// In clankers-tui
pub struct App {
    // All pure view state stays here (35+ fields)
    pub state: AppState,
    pub theme: Theme,
    pub editor: Editor,
    // ...

    // External data access via trait
    bridge: Box<dyn AppBridge>,
}

pub trait AppBridge: Send {
    fn cost_provider(&self) -> Option<&dyn CostProvider>;
    fn completion_source(&self) -> &dyn CompletionSource;
    fn plugin_host(&self) -> &dyn PluginWidgetHost;
    fn process_source(&self) -> &dyn ProcessDataSource;
    fn input_mode(&self) -> InputMode;
    fn set_input_mode(&mut self, mode: InputMode);
    fn thinking_level(&self) -> ThinkingLevel;
    fn set_thinking_level(&mut self, level: ThinkingLevel);
    fn clipboard_check(&mut self) -> Option<ClipboardResult>;
    fn plan_state(&self) -> &PlanState;
    fn plan_state_mut(&mut self) -> &mut PlanState;
}
```

**Rationale:** Splitting `App` into `ViewState` + `AppContext` would require
changing every `&mut app` reference in 55 files (render functions, event
handlers, panel methods). A bridge trait keeps the internal code structure
identical — components still call `app.bridge.cost_provider()` instead of
`app.cost_tracker`. The refactoring is localized to field access sites.

**Alternatives considered:**
- Full split into `ViewState` + `AppContext`: 500+ call-site changes, high risk
  of regressions, marginal benefit over trait approach.
- Keep external types on App with re-exports: Defeats the purpose of extraction,
  TUI crate still depends on everything.

### `EventLoopRunner` moves to TUI crate

**Choice:** `EventLoopRunner` (currently in `src/modes/event_loop_runner/`) moves
into `clankers-tui`. It owns the render loop, terminal handle, and event
dispatch — all TUI concerns.

**Rationale:** The runner's `run()` method is the TUI's main loop. It calls
`render()`, polls terminal events, and dispatches key/mouse input. The only
external interaction is receiving `TuiEvent`s from a channel (which the main
crate sends after translating `AgentEvent`s). This is a clean boundary.

The main crate creates the `App`, constructs the `AppBridge` impl, spawns the
agent, and hands everything to `EventLoopRunner::run()`.

### Leader menu builder uses `CompletionSource` trait

**Choice:** The leader menu builder (`builder.rs`, 11 external refs) currently
imports `SlashCommand`, `SlashRegistry`, `Action`, `CoreAction`, `ExtendedAction`,
`Conflict`, and priority constants. Replace with:

1. `CompletionSource` trait provides `commands() -> Vec<SlashCommandInfo>` where
   `SlashCommandInfo` is a plain struct with name, description, leader_key.
2. `Action`/`CoreAction`/`ExtendedAction` move to `clankers-tui-types` — they're
   TUI concepts (keybinding actions) that config references, not the other way
   around.
3. `Conflict` and priority constants move to `clankers-tui-types` — they're
   part of the menu system.

**Rationale:** The leader menu is entirely a TUI feature. The action enums
describe TUI operations (scroll, focus, split pane). The conflict resolution
system is for TUI menu rendering. These belong in the TUI domain. The config
crate can depend on `clankers-tui-types` for `Action`/`ExtendedAction` since
those are lightweight enums.

### `merge_interactive` depends on message types via a trait

**Choice:** `merge_interactive.rs` imports 5 provider/session types
(`AgentMessage`, `Content`, `MessageId`, `UserMessage`, `MessageEntry`). Replace
with a `MergeMessageProvider` trait that returns display-oriented data:

```rust
pub trait MergeMessageProvider {
    fn message_summaries(&self, ids: &[String]) -> Vec<MergeMessageSummary>;
}

pub struct MergeMessageSummary {
    pub id: String,
    pub role: MessageRole,
    pub preview: String,
    pub is_selected: bool,
}
```

**Rationale:** The merge interactive view renders a checkbox list of messages.
It doesn't need the full `AgentMessage` enum (7 variants, provider types) — it
needs a role, a preview string, and a selection state. The main crate implements
the trait by mapping `AgentMessage` → `MergeMessageSummary`.

### `process_panel` depends on process data via `ProcessDataSource`

**Choice:** `process_panel.rs` imports `ProcessMonitorHandle` and `ProcessState`.
Replace with the `ProcessDataSource` trait (defined above). The panel calls
`app.bridge.process_source().processes()` instead of holding a monitor handle.

**Rationale:** The process panel renders a list of processes with CPU/RSS stats.
It doesn't need the monitor's internal state machine — it needs a snapshot of
current processes. The trait returns `Vec<ProcessInfo>` where `ProcessInfo` is
a TUI-owned struct.

### `widget_host` depends on plugin types via `PluginWidgetHost`

**Choice:** `widget_host.rs` imports `Widget`, `PluginUIState`,
`PluginNotification` from the plugin system. Replace with `PluginWidgetHost`
trait that returns TUI-owned types (`WidgetSpec`, `NotificationSpec`,
`StatusSegmentSpec`).

**Rationale:** Same pattern as process panel. The TUI renders plugin widgets
but shouldn't know about WASM manifests, plugin lifecycle, or host functions.

### Slash completions via `CompletionSource` trait

**Choice:** `slash_menu.rs` imports `CompletionItem` from slash commands.
`leader_menu/builder.rs` imports `SlashRegistry`, `SlashContributor`,
`BuiltinSlashContributor`. Replace with `CompletionSource` trait:

```rust
pub trait CompletionSource: Send + Sync {
    fn completions(&self, prefix: &str) -> Vec<CompletionItem>;
    fn commands(&self) -> Vec<SlashCommandInfo>;
}

// These stay in clankers-tui-types (they're display-oriented)
pub struct CompletionItem {
    pub name: String,
    pub description: String,
    pub kind: CompletionKind,
}

pub struct SlashCommandInfo {
    pub name: String,
    pub description: String,
    pub leader_key: Option<LeaderBinding>,
}
```

**Rationale:** The slash menu renders a fuzzy-filtered list of command names.
It doesn't need the `SlashRegistry` (which holds handler functions) — it needs
a list of `(name, description)` pairs. The main crate wraps `SlashRegistry`
to implement `CompletionSource`.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                        clankers (main binary)                       │
│                                                                     │
│  ┌───────────────┐  ┌──────────────┐  ┌──────────────────────────┐ │
│  │ Agent/Session  │  │ Tools/Modes  │  │  AppBridge impl          │ │
│  │                │  │              │  │  (wires concrete types   │ │
│  │  AgentEvent ──────► translate() ──► TuiEvent                  │ │
│  │                │  │              │  │                          │ │
│  │                │  │  uses types ◄───── clankers-tui-types      │ │
│  └────────────────┘  └──────────────┘  └───────────┬──────────────┘ │
│                                                    │                │
│                           constructs App + bridge  │                │
│                           calls runner.run()       │                │
└────────────────────────────────────────────────────┼────────────────┘
                                                     │
                    ┌────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                     clankers-tui (rendering crate)                  │
│                                                                     │
│  ┌─────────────┐  ┌────────────────┐  ┌─────────────────────────┐  │
│  │  App struct  │  │  EventLoop     │  │  Components (45 files)  │  │
│  │  (view state)│  │  Runner        │  │  panels, overlays,      │  │
│  │             │  │                │  │  editor, markdown,      │  │
│  │  bridge: ───────► receives ◄───────  block_view, leader_menu │  │
│  │  Box<dyn    │  │  TuiEvent      │  │  status_bar, etc.       │  │
│  │  AppBridge> │  │  from channel  │  │                         │  │
│  └─────────────┘  └────────────────┘  └─────────────────────────┘  │
│                                                                     │
│  Traits defined here:                                               │
│    AppBridge, CostProvider, CompletionSource,                       │
│    PluginWidgetHost, ProcessDataSource, MergeMessageProvider        │
│                                                                     │
│  Deps: ratatui, ratatui-hypertile, crossterm, clankers-tui-types    │
│  NO dep on: clankers, clankers-router, clankers-auth, etc.          │
└─────────────────────────────────────────────────────────────────────┘
                    │
                    │ depends on
                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                   clankers-tui-types (shared types)                  │
│                                                                     │
│  SubagentEvent, DisplayMessage, DisplayImage, MessageRole,          │
│  AppState, RouterStatus, PendingImage, ActiveToolExecution,         │
│  BlockEntry, ConversationBlock, PanelId, PanelAction, HitRegion,    │
│  TodoStatus, MenuPlacement, MenuContribution, LeaderAction,         │
│  MenuContributor, InputMode, ThinkingLevel, CostSummary,            │
│  BudgetStatus, CompletionItem, SlashCommandInfo, Action,            │
│  CoreAction, ExtendedAction                                         │
│                                                                     │
│  Deps: serde (derive only). NO ratatui, NO crossterm.               │
└─────────────────────────────────────────────────────────────────────┘
```

## Data Flow

### Agent event → TUI rendering

1. Agent emits `AgentEvent::MessageUpdate { delta: ContentDelta::Text(s) }`
2. Main crate's event translator receives it on broadcast channel
3. Translator maps to `TuiEvent::TextDelta(s)` — drops all provider types
4. Sends `TuiEvent` on the TUI's event channel (`mpsc`)
5. `EventLoopRunner` receives `TuiEvent::TextDelta(s)` in its loop
6. Calls `app.handle_tui_event(event)` — updates `streaming.text`
7. Next render frame picks up the new text

### Tool output → streaming panel

1. Tool calls `emit_progress("line of output")`
2. Agent emits `AgentEvent::ToolExecutionUpdate { call_id, partial }`
3. Translator maps to `TuiEvent::ToolOutput { call_id, text }`
4. `App::handle_tui_event` feeds text to `StreamingOutputManager`
5. Streaming output component renders with scroll state

### Slash completion in editor

1. User types `/he` in editor
2. Editor emits prefix to slash menu component
3. Slash menu calls `app.bridge.completion_source().completions("/he")`
4. Bridge impl queries `SlashRegistry::complete("/he")` in main crate
5. Returns `Vec<CompletionItem>` with name + description
6. Slash menu renders filtered list

### Leader menu building

1. On startup, main crate calls `app.rebuild_leader_menu()`
2. `rebuild_leader_menu()` collects `MenuContribution` items from:
   - Builtin keybindings (hardcoded in TUI crate)
   - Plugin manifest entries (via `bridge.plugin_host().menu_items()`)
   - User config (via `bridge.user_menu_items()`)
3. `LeaderMenu::build()` deduplicates by (key, placement), highest priority wins
4. Result stored on `App.overlays.leader_menu`

### Subagent lifecycle

1. Tool spawns subagent, sends `SubagentEvent::Started` to panel channel
2. Main crate receives it, forwards as `TuiEvent::Subagent(event)`
3. TUI routes to both `SubagentPanel` (overview list) and per-pane manager
4. User presses `x` on focused subagent pane → `SubagentEvent::KillRequest`
5. Kill request goes back through the panel channel to the tool layer
6. No TUI→main crate boundary crossed for kill — `SubagentEvent` is a shared type

## File Movement Plan

### Files that move unchanged (39 files, ~14,000 lines)

All component files with zero external imports. These copy directly into
`crates/clankers-tui/src/components/`:

```
account_selector.rs    confirm.rs         environment_panel.rs  image.rs
input.rs               loader.rs          notification.rs       select_list.rs
settings.rs            tool_output.rs     tree_view.rs          header.rs
diff_view.rs           messages.rs        session_selector.rs   scroll.rs
editor/*.rs            markdown.rs        history_search.rs     output_search.rs
branch_switcher.rs     branch_compare.rs  branch_panel.rs       peers_panel.rs
subagent_panel.rs      subagent_pane.rs   todo_panel.rs         file_activity_panel.rs
git_status.rs          context_gauge.rs   block.rs              prelude.rs
model_selector.rs      session_panel.rs   block_view/render.rs
```

### Files that move with import changes (10 files, ~4,000 lines)

These files have external imports that get replaced with trait calls:

```
app/mod.rs             → replace 8 external fields with bridge
app/agent_events.rs    → becomes TuiEvent handler (no AgentEvent)
render.rs              → no external changes needed (all deps are TUI-internal)
status_bar.rs          → use bridge.cost_provider() and bridge.input_mode()
cost_overlay.rs        → use bridge.cost_provider()
leader_menu/builder.rs → use bridge.completion_source() for commands
leader_menu/mod.rs     → Action types come from tui-types
widget_host.rs         → use bridge.plugin_host()
process_panel.rs       → use bridge.process_source()
slash_menu.rs          → use bridge.completion_source()
progress_renderer.rs   → ToolProgress/ProgressKind move to tui-types
merge_interactive.rs   → use bridge.merge_provider()
block_view/mod.rs      → ToolProgress comes from tui-types
```

### Files that move with structural changes (2 files)

```
event.rs               → gains TuiEvent enum (replaces AppEvent + agent events)
panel.rs               → PanelId, PanelAction, MenuContributor move to tui-types
                          Panel trait stays in tui crate
```

### Files that stay in main crate (new or modified)

```
src/bridge.rs          — AppBridge impl (new, ~200 lines)
src/event_translator.rs — AgentEvent → TuiEvent translation (refactored from agent_events.rs)
src/modes/interactive.rs — updated to use clankers-tui API
src/modes/event_handlers.rs — updated imports
src/slash_commands/     — all files update `use crate::tui::` → `use clankers_tui_types::`
src/tools/              — update SubagentEvent import path
```

### Types that move to `clankers-tui-types`

```
src/tui/components/subagent_event.rs  → crates/clankers-tui-types/src/subagent.rs
src/tui/app/mod.rs (types only)       → crates/clankers-tui-types/src/display.rs
src/tui/components/block.rs (types)   → crates/clankers-tui-types/src/block.rs
src/tui/panel.rs (PanelId, PanelAction) → crates/clankers-tui-types/src/panel.rs
src/config/keybindings/actions.rs      → crates/clankers-tui-types/src/actions.rs
src/registry.rs (Conflict, priorities) → crates/clankers-tui-types/src/registry.rs
```
