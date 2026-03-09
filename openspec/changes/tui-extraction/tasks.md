# tui-extraction — Tasks

## Phase 1: Create `clankers-tui-types` crate (shared types)

Move cross-boundary types into a standalone crate with no ratatui dependency.
This phase has zero impact on runtime behavior — it's a re-export refactor.

- [ ] Create `crates/clankers-tui-types/Cargo.toml` with deps: `serde`, `chrono`
- [ ] Create `crates/clankers-tui-types/src/lib.rs` with module declarations
- [ ] Move `SubagentEvent` from `src/tui/components/subagent_event.rs` → `src/subagent.rs`
- [ ] Move display types from `src/tui/app/mod.rs` → `src/display.rs`:
      `DisplayMessage`, `DisplayImage`, `MessageRole`, `AppState`, `RouterStatus`,
      `PendingImage`, `ActiveToolExecution`
- [ ] Move block types from `src/tui/components/block.rs` → `src/block.rs`:
      `BlockEntry`, `ConversationBlock`
- [ ] Move panel types from `src/tui/panel.rs` → `src/panel.rs`:
      `PanelId`, `PanelAction`, `HitRegion`
- [ ] Move menu types: `MenuPlacement`, `MenuContribution`, `MenuContributor` trait,
      `LeaderAction` → `src/menu.rs`
- [ ] Move `TodoStatus` → `src/panel.rs`
- [ ] Move `InputMode` from `src/config/keybindings/` → `src/input.rs`
- [ ] Create `ThinkingLevel` enum in `src/display.rs` (Off | Brief | Full)
- [ ] Create `CostSummary` and `BudgetStatus` structs in `src/cost.rs`
- [ ] Create `CompletionItem` and `SlashCommandInfo` structs in `src/completion.rs`
- [ ] Move `Action`, `CoreAction`, `ExtendedAction` from `src/config/keybindings/actions.rs` → `src/actions.rs`
- [ ] Move `Conflict`, `PRIORITY_BUILTIN`, `PRIORITY_PLUGIN`, `PRIORITY_USER` from `src/registry.rs` → `src/registry.rs`
- [ ] Create `ToolProgressData` struct in `src/progress.rs` (TUI-owned mirror of `ToolProgress`)
- [ ] Add `clankers-tui-types` to workspace `Cargo.toml` members
- [ ] Update main crate `Cargo.toml` to depend on `clankers-tui-types`
- [ ] Add re-exports in main crate: `pub use clankers_tui_types::*` in bridge module
- [ ] `cargo check` passes with zero new warnings
- [ ] All existing tests pass

## Phase 2: Update imports in main crate (re-export bridge)

Replace `use crate::tui::` paths for moved types with `use clankers_tui_types::`.
The TUI module still exists — only the types moved. This is a mechanical
find-and-replace with no logic changes.

- [ ] Update 8 files in `src/tools/` that import `SubagentEvent`:
      change `use crate::tui::components::subagent_event::SubagentEvent`
      → `use clankers_tui_types::SubagentEvent`
- [ ] Update 7 files in `src/slash_commands/` that import display/panel types
- [ ] Update 9 files in `src/modes/` that import `App`, `SubagentEvent`, `PanelId`, etc.
- [ ] Update 2 files in `src/plugin/` that import menu types
- [ ] Update 1 file in `src/config/` that imports `MenuPlacement`
- [ ] Update `src/tui/app/mod.rs` to re-export from `clankers_tui_types`
      (temporary compat — removed in Phase 5)
- [ ] Update `src/tui/panel.rs` to re-export `PanelId`, `PanelAction` from types crate
- [ ] Update `src/tui/components/block.rs` to re-export from types crate
- [ ] Update `src/tui/components/subagent_event.rs` to re-export from types crate
- [ ] `cargo check` passes
- [ ] `cargo test` passes — full suite, no regressions

## Phase 3: Define TUI trait boundaries

Create the trait interfaces that `clankers-tui` will use to access external
data. Implement them in the main crate. No file moves yet — this phase
establishes the interface contract.

### Done

- [x] `CostProvider` trait in clankers-tui-types (`summary`, `budget_status`, `total_cost`)
      Implemented for `CostTracker`; App field changed to `Arc<dyn CostProvider>`
- [x] `CompletionSource` trait in clankers-tui-types (`completions`, `slash_commands`)
      Implemented for `SlashRegistry`; `SlashMenu.update()` uses trait
- [x] Moved `PluginUIState`, `Widget`, `Direction`, `StatusSegment`, `PluginNotification`
      to clankers-tui-types; `PluginUIState.apply()` → free fn `apply_ui_action()`
- [x] Moved `ToolProgress` + builder methods to clankers-tui-types
- [x] Removed dead `action_registry` field from App
- [x] Moved `ThinkingLevel`, `PlanState`, `ClipboardResult` to clankers-tui-types
- [x] Moved `BudgetStatus`, `CostSummary`, `ModelCostBreakdown` to clankers-tui-types
- [x] Switched `SlashCommandContributor` to use `SlashCommandInfo` from tui-types
- [x] Cleaned render.rs, leader_menu imports to use tui-types directly
- [x] External refs in src/tui/: 87 → 15 (83% reduction)
- [x] 57 of 64 TUI files are free of external refs (89%)
- [x] All 1,543 tests pass

### Remaining (deferred to Phase 4+)

- [ ] `ProcessDataSource` trait — process_panel.rs is self-contained, low priority
- [ ] `AppBridge` supertrait — depends on Phase 4 TuiEvent first
- [ ] 7 files still have external refs:
      - `app/mod.rs` (11) — SlashRegistry init plumbing (moves with bridge)
      - `agent_events.rs` (9) — Phase 4 TuiEvent translation
      - `merge_interactive.rs` (5) — Phase 4 message types
      - `process_panel.rs` (2) — ProcessMonitorHandle
      - `peers_panel.rs` (1) — PeerRegistry
      - `markdown.rs` (1) — syntax highlighting util
      - `leader_menu/mod.rs` (1) — builtin_command_infos()

## Phase 4: Create `TuiEvent` and event translation layer

Replace direct `AgentEvent` consumption in the TUI with TUI-native events.
The main crate translates between them.

### Done

- [x] Defined `TuiEvent` enum in `crates/clankers-tui-types/src/events.rs`:
      AgentStart/End, ContentBlockStart/Stop, TextDelta/ThinkingDelta,
      ToolCall/Start/Output/Done/ProgressUpdate/Chunk, UserInput,
      SessionCompaction, UsageUpdate
- [x] Created `src/event_translator.rs` with `translate(&AgentEvent) -> Option<TuiEvent>`
      and `extract_tool_content()` helper
- [x] Rewrote `agent_events.rs`: `handle_agent_event(&AgentEvent)` → `handle_tui_event(&TuiEvent)`
      All 9 external imports eliminated (AgentEvent, Content, ContentDelta,
      ToolResult, ToolResultContent, Usage)
- [x] Updated `event_loop_runner` to call translator before forwarding to TUI
- [x] `agent_events.rs` now has 0 external refs (was 9)
- [x] All 1,543 tests pass, 0 warnings

### Remaining (deferred)

- [ ] `merge_interactive.rs` still uses AgentMessage, Content, MessageId, MessageEntry
      (5 refs) — complex boundary, niche feature, defer to Phase 5+
- [ ] Spawn separate translator task (currently inline in event loop) — not needed
      until TUI becomes a separate process/crate boundary

## Phase 5: Create `clankers-tui` crate and move files

The big move. All 55 TUI files move into the new crate. The main crate's
`src/tui/` module is replaced with a re-export of `clankers-tui`.

- [ ] Create `crates/clankers-tui/Cargo.toml` with deps:
      `ratatui`, `ratatui-hypertile`, `crossterm`, `clankers-tui-types`,
      `syntect`, `pulldown-cmark`, `unicode-width`, `chrono`, `serde`,
      `image` (for sixel), `textwrap`
- [ ] Create `crates/clankers-tui/src/lib.rs` with public module structure
- [ ] Move `src/tui/app/` → `crates/clankers-tui/src/app/`
- [ ] Move `src/tui/components/` → `crates/clankers-tui/src/components/`
- [ ] Move `src/tui/panel.rs` → `crates/clankers-tui/src/panel.rs`
- [ ] Move `src/tui/panes.rs` → `crates/clankers-tui/src/panes.rs`
- [ ] Move `src/tui/render.rs` → `crates/clankers-tui/src/render.rs`
- [ ] Move `src/tui/event.rs` → `crates/clankers-tui/src/event.rs`
- [ ] Move `src/tui/theme.rs` → `crates/clankers-tui/src/theme.rs`
- [ ] Move `src/tui/selection.rs` → `crates/clankers-tui/src/selection.rs`
- [ ] Move `src/tui/widget_host.rs` → `crates/clankers-tui/src/widget_host.rs`
- [ ] Move trait definitions from `src/tui/traits.rs` → `crates/clankers-tui/src/traits.rs`
- [ ] Move event types from `src/tui/events.rs` → `crates/clankers-tui/src/events.rs`
- [ ] Update all `use crate::tui::` → `use crate::` within the new crate
      (internal references become crate-local)
- [ ] Update all `pub(crate)` visibility in moved files to `pub` where needed
      for the main crate to access
- [ ] Add `clankers-tui` to workspace `Cargo.toml` members
- [ ] Update main crate `Cargo.toml` to depend on `clankers-tui`
- [ ] Replace `src/tui/mod.rs` with re-exports:
      ```rust
      pub use clankers_tui::*;
      ```
- [ ] Update `src/modes/interactive.rs` to use `clankers_tui::EventLoopRunner`
- [ ] Update `src/modes/event_loop_runner/` — move to TUI crate or keep as thin
      wrapper calling `clankers_tui::run_event_loop()`
- [ ] `cargo check` passes for both workspace crates
- [ ] `cargo test` passes — full suite

## Phase 6: Move `EventLoopRunner` to TUI crate

The event loop is a TUI concern. Move it into `clankers-tui` and update the
main crate to call into it.

- [ ] Move `src/modes/event_loop_runner/mod.rs` → `crates/clankers-tui/src/runner.rs`
- [ ] Move `src/modes/event_loop_runner/key_handler.rs` → `crates/clankers-tui/src/key_handler.rs`
- [ ] Update runner to receive `TuiEvent` from `mpsc::Receiver` instead of
      subscribing to broadcast channel
- [ ] Define `TuiOutput` enum for actions the TUI needs the main crate to
      perform: `SendPrompt(String)`, `ExecuteSlashCommand(String)`,
      `Quit`, `OpenEditor`, `RequestClipboard`, etc.
- [ ] Runner returns `TuiOutput` actions via a channel — main crate acts on them
- [ ] Update `interactive.rs` to:
      1. Create `AppBridgeImpl`
      2. Create `App` with bridge
      3. Spawn event translator task
      4. Call `clankers_tui::run(app, tui_event_rx, output_tx)`
      5. Handle `TuiOutput` actions in main loop
- [ ] `cargo check` passes
- [ ] `cargo test` passes

## Phase 7: Clean up re-exports and visibility

Remove the compatibility shim in `src/tui/mod.rs`. All external code now
imports from `clankers_tui` or `clankers_tui_types` directly.

- [ ] Remove `src/tui/mod.rs` re-export shim
- [ ] Remove `src/tui/` directory entirely from main crate
- [ ] Update all remaining `use crate::tui::` in main crate →
      `use clankers_tui::` or `use clankers_tui_types::`
- [ ] Audit `pub` visibility in `clankers-tui` — minimize public surface:
      - `App`, `EventLoopRunner`, `Theme`, `Panel` trait: pub
      - Component internals: `pub(crate)` where possible
      - Render functions: `pub(crate)` (only called by runner)
- [ ] Remove stale re-exports from `src/lib.rs`
- [ ] `cargo check` passes with zero warnings about unused imports
- [ ] `cargo test` passes — full suite
- [ ] `cargo clippy` clean

## Phase 8: Move keybinding dispatch to TUI crate

The keybinding config module has types that belong in both crates. Separate
them cleanly.

- [ ] `Action`, `CoreAction`, `ExtendedAction` already in `clankers-tui-types` (Phase 1)
- [ ] `ActionRegistry` (key→action mapping) moves to `clankers-tui`:
      it's consumed by key_handler, which is now in the TUI crate
- [ ] `InputMode` already in `clankers-tui-types` (Phase 1)
- [ ] Keybinding parser (`parser.rs`) stays in main crate — it reads config
      files and constructs `ActionRegistry`
- [ ] Keybinding defaults (`defaults.rs`) moves to `clankers-tui` — it defines
      the default keymap
- [ ] Main crate passes constructed `ActionRegistry` into `App` via `AppBridge`
- [ ] `cargo check` passes
- [ ] `cargo test` passes

## Phase 9: Verify parallel compilation benefit

Confirm the extraction delivers on its promise of faster builds.

- [ ] Measure `cargo build --timings` before and after extraction
- [ ] Verify `clankers-tui` and `clankers-tui-types` compile in parallel with
      unrelated crates (`clankers-router`, `clankers-auth`, etc.)
- [ ] Verify incremental rebuild time for TUI-only changes
- [ ] Verify incremental rebuild time for non-TUI changes (should not recompile TUI)
- [ ] Document build time comparison in commit message

## Phase 10: Documentation and cleanup

- [ ] Add `crates/clankers-tui/README.md` with crate overview, architecture,
      trait boundary documentation
- [ ] Add `crates/clankers-tui-types/README.md` listing all shared types
- [ ] Add module-level doc comments to `lib.rs` for both crates
- [ ] Update main `README.md` workspace section if it lists crates
- [ ] Update `Cargo.toml` workspace members list
- [ ] Commit with message summarizing the extraction: files moved, traits
      introduced, compile time impact
