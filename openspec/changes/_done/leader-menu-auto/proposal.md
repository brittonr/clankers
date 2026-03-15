# Proposal: Dynamic Leader Menu Registration

## Problem

The leader menu (`src/tui/components/leader_menu.rs`) is entirely hardcoded in
`LeaderMenu::new()`. Every item, submenu, and key binding is a static literal.
This creates several problems:

1. **Plugins can't add items.** Plugins declare `commands` in their manifest but
   have no way to surface them in the leader menu. Users must know the
   slash-command name.

2. **Slash commands and leader menu are disconnected.** The leader menu manually
   duplicates slash command strings (`"/new"`, `"/compact"`) with no shared
   registry. Adding a new slash command requires editing two files.

3. **Users can't customize the menu.** No config mechanism to add, remove, or
   rebind leader menu entries.

4. **Prompt templates are invisible.** Templates register via
   `register_prompt_templates()` for slash completion but have no leader menu
   presence.

## Proposed Solution

Replace the hardcoded menu with a **trait-based contribution system** where
multiple sources register menu items into a shared builder. The trait is the
right abstraction (not a macro) because:

- **Plugins load at runtime** — WASM plugins are discovered and loaded
  dynamically. Macros only work at compile time, so you'd need a trait for
  plugins anyway, making the macro redundant.
- **User config is runtime data** — TOML/YAML overrides are parsed at startup.
- **The codebase already uses traits** for extension points (`Tool`,
  `Provider`, `Panel`). A `MenuContributor` trait follows the established
  pattern.
- **Macros add complexity** — proc macros require a separate crate, are harder
  to debug, and don't compose with runtime sources. A declarative macro could
  reduce boilerplate for builtins, but the savings are marginal (the current
  `SlashCommand` structs already carry all the metadata needed).

## Why Not a Macro

A `#[leader_menu(key = 'n', submenu = "session")]` attribute macro on slash
commands looks appealing but fails on three counts:

1. **Can't handle plugins.** Plugin commands come from `plugin.json` at runtime.
   You'd still need a runtime registration path, making the macro a partial
   solution that adds a second mechanism.

2. **Can't handle user config.** Users overriding keys or hiding items is
   inherently runtime.

3. **Inventory/linkme crates** could auto-collect annotated statics, but they
   add linker-section magic that's fragile across platforms and doesn't work
   for the WASM plugin case anyway.

The trait-based approach handles all three sources uniformly.

## Scope

- `MenuContributor` trait with `fn menu_items(&self) -> Vec<MenuContribution>`
- Builtin slash commands implement the trait (or a free function adapter)
- `PluginManager` implements the trait, reading from loaded manifests
- User config (`leader_menu` section in settings) implements the trait
- `LeaderMenu::build(contributors)` replaces `LeaderMenu::new()`
- Priority system for key conflicts: user config > plugin > builtin
- Existing behavior preserved — default menu looks identical

## Relationship to Dynamic Registry Initiative

This is Phase 1 of a broader effort to replace hardcoded registries across 6
subsystems with trait-based dynamic registration. See
`../dynamic-registry/proposal.md` for the full plan. The leader menu is the
proving ground — smallest scope, lowest risk, establishes the shared pattern
and priority constants used by all subsequent phases.
