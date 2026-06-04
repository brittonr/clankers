## Why

Durable process/job management persists metadata, stores logs, emits notifications, and exposes remote observation. Without a precise security/redaction contract, command previews, argv, environment values, log excerpts, backend references, or notifications could leak secrets across sessions or into redb.

## What Changes

- **Persistence redaction policy**: Define what metadata may be stored in redb and what must be omitted or redacted.
- **Log access policy**: Capability-gate raw/bounded logs and safe excerpts separately.
- **Notification safety**: Require notification excerpts to be redacted and bounded.
- **Audit-safe backend refs**: Store backend IDs and command previews safely without exposing secrets.

## Capabilities

### New Capabilities

- `process-job-security-redaction`: Security and redaction contract for durable process/job metadata, logs, notifications, and receipts.

### Modified Capabilities

- `durable-process-jobs`: Uses this policy for persisted metadata, log access, notifications, and remote observation.
- `process-job-tool-api`: Receipts/errors must follow this redaction policy.
- `process-job-notification-events`: Notification events must follow this redaction policy.

## Impact

- **Files likely affected**: process/job store DTOs, receipt projection, notification event creation, log store/read paths, capability checks, tests/fixtures.
- **APIs**: Defines safe vs raw fields and capability requirements.
- **Testing**: Redaction fixtures for env/argv/log excerpts, capability denial tests, persistence tests asserting secrets are not stored in redb metadata.
