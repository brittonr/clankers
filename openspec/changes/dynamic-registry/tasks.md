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

## Phase 2: Slash Command Handler Extraction ✅

_Highest pain/payoff. Eliminates 1,831-line match block._

- [x] **2.1** Define `SlashContext` in `src/slash_commands/handlers/mod.rs`
- [x] **2.2** Define `SlashHandler` trait
- [x] **2.3** Create `dispatch()` routing `SlashAction` → handler structs
- [x] **2.4** Extract all 38 match arms into handler files (13 files):
  - [x] `info.rs` — Help, Status, Usage, Version, Quit
  - [x] `context.rs` — Clear, Reset, Compact, Undo
  - [x] `model.rs` — Model, Think, Role
  - [x] `navigation.rs` — Cd, Shell
  - [x] `export.rs` — Export
  - [x] `auth.rs` — Login, Account
  - [x] `tools.rs` — Tools, Plugin
  - [x] `swarm.rs` — Worker, Share, Subagents, Peers
  - [x] `tui.rs` — Layout, Preview, Editor, Todo, Plan, Review
  - [x] `memory.rs` — SystemPrompt, Memory
  - [x] `branching.rs` — Fork, Rewind, Branches, Switch, Label
  - [x] `session.rs` — Session
  - [x] `prompt_template.rs` — PromptTemplate
- [x] **2.5** `execute_slash_command()` → thin 10-line wrapper calling
      `dispatch()`
- [x] **2.6** Helper functions made `pub(crate)`: `resume_session_from_file`,
      `parse_oauth_input`, `parse_account_flag`, `format_time_ago`,
      `strip_frontmatter`, `probe_peer_background`,
      `discover_peers_background`

### Phase 2 remaining (future):
- [ ] `SlashCommandDef`, `SlashContributor` trait, `SlashRegistry`
- [ ] Plugin command registration via manifest
- [ ] Delete `SlashAction` enum (replace with string-keyed registry lookup)
- [ ] Slash menu autocomplete from registry

## Phase 3: Panel Focus Consolidation ✅

_Eliminates dual-enum and 250-line nested match._

- [x] **3.1** Route panel key events through `Panel::handle_key_event()`
      instead of duplicate dispatch in `interactive.rs`
- [x] **3.2** Replace `panel_focused`/`panel_tab`/`right_panel_tab` with
      `FocusTracker` (already existed, now sole source of truth)
- [x] **3.3** Delete `PanelTab` enum (5 variants)
- [x] **3.4** Delete `sync_focus_from_legacy()` bridge
- [x] **3.5** Delete `panel_id_to_tab()` converter
- [x] **3.6** Use `FocusTracker.cycle_in_column()` for Tab cycling
- [x] **3.7** Use `FocusTracker.focus_side()` for h/l column navigation
- [x] **3.8** Add `App::close_focused_panel_views()` for clean unfocus
- [x] **3.9** Side-effect keys (subagent kill, peer probe) remain as
      explicit pre-dispatch matches

### Phase 3 remaining (future):
- [ ] Move panel ownership from App fields into `PanelManager`
- [ ] Typed accessors (`app.todo_panel()` → `app.panels.get::<TodoPanel>()`)
- [ ] Convert `PanelId` enum to string-based (for plugin panels)
- [ ] `MenuContributor` impl for `PanelManager` (auto-generate layout toggles)
- [ ] Delete `PanelId::Environment` (unused placeholder)

## Phase 4: Tool Collision List ✅

- [x] **4.1** `build_plugin_tools()` derives `builtin_names` from actual
      tool list instead of hardcoded 18-entry array

## Phase 5: Model Roles ✅

- [x] **5.1** Replace `ModelRole` enum (6 variants) with `ModelRoles` struct
      (string-keyed `IndexMap<String, ModelRoleDef>`)
- [x] **5.2** `ModelRoles::with_defaults()` seeds 6 builtins with keywords
- [x] **5.3** `merge()` for user-defined roles (add new, override existing)
- [x] **5.4** `resolve()` fallback chain: role model → default model → fallback
- [x] **5.5** `infer()` matches keywords from all roles including user-defined
- [x] **5.6** Aliases preserved in `get()` (fast→smol, large→slow, etc.)
- [x] **5.7** `/role` command dynamically lists available roles
- [x] **5.8** `ModelRolesConfig` → `ModelRoles` in settings.rs
- [x] **5.9** Tests: defaults, aliases, resolve fallback, merge, infer,
      user-defined role inference, reset, names

## Phase 6: Keybinding Actions (deferred)

_Lowest priority. 566 references to `Action::` across 16 files. The full
`CoreAction`/`ExtendedAction` split is high churn for low payoff until
plugin actions are needed._

- [ ] **6.1** Define `CoreAction` enum (~20 stable variants)
- [ ] **6.2** Define `ExtendedActionDef` struct with handler closure
- [ ] **6.3** Define `ActionRegistry` with `register()` and `dispatch()`
- [ ] **6.4** Define unified `Action` enum as `Core(CoreAction) | Extended(String)`
- [ ] **6.5** Migrate `parse_action()`: core match + extended fallback
- [ ] **6.6** Register builtin extended actions at init
- [ ] **6.7** Update keybinding presets
- [ ] **6.8** Plugin action registration via manifest
- [ ] **6.9** Tests
