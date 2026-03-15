# tui-extraction — Extract TUI into a Separate Crate

## Intent

The TUI lives in `src/tui/` as a module inside the main `clankers` crate. It's
18,650 lines across 55 files — the single largest module in the codebase. This
creates several problems:

- **Compile times**: Any change to the TUI recompiles the entire crate. A
  separate crate gets its own compilation unit, enabling parallel builds.
- **Dependency confusion**: The `App` struct holds fields from 10+ other modules
  (keybindings, slash commands, provider types, cost tracking, plugins, procmon).
  This makes it unclear where TUI ends and application logic begins.
- **Testing friction**: TUI unit tests compile the entire crate even when they
  only test rendering logic. A separate crate with mock traits compiles faster.
- **Reuse impossible**: The TUI can't be used by other frontends (e.g., a
  stripped-down TUI for daemon mode, or a future GUI) because it's entangled
  with agent/session/tool internals.
- **Circular reasoning about changes**: Adding any new feature requires
  understanding whether it belongs in TUI, application logic, or shared types.
  A crate boundary makes this explicit.

The goal is to extract `src/tui/` into `crates/clankers-tui/` with clean trait
boundaries. The TUI crate should depend on shared types but never import from
the main crate. The main crate depends on `clankers-tui` and wires concrete
types into TUI traits.

## Current State

### What's in `src/tui/`

| Category | Files | Lines | Key types |
|----------|-------|-------|-----------|
| Core framework | 6 | 1,676 | `App`, `AppEvent`, `render()` |
| Panel/pane infra | 5 | 1,767 | `Panel` trait, `PaneRegistry`, `PanelManager`, `Theme` |
| Components | 45 | ~15,250 | Panels, overlays, editor, markdown, leader menu, block view |
| **Total** | **55** | **~18,650** | |

### Coupling summary

- **39 of 55 files** (70%) are pure TUI — zero imports from the rest of the crate
- **7 files** concentrate all outbound coupling (87 external refs total)
- **28 external files** import from `src/tui/`, primarily `App`, `SubagentEvent`,
  `PanelId`, `Theme`, and display types
- The `App` struct is the worst offender — 40+ fields, 22 external references,
  mixes view state with application wiring

## Scope

### In Scope

- New `crates/clankers-tui/` workspace crate
- Shared types crate `crates/clankers-tui-types/` for cross-boundary types
  (`SubagentEvent`, `DisplayMessage`, `MessageRole`, etc.)
- Trait boundaries replacing direct imports of agent, provider, tool, plugin,
  config, and slash command types
- Splitting `App` into pure view state (moves to TUI crate) and application
  context (stays in main crate, passed via trait/struct)
- Moving all 55 TUI files into the new crate
- Updating all 28 external files to use the new crate paths
- Maintaining identical runtime behavior — no UX changes

### Out of Scope

- Redesigning the TUI architecture (panel system, BSP tiling, etc.)
- Adding new TUI features
- Changing the component API (Panel trait, draw signatures, etc.)
- Extracting other modules into crates (slash commands, tools, etc.)
- Breaking the public API of the main crate (CLI, config files, etc.)

## Approach

### Strategy: Bottom-up extraction with shared types crate

The extraction follows the existing crate pattern (`clankers-auth`,
`clankers-router`, `clankers-matrix`): each extracted crate is fully
self-contained with no references back to the main crate.

**Three new crates:**

1. **`clankers-tui-types`** — Shared types that flow between TUI and the rest
   of the system. These are currently defined in `src/tui/` but used by tools,
   modes, and slash commands. Moving them to a shared crate breaks the
   circular dependency.

2. **`clankers-tui`** — The TUI itself: rendering, panels, components, event
   handling, editor, theme. Depends on `clankers-tui-types` and defines traits
   for everything it needs from the application layer.

3. **Main crate** — Implements TUI traits with concrete types, constructs the
   `App`, wires events. Depends on both `clankers-tui` and `clankers-tui-types`.

**Key architectural decisions:**

- **Traits for external deps**: Instead of importing `CostTracker`,
  `SlashRegistry`, `PluginUIState`, etc., the TUI crate defines traits like
  `CostProvider`, `CompletionSource`, `PluginRenderer`. The main crate
  implements these traits.

- **App split**: The current `App` struct becomes two parts:
  - `ViewState` (in `clankers-tui`) — all rendering state, scroll positions,
    conversation blocks, panel manager, tiling, theme, editor
  - `AppBridge` trait (in `clankers-tui`) — methods the TUI calls to get
    data from the application (cost info, slash completions, model list, etc.)
  - `AppContext` (in main crate) — implements `AppBridge`, holds `CostTracker`,
    `SlashRegistry`, `ActionRegistry`, `PluginManager`, etc.

- **Event translation**: `AgentEvent` stays in the main crate. The main crate
  translates `AgentEvent` → TUI-native events before forwarding to the TUI.
  This keeps the TUI ignorant of agent internals.

### What doesn't change

- The `Panel` trait, `PaneRegistry`, `PaneKind`, BSP tiling — all stay as-is
- Component rendering logic — identical, just in a different crate
- Key handling flow — same dispatch, just through a trait boundary
- Event loop structure — `EventLoopRunner` moves to TUI crate

### Dependency graph after extraction

```
clankers (main binary)
  ├── clankers-tui          (rendering, panels, components)
  │     └── clankers-tui-types  (shared display types)
  ├── clankers-tui-types    (SubagentEvent, DisplayMessage, etc.)
  ├── clankers-router       (model routing, existing)
  ├── clankers-auth         (UCAN auth, existing)
  ├── clankers-matrix       (Matrix bridge, existing)
  └── clankers-plugin-sdk   (WASM plugin types, existing)
```

No crate has a reverse dependency on the main crate. `clankers-tui-types` is
a leaf crate with minimal deps (just `serde`, `ratatui`).
