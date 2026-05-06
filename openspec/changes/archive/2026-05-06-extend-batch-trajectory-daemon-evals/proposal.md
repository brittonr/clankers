## Why

The batch runner baseline covers local foreground jobs. Hermes parity and self-evolution need resumable daemon/session-backed eval runs with trajectory export and objective receipts.

## What Changes

- Add daemon/session execution mode for batch jobs.
- Add resumable run manifests and objective evaluation receipts.
- Export JSONL, ShareGPT, and eval/RL trajectory formats with redaction policy.

## Out of Scope

- Unbounded concurrent provider calls.
- Persisting API keys or raw provider payloads outside intentional trajectory records.

## Capabilities

### New Capabilities
- `batch-trajectory-runner` follow-up behavior for extend batch trajectories for daemon evals.

### Modified Capabilities
- `batch-trajectory-runner` gains implementation-ready requirements for the next Hermes parity slice.

## Impact

- **Files**: OpenSpec artifacts first; implementation tasks identify expected Rust/docs touch points.
- **APIs**: May add CLI flags, tool schemas, settings fields, or daemon/session messages as described in the design.
- **Testing**: Targeted unit/integration checks plus `cargo check --tests` for touched crates.
