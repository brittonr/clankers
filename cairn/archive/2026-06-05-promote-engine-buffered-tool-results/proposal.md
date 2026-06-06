# Change: Promote Engine Buffered Tool Results

## Why

The embedded SDK experimental budget still carries four `clankers-engine` rows for `EngineBufferedToolResult` and its fields. The type is not a desktop integration detail: it is the public shape behind `EngineState::buffered_tool_results`, which is already part of the supported engine state surface. Leaving the inner record experimental makes the state contract inconsistent and keeps the budget from converging.

## What Changes

- Promote `EngineBufferedToolResult` and its fields from `experimental` to `supported` in the generated embedded SDK inventory.
- Update the experimental SDK port budget so the engine buffered-result group is promoted with deterministic reducer evidence instead of retained as experimental.
- Refresh brick stability and release receipt artifacts that pin the embedded SDK inventory counts and hashes.

## Impact

- **Files**: `crates/clankers-engine/src/lib.rs`, `docs/src/generated/embedded-sdk-api.md`, `policy/embedded-lego/*.json`, `scripts/emit-embedded-sdk-release-receipt.rs`, and Cairn evidence for this change.
- **Testing**: focused `clankers-engine` buffered reducer tests plus embedded SDK inventory/budget/brick rails and the aggregate embedded SDK acceptance runner.
