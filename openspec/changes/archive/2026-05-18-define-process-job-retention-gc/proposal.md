## Why

Durable process/job support will persist metadata, log files or backend log references, notification events, and audit-like lifecycle facts. Without explicit retention and garbage collection semantics, logs can grow unbounded, redb can keep stale records forever, and cleanup can accidentally delete active job state.

## What Changes

- **Retention classes**: Define active, recent-completed, failed, adopted, notification, and tombstone retention behavior.
- **GC receipts**: Add typed garbage collection summaries for metadata/log/event cleanup.
- **Active job protection**: Forbid completed-job GC from removing running or unreconciled active jobs.
- **Nix/log integration**: Specify tmpfiles/logrotate/journald interaction and missing-log degradation.
- **Disk pressure behavior**: Define bounded output/disk-full behavior without corrupting metadata.

## Capabilities

### New Capabilities

- `process-job-retention-gc`: Retention and garbage collection contract for durable process/job metadata, logs, and notifications.

### Modified Capabilities

- `durable-process-jobs`: Uses this policy for metadata/log lifecycle and registry history.
- `nixos-process-job-config`: Materializes daemon defaults and log/state directories for retention.

## Impact

- **Files likely affected**: process/job store, native log store, notification event store, NixOS module, process tool GC action, TUI/history projections, tests.
- **APIs**: Adds typed GC operation/receipt and retention class fields.
- **Testing**: Active-job protection tests, missing-log tests, disk-full/output-overflow tests, Nix module eval tests.
