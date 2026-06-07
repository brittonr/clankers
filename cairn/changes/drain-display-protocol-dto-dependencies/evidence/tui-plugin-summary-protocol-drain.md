Artifact-Type: validation-log
Task-ID: I6,V5
Covers: r[remaining-coupling-drain.display-protocol-dependency-drain.protocol-edge], r[remaining-coupling-drain.display-protocol-dependency-drain.validation]
Status: pass

## Scope

Removed the `clankers-protocol` dependency from `clankers-tui`:

- Added a display-edge `clanker_tui_types::PluginSummary` for stored daemon plugin-list UI state.
- `clankers-tui::App` stores the display DTO instead of `clankers_protocol::PluginSummary`.
- Attach event handling projects protocol `DaemonEvent::PluginList` summaries into the TUI display DTO at the attach/protocol edge.

## Validation

Commands run from repository root:

```text
cargo check -p clankers-tui
cargo check -p clanker-tui-types -p clankers-tui --tests
cargo check -p clankers
cargo check -p clankers --tests
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
```

All commands exited 0.

## Result

The lego dependency ownership inventory now records `clankers-protocol` with 2 internal dependents instead of 3. `clankers-tui` no longer imports the protocol crate; protocol DTO projection stays in root attach/event handling.
