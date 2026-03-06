# Tasks: Dynamic Leader Menu Registration

## Phase 1: Core Trait and Builder

- [ ] **1.1** Define `MenuContributor` trait, `MenuContribution`, `MenuPlacement`,
      priority constants in `src/tui/components/leader_menu.rs`
- [ ] **1.2** Implement `LeaderMenu::build(contributors, hidden)` that collects,
      deduplicates, resolves conflicts, and builds the menu tree
- [ ] **1.3** Add `KeyConflict` struct and return conflicts from `build()`
- [ ] **1.4** Unit tests: conflict resolution, auto-submenu creation, hidden
      entries, empty contributors

## Phase 2: Builtin Migration

- [ ] **2.1** Add `leader_key: Option<LeaderBinding>` field to `SlashCommand`
- [ ] **2.2** Populate `leader_key` on relevant builtin slash commands (session
      submenu items, compact, help, layout submenu items)
- [ ] **2.3** Write `slash_command_contributions()` adapter function
- [ ] **2.4** Write `builtin_keymap_contributions()` for non-slash items (model
      selector, thinking toggle, etc.) and submenu openers
- [ ] **2.5** Replace `LeaderMenu::new()` call in `App::new()` with
      `LeaderMenu::build()` using builtin contributors
- [ ] **2.6** Verify: default menu is identical to the old hardcoded menu

## Phase 3: Plugin Integration

- [ ] **3.1** Add `leader_menu: Vec<PluginLeaderEntry>` to `PluginManifest`
- [ ] **3.2** Add `PluginLeaderEntry` struct with serde derives
- [ ] **3.3** Implement `MenuContributor` for `PluginManager`
- [ ] **3.4** Rebuild leader menu after plugin load/unload in `interactive.rs`
- [ ] **3.5** Validation: warn on bad entries (non-ASCII key, empty label,
      missing `/` prefix)
- [ ] **3.6** Test: plugin with `leader_menu` in manifest adds items; unload
      removes them

## Phase 4: User Config

- [ ] **4.1** Add `LeaderMenuConfig` to settings struct
- [ ] **4.2** Implement `MenuContributor` for `LeaderMenuConfig`
- [ ] **4.3** Convert `hide` list to `HashSet<(char, MenuPlacement)>` and pass
      to `build()`
- [ ] **4.4** Wire up in `interactive.rs` — user config contributor passed to
      `build()` alongside builtins and plugins
- [ ] **4.5** Test: user item overrides builtin, user hide removes builtin,
      user creates new submenu

## Phase 5: Polish

- [ ] **5.1** Log `KeyConflict` diagnostics to stderr/debug log
- [ ] **5.2** Add `/leader` slash command to dump current menu structure
      (debugging aid)
- [ ] **5.3** Document `leader_menu` config in help text
- [ ] **5.4** Update plugin SDK README with `leader_menu` manifest field
