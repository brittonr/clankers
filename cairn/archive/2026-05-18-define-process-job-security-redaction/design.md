## Context

Long-running jobs can include secrets in argv, env, stdout/stderr, paths, headers, and service logs. Durable support expands exposure because metadata and notifications persist beyond the original agent turn and may be visible to attached/remote clients.

## Goals / Non-Goals

**Goals:**

- Prevent raw secrets from being persisted in redb process/job metadata.
- Capability-gate log reads and raw command/argv/environment details.
- Ensure notification excerpts and receipt summaries are bounded and redacted.
- Keep redaction policy centralized and testable.

**Non-Goals:**

- Do not claim perfect secret detection in arbitrary logs.
- Do not mutate backend-owned logs such as journald or pueue logs.
- Do not prevent authorized users from reading raw logs when explicitly permitted.

## Decisions

### Decision 1: metadata stores safe previews, not raw secrets

**Choice:** Persist bounded command previews, redacted environment summaries, safe cwd/workspace policy, backend refs, and log refs. Do not persist raw env, headers, tokens, or full command lines that fail redaction.

**Rationale:** Metadata is frequently listed and replayed; it must be safe by default.

### Decision 2: log bytes are more sensitive than metadata

**Choice:** Treat raw/bounded log access as a separate capability from observe/list. Safe excerpts may be returned in receipts/notifications only after redaction and bounding.

**Rationale:** stdout/stderr often contain secrets or proprietary data. Listing a job should not imply raw log access.

### Decision 3: redaction is a service/projection concern

**Choice:** Backend adapters return raw backend facts to the service where necessary, but persistence, receipt projection, and notification creation call a centralized redaction helper before storing or emitting safe fields.

**Rationale:** Duplicating redaction across native/pueue/systemd backends will drift and leak.

## Validation Plan

- `openspec validate define-process-job-security-redaction --strict --json`
- Fixture tests for env, argv, header, token, path, and log excerpt redaction.
- Persistence tests asserting redb metadata excludes raw secrets.
- Capability tests for observe-only vs log-read vs raw-log access.
- Notification/receipt tests asserting safe excerpts are bounded and redacted.
