## Why

The agent turn loop remains the largest core module and mixes state transition, provider streaming, tool execution, transcript/session side effects, cancellation, and usage accounting. This change turns the decomposition finding into an implementation-ready OpenSpec so the drain can proceed without rediscovering scope.

## What Changes

- Decompose `crates/clankers-agent/src/turn/mod.rs` into named modules around stable functional-core / imperative-shell boundaries.
- Preserve current public API, command behavior, receipts, metadata redaction, and regression coverage.
- Add focused parity/negative tests before broad cleanup.

## Capabilities

### Modified Capabilities
- `thin-agent-shell`: Adds a decomposition requirement for the current implementation seam.

## Impact

- **Files**: `crates/clankers-agent/src/turn/mod.rs` plus new sibling modules/tests as needed.
- **APIs**: Existing external APIs should remain source-compatible unless the design explicitly documents a compatibility alias.
- **Dependencies**: No new runtime dependency is expected for the decomposition itself.
- **Testing**: Targeted nextest filters for the seam, `cargo check --tests` for touched crates, strict OpenSpec validation, and `git diff --check`.
