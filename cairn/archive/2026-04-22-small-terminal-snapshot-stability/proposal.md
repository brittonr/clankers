## Why

`tests/tui/visual.rs::snapshot_small_terminal` is failing late in `cargo nextest run --fail-fast`, which blocks unrelated work and leaves the TUI visual baseline untrusted. The failure needs its own change so the auth/Codex workstream can stay focused while the small-terminal snapshot is investigated and stabilized separately.

## What Changes

- Investigate why the `small_12x50_structure` visual snapshot drifts between expected and actual output.
- Make the 12x50 startup snapshot deterministic again by fixing either the TUI layout/rendering path or the test harness/snapshot expectation, whichever the investigation proves is correct.
- Add focused regression coverage so small-terminal visual drift is caught with clearer evidence and without coupling to unrelated TUI state.
- Document the chosen stabilization rule for future TUI snapshot updates.

## Capabilities

### New Capabilities
- `tui-visual-snapshot-stability`: Define and verify deterministic small-terminal visual snapshot behavior for the startup TUI layout.

### Modified Capabilities
- None.

## Impact

- `tests/tui/visual.rs`
- `tests/tui/snapshots/tui_tests__tui__visual__small_12x50_structure.snap`
- TUI layout/rendering code under `src/` and `crates/clankers-tui/` if the root cause is real rendering drift rather than snapshot/test harness drift
- Visual snapshot maintenance workflow for small-terminal regressions
