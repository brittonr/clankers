## Why

Durable process/job handles must be honest after daemon restart or crash. Native processes may still exist but lose stdout pipe continuity; backend jobs may be done, missing, unavailable, or ambiguous. A precise reconciliation state machine prevents overclaiming recovery and protects against PID reuse or backend ID confusion.

## What Changes

- **Reconciliation statuses**: Define running, reattached, reattached-log-incomplete, exited, lost-after-restart, backend-unavailable, orphaned, and identity-mismatch states.
- **PID/backend identity checks**: Require verification before adopting persisted records after restart.
- **Backend-specific reconciliation**: Define native, pueue, and systemd reconciliation behavior through interfaces.
- **Honest receipts**: Ensure list/poll/log/kill report degraded states explicitly.

## Capabilities

### New Capabilities

- `process-job-reconciliation-state-machine`: Restart/crash reconciliation semantics for durable process/job records.

### Modified Capabilities

- `durable-process-jobs`: Uses this state machine during daemon startup and backend availability changes.

## Impact

- **Files likely affected**: daemon startup hooks, process/job store, native backend adapter, pueue/systemd adapters, process list/poll/log receipts, tests.
- **APIs**: Adds typed reconciliation states and degraded log/status details.
- **Testing**: Temp db/log restart tests, fake backend reconciliation tests, PID reuse tests where practical.
