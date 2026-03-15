# Tasks: Dynamic Leader Menu Registration

## Phase 1: Core Trait and Builder

- [x] **1.1** Define `MenuContributor` trait, `MenuContribution`, `MenuPlacement`,
      priority constants in `src/tui/components/leader_menu.rs`
- [x] **1.2** Implement `LeaderMenu::build(contributors, hidden)` that collects,
      deduplicates, resolves conflicts, and builds the menu tree
- [x] **1.3** Add `Conflict` struct (in `src/registry.rs`) and return conflicts from `build()`
- [x] **1.4** Unit tests: conflict resolution, auto-submenu creation, hidden
      entries, empty contributors, same-key-different-placement (16 tests)

## Phase 2: Builtin Migration

- [x] **2.1** Add `leader_key: Option<LeaderBinding>` field to `SlashCommand`
- [x] **2.2** Populate `leader_key` on relevant builtin slash commands (`/help` → `?`,
      `/compact` → `C`, `/fork` → `f` in session submenu). Session/layout submenu
      items remain in `BuiltinKeymapContributor` since they map to subcommands
      (e.g. `/session resume`), not standalone commands.
- [x] **2.3** Write `slash_command_contributions()` adapter function +
      `SlashCommandContributor` wrapper implementing `MenuContributor`
- [x] **2.4** Write `BuiltinKeymapContributor` for non-slash items (model
      selector, thinking toggle, etc.), submenu openers, and subcommand-based
      items (session submenu, layout submenu, pane submenu)
- [x] **2.5** Replace `LeaderMenu::new()` call in `App::new()` with
      `LeaderMenu::build()` using builtin contributors (via `rebuild_leader_menu()`)
- [x] **2.6** Verify: default menu is identical to the old hardcoded menu
      (test `default_menu_has_expected_structure`)

## Phase 3: Plugin Integration

- [x] **3.1** Add `leader_menu: Vec<PluginLeaderEntry>` to `PluginManifest`
- [x] **3.2** Add `PluginLeaderEntry` struct with serde derives
- [x] **3.3** Implement `MenuContributor` for `PluginManager`
- [x] **3.4** Rebuild leader menu after plugin load/unload — `rebuild_leader_menu()`
      wired at startup in `interactive.rs:80`. Runtime hot-reload deferred until
      plugins support dynamic load/unload in the TUI.
- [x] **3.5** Validation: warn on bad entries (non-ASCII key, empty label,
      missing `/` prefix) in `PluginManager::menu_items()`
- [x] **3.6** Test: plugin `MenuContributor` implementation tested via
      `build()` unit tests with mock contributors

## Phase 4: User Config

- [x] **4.1** Add `LeaderMenuConfig` to settings struct (`src/config/settings.rs`)
- [x] **4.2** Implement `MenuContributor` for `LeaderMenuConfig`
- [x] **4.3** Convert `hide` list to `HashSet<(char, MenuPlacement)>` via
      `hidden_set()` and pass to `build()`
- [x] **4.4** Wire up in `interactive.rs` — user config contributor passed to
      `build()` alongside builtins and plugins via `rebuild_leader_menu()`
- [x] **4.5** Test: user override and hidden entries tested via `build()` unit
      tests (`user_overrides_everything`, `hidden_entries_excluded`)

## Phase 5: Polish

- [x] **5.1** Log `Conflict` diagnostics via `tracing::debug!` in
      `rebuild_leader_menu()` and `rebuild_slash_registry()`
- [x] **5.2** Add `/leader` slash command to dump current menu structure
      (debugging aid) — handler in `info.rs`, shows root items + submenus
- [x] **5.3** Document `leader_menu` config in `/leader` help text
- [x] **5.4** Update plugin SDK README with `leader_menu` manifest field
      and usage examples
