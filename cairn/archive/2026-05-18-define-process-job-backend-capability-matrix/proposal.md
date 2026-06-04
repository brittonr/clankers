## Why

Native child processes, pueue tasks, and systemd units do not support the same operations. Durable process/job support needs an explicit capability matrix so API receipts can fail honestly instead of pretending every backend supports stdin, restart, adoption, resource limits, queueing, and log cursors.

## What Changes

- **Backend capability matrix**: Define supported/unsupported operations for native, pueue, and systemd backends.
- **Typed unsupported behavior**: Require `unsupported_action_for_backend` receipts before mutation.
- **Backend-neutral projection**: Expose capability information through DTOs so tool/API/TUI clients can adapt without backend-specific parsing.
- **Contract tests**: Add fake and backend-specific tests proving capabilities are advertised and enforced consistently.

## Capabilities

### New Capabilities

- `process-job-backend-capability-matrix`: Backend operation and feature support contract for durable process/jobs.

### Modified Capabilities

- `durable-process-jobs`: Uses the capability matrix for backend selection, validation, and receipts.
- `process-job-tool-api`: Uses matrix-derived errors for unsupported actions.

## Impact

- **Files likely affected**: process/job backend traits, native backend adapter, pueue adapter, systemd adapter, DTOs, process tool validation, TUI/daemon projections, backend tests.
- **APIs**: Adds backend capability descriptors and typed unsupported-action details.
- **Testing**: Matrix fixtures, fake backend contract tests, native compatibility tests, pueue/systemd availability tests.
