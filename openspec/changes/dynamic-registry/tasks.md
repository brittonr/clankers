# Tasks: Dynamic Registry Pattern

## Phase 1: Leader Menu (`MenuContributor`) ✅

_Proves the pattern. Smallest scope, safest starting point._

- [x] **1.1** Create `src/registry.rs` with shared priority constants and
      `Conflict` struct
- [x] **1.2** Define `MenuContributor` trait, `MenuContribution`,
      `MenuPlacement` in `src/tui/components/leader_menu.rs`
- [x] **1.3** Implement `LeaderMenu::build(contributors, hidden)` builder
- [x] **1.4** Add `leader_key: Option<LeaderBinding>` to `SlashCommand`
- [ ] **1.5** Populate `leader_key` on relevant builtin slash commands
      _(field added, all set to `None` — bindings to be populated when
      slash command registry lands in Phase 2)_
- [x] **1.6** Write `BuiltinKeymapContributor` for non-slash items
      (model selector, thinking toggle, submenu openers, etc.)
- [x] **1.7** Replace `LeaderMenu::new()` with `LeaderMenu::build()` in
      `App::new()` (via `rebuild_leader_menu()`)
- [x] **1.8** Add `leader_menu: Vec<PluginLeaderEntry>` to `PluginManifest`
- [x] **1.9** Implement `MenuContributor` for `PluginManager` (with validation)
- [x] **1.10** Add `LeaderMenuConfig` to settings, implement `MenuContributor`
      (items, hide rules, hidden_set())
- [x] **1.11** Rebuild leader menu at init with all contributors; plugin
      loading is startup-only so no dynamic rebuild needed yet
- [x] **1.12** Unit tests: conflict resolution, auto-submenu, hidden entries,
      empty contributors, priority ordering, same-key-different-placement
- [x] **1.13** Verify: default menu identical to old hardcoded menu
      (19 tests pass including `default_menu_has_expected_structure`)

## Phase 2: Slash Commands (`SlashCommandHandler`)

_Highest pain/payoff. Eliminates 1,831-line match block. Unlocks plugin
commands._

- [x] **2.1** Define `SlashContext` in `src/slash_commands/handlers/mod.rs`
- [ ] **2.2** Define `SlashCommandDef`, `SlashContributor` trait,
      `SlashRegistry`
- [x] **2.3** Create `src/slash_commands/handlers/` directory, `dispatch()`
      function routing through `execute_slash_command`
- [ ] **2.4** Extract handler structs from match arms, grouped by domain:
  - [ ] **2.4a** `session.rs` — Session, New, Resume
  - [ ] **2.4b** `model.rs` — Model, Role, Think
  - [ ] **2.4c** `navigation.rs` — Cd, Shell
  - [ ] **2.4d** `context.rs` — Clear, Reset, Compact, Undo
  - [ ] **2.4e** `info.rs` — Help, Status, Usage, Version
  - [ ] **2.4f** `tools.rs` — Tools, Plugin
  - [ ] **2.4g** `swarm.rs` — Worker, Share, Subagents, Peers
  - [ ] **2.4h** `tui.rs` — Layout, Preview, Editor, Todo
  - [ ] **2.4i** `auth.rs` — Login, Account
  - [ ] **2.4j** `memory.rs` — Memory, SystemPrompt
  - [ ] **2.4k** `branching.rs` — Fork, Rewind, Branches, Switch, Label
  - [ ] **2.4l** `export.rs` — Export
- [ ] **2.5** Implement `builtin_slash_contributor()` returning all handlers
- [ ] **2.6** Replace `handle_slash_command()` match block with
      `registry.dispatch()`
- [ ] **2.7** Implement `SlashContributor` for `PluginManager`
- [ ] **2.8** Wire `SlashRegistry` as `MenuContributor` (unifies leader menu
      and slash command sources of truth)
- [ ] **2.9** Delete `SlashAction` enum
- [ ] **2.10** Update slash menu autocomplete to use `registry.completions()`
- [ ] **2.11** Tests: builtin dispatch, plugin command dispatch, completion,
      conflict resolution

## Phase 3: Panel Registry

_Eliminates dual-enum problem and 250-line nested match._

- [ ] **3.1** Extend `Panel` trait with `id()`, `label()`, `default_column()`,
      `handle_key_event()` returning `PanelKeyResult`
- [ ] **3.2** Create `PanelManager` struct with `register()`, `toggle()`,
      `focus_next()`, `focus_prev()`, `handle_key()`
- [ ] **3.3** Add `PanelManager` to `App` alongside existing panel fields
- [ ] **3.4** Migrate panels one at a time to `PanelManager`:
  - [ ] **3.4a** `ProcessPanel` (least coupled, good first candidate)
  - [ ] **3.4b** `PeersPanel`
  - [ ] **3.4c** `SubagentPanel`
  - [ ] **3.4d** `TodoPanel`
  - [ ] **3.4e** `FileActivityPanel`
- [ ] **3.5** Add typed accessors to `App` (`todo_panel()`,
      `todo_panel_mut()`, etc.)
- [ ] **3.6** Replace panel-focused match block in `handle_action()` with
      `panels.handle_key()`
- [ ] **3.7** Delete `PanelTab` enum entirely
- [ ] **3.8** Convert `PanelId` enum to `PanelId(String)` newtype with
      `const` well-known IDs
- [ ] **3.9** Update layout presets to use string-based panel references
- [ ] **3.10** Implement `MenuContributor` for `PanelManager` (auto-generates
      layout toggle entries)
- [ ] **3.11** Tests: panel registration, focus cycling, toggle, key dispatch

## Phase 4: Tool Collision List ✅

_Trivial fix. No trait needed._

- [x] **4.1** Change `build_plugin_tools()` signature to accept
      `&[Arc<dyn Tool>]`
- [x] **4.2** Derive `builtin_names` from `tools.iter().map(|t| t.definition().name)`
- [x] **4.3** Delete hardcoded `builtin_names` array
- [x] **4.4** Update call site in `common.rs` and 4 test sites

## Phase 5: Model Roles

_Small scope, user-facing improvement._

- [ ] **5.1** Create `ModelRoleDef` struct with name, description, model,
      keywords
- [ ] **5.2** Create `ModelRoles` struct with `with_defaults()`,
      `merge_user_roles()`, `get()`, `all()`, `infer()`
- [ ] **5.3** Add `[[model_roles]]` section to settings config
- [ ] **5.4** Replace `ModelRole` enum with `ModelRoles` struct throughout
- [ ] **5.5** Update `/role` command to list roles dynamically
- [ ] **5.6** Delete `ModelRole` enum, `ModelRole::parse()`,
      `ModelRole::all()`, `ModelRole::description()`
- [ ] **5.7** Tests: default roles, user override, user-defined role, inference

## Phase 6: Keybinding Actions

_Lowest priority. Ship after phases 2 and 3 are stable._

- [ ] **6.1** Define `CoreAction` enum (~20 stable variants)
- [ ] **6.2** Define `ExtendedActionDef` struct with handler closure
- [ ] **6.3** Define `ActionRegistry` with `register()` and `dispatch()`
- [ ] **6.4** Define unified `Action` enum as `Core(CoreAction) | Extended(String)`
- [ ] **6.5** Migrate `parse_action()`: core match + extended fallback
- [ ] **6.6** Register builtin extended actions at init (leader menu, thinking
      toggle, panel focus, model selector, etc.)
- [ ] **6.7** Update keybinding presets (helix_normal, vim_normal, etc.)
- [ ] **6.8** Add plugin action registration via manifest `actions` field
- [ ] **6.9** Delete old `Action` enum variants that moved to extended
- [ ] **6.10** Tests: core action parse, extended action dispatch, unknown
      action handling
