## Why

Manual checkpoint commands are useful, but Hermes-style autonomous repo work needs automatic protection before file-mutating tools and a clear rollback UX tied to session receipts.

## What Changes

- Create git-backed checkpoints before configured file-mutating tools.
- Expose rollback/list/review surfaces with confirmation.
- Attach safe checkpoint ids to tool/session metadata.

## Out of Scope

- Rolling back without explicit confirmation.
- Non-git or remote snapshot backends.

## Capabilities

### New Capabilities
- `checkpoints-rollback` follow-up behavior for add automatic checkpoints and rollback.

### Modified Capabilities
- `checkpoints-rollback` gains implementation-ready requirements for the next Hermes parity slice.

## Impact

- **Files**: OpenSpec artifacts first; implementation tasks identify expected Rust/docs touch points.
- **APIs**: May add CLI flags, tool schemas, settings fields, or daemon/session messages as described in the design.
- **Testing**: Targeted unit/integration checks plus `cargo check --tests` for touched crates.
