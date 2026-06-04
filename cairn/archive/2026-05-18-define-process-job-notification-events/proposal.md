## Why

Durable background jobs need useful completion/readiness notifications without spamming agent context or coupling backends to TUI/daemon delivery. The durable process/job spec requires `notify_on_complete` and bounded `watch_patterns`; this change pins the event, delivery, replay, and dedup contract.

## What Changes

- **Notification event schema**: Define stable event IDs and typed completion/readiness payloads.
- **Delivery sinks**: Route events through an interface that can target attached clients, daemon streams, persisted replay, and future bridges.
- **Detach/reattach replay**: Persist actionable events so users do not miss completion while detached.
- **Rate limiting/dedup**: Bound noisy watch patterns and make multi-client delivery idempotent.

## Capabilities

### New Capabilities

- `process-job-notification-events`: Stable long-running job notification event contract.

### Modified Capabilities

- `durable-process-jobs`: Uses this event model for completion and readiness delivery.

## Impact

- **Files likely affected**: daemon event types, session replay/ledger integration, process/job notification service, TUI notification handling, remote attach events, tests.
- **APIs**: Adds typed process/job notification events and delivery receipts.
- **Testing**: Fake sink tests, detach/reattach replay, dedup, noisy pattern suppression, capability-filtered delivery.
