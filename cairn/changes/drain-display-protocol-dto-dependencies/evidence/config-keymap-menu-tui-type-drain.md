Artifact-Type: validation-log
Task-ID: I8,V7
Covers: r[remaining-coupling-drain.display-protocol-dependency-drain.neutral-display-dtos], r[remaining-coupling-drain.display-protocol-dependency-drain.validation]
Status: pass

## Scope

Removed the `clanker-tui-types` dependency from `clankers-config`:

- `clankers-config::keybindings` now owns only serializable keymap settings (`KeymapConfig`, `KeymapPreset`) and no longer reexports TUI action/input types.
- Runtime/TUI action call sites import `clanker_tui_types::{Action, CoreAction, ExtendedAction, InputMode}` directly at the display/app edge.
- User-configured leader-menu items are projected in the root `interactive` shell through `ConfigLeaderMenuContributor`, keeping `LeaderMenuConfig` as settings data only.

## Validation

Commands run from repository root:

```text
cargo check -p clankers-config -p clankers
cargo check -p clankers-config -p clankers --tests
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
```

All commands exited 0.

## Result

The lego dependency ownership inventory now records `clanker-tui-types` with 2 internal dependents instead of 3. `clankers-config` is no longer one of the display DTO dependents.
