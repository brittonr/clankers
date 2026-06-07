Artifact-Type: validation-log
Task-ID: I7,V6
Covers: r[remaining-coupling-drain.display-protocol-dependency-drain.neutral-display-dtos], r[remaining-coupling-drain.display-protocol-dependency-drain.validation]
Status: pass

## Scope

Moved plugin UI DTOs out of the TUI type edge and into the neutral message contract crate:

- Added neutral `clanker_message::plugin` contracts for `Widget`, `Direction`, `PluginUiState`, `StatusSegment`, `PluginNotification`, and `PluginSummary`.
- `clanker-tui-types` now reexports those neutral plugin UI contracts for display-edge compatibility.
- `clankers-plugin` now reexports plugin UI DTOs from `clanker-message` and no longer depends on `clanker-tui-types`.

## Validation

Commands run from repository root:

```text
cargo check -p clanker-message -p clanker-tui-types -p clankers-plugin
cargo check -p clankers
cargo check -p clankers --tests
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
```

All commands exited 0.

## Result

The lego dependency ownership inventory now records `clanker-tui-types` with 3 internal dependents instead of 4. `clankers-plugin` now depends on the neutral `clanker-message` DTO owner instead of the display-edge TUI type crate.
