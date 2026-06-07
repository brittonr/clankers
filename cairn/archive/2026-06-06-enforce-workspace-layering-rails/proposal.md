# Change: Enforce Workspace Layering Rails

## Why

Clankers has many crates and a growing number of source-boundary rails. The desired dependency direction is clear, but some checks are still encoded as local denied-token lists. A generated workspace layering rail would make coupling regressions visible before a green crate starts depending on daemon, TUI, provider discovery, plugin runtime, or root shell code.

## What Changes

- Define a workspace layer map for green contracts, host/facade crates, agent/controller orchestration, and application-edge shells.
- Generate dependency and public-constructor inventories from Cargo metadata and source ASTs where practical.
- Fail with owner diagnostics when a lower layer imports or constructs a higher-layer type outside a named adapter.

## Impact

- **Files**: architecture rail scripts, `Cargo.toml` workspace metadata or policy files, FCIS boundary tests, generated dependency inventories, and docs.
- **Testing**: workspace layering rail, FCIS controller shell boundaries, embedded SDK dependency rails, Cairn validation/gates, and full release harness when adopted broadly.
