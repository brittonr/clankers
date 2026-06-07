Artifact-Type: validation-log
Task-ID: I4,V3
Covers: r[remaining-coupling-drain.display-protocol-dependency-drain.neutral-display-dtos], r[remaining-coupling-drain.display-protocol-dependency-drain.rails], r[remaining-coupling-drain.display-protocol-dependency-drain.validation]
Status: pass

## Scope

Drained two non-edge `clanker-tui-types` dependencies by moving neutral contracts into `clanker-message` and keeping `clanker-tui-types` as a display-edge reexport surface:

- `clankers-model-selection` now imports/reexports `BudgetStatus`, `CostSummary`, `ModelCostBreakdown`, and `CostProvider` from `clanker-message` instead of `clanker-tui-types`.
- `clankers-procmon` now implements `clanker_message::ProcessDataSource` and emits `clanker_message::ProcessSnapshot` / `ProcessDisplayState` instead of importing from `clanker-tui-types`.

## Validation

Commands run from repository root:

```text
cargo check -p clankers-procmon -p clankers-model-selection -p clanker-tui-types -p clanker-message
cargo check -p clankers-tui
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
```

All commands exited 0.

## Result

The dependency-ownership rail now records `clanker-tui-types` with 5 dependents instead of 7. The removed dependents are `clankers-model-selection` and `clankers-procmon`; both depend on the neutral `clanker-message` contract instead.
