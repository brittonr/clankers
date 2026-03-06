# Proposal: Dynamic Registry Pattern

## Problem

Six major subsystems use hardcoded enums and static vecs that must be manually
kept in sync across multiple files. Adding a single feature (one slash command,
one panel, one leader menu item) routinely requires edits to 3–6 files and
touching match blocks that are hundreds or thousands of lines long. Plugins
cannot extend most of these subsystems at all.

### Affected Subsystems

| Subsystem | Enum/Registry | Lines of dispatch | Files touched to add one |
|-----------|---------------|-------------------|--------------------------|
| Slash commands | `SlashAction` (37 variants) | 1,831 | 3 |
| Panels | `PanelId` (6) + `PanelTab` (5) | 250 | 6+ |
| Leader menu | hardcoded `LeaderMenu::new()` | — | 1 (but disconnected) |
| Tool collision list | manual `builtin_names` HashSet | — | 1 (silent bugs) |
| Model roles | `ModelRole` (6 variants) | — | 1 + help text |
| Keybinding actions | `Action` (52 variants) | 55 (parse) | 3+ |

The worst offender is `handle_slash_command()` in `interactive.rs`: a single
1,831-line match block that grows with every new command.

## Proposed Solution

Apply the same **trait-based contributor pattern** across all six subsystems.
Each subsystem gets:

1. A **contributor trait** — anything (builtins, plugins, user config) can
   implement it to register entries.
2. A **builder** that collects contributions, resolves conflicts by priority,
   and produces the runtime data structure.
3. A **rebuild trigger** so the registry updates when plugins load/unload or
   config changes.

This is not a macro approach. Rationale:

- **Plugins are WASM, loaded at runtime.** Macros can't handle them.
- **User config is runtime data.** TOML/YAML overrides parsed at startup.
- **The codebase already uses traits** for extension points (`Tool`, `Provider`,
  `Panel`). This extends the pattern consistently.
- **Macros add a second mechanism** without eliminating the need for runtime
  registration. One mechanism is simpler.

## Scope

Six phases, each self-contained and independently shippable:

1. **Leader menu** — `MenuContributor` trait (smallest, proves the pattern)
2. **Slash commands** — `SlashCommandHandler` trait (highest pain/payoff)
3. **Panel registry** — dynamic `HashMap<PanelId, Box<dyn Panel>>` (eliminates
   dual-enum)
4. **Tool collision list** — derive from actual tool vec (trivial fix)
5. **Model roles** — string-keyed map instead of enum
6. **Keybinding actions** — split stable core from extensible feature actions

Each phase is detailed in its own spec under `specs/`.
