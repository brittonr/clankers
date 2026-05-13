## Why

Attach mode is a large same-substrate client that mixes session resolution, socket IO, local slash command handling, TUI loop state, daemon event projection, and test support. This change turns the decomposition finding into an implementation-ready OpenSpec so the drain can proceed without rediscovering scope.

## What Changes

- Decompose `src/modes/attach.rs` into named modules around stable functional-core / imperative-shell boundaries.
- Preserve current public API, command behavior, receipts, metadata redaction, and regression coverage.
- Add focused parity/negative tests before broad cleanup.

## Capabilities

### Modified Capabilities
- `attach-module-split`: Adds a decomposition requirement for the current implementation seam.

## Impact

- **Files**: `src/modes/attach.rs` plus new sibling modules/tests as needed.
- **APIs**: Existing external APIs should remain source-compatible unless the design explicitly documents a compatibility alias.
- **Dependencies**: No new runtime dependency is expected for the decomposition itself.
- **Testing**: Targeted nextest filters for the seam, `cargo check --tests` for touched crates, strict OpenSpec validation, and `git diff --check`.
