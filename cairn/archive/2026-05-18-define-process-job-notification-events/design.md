## Context

Background tasks should continue while the agent works on other things. Completion and rare readiness events are useful; streaming every line into the conversation is not. Delivery must work for local TUI, daemon attach, remote attach, and future bridges without making backends know about those transports.

## Goals / Non-Goals

**Goals:**

- Define typed completion/readiness event payloads.
- Persist event metadata for reattach replay.
- Deduplicate events per delivery target.
- Rate-limit and suppress noisy watch patterns.
- Keep backends decoupled from delivery transports.

**Non-Goals:**

- Do not implement general pub/sub infrastructure unrelated to process/jobs.
- Do not push continuous stdout/stderr as notifications.
- Do not guarantee watch-pattern delivery as strongly as terminal completion delivery.

## Decisions

### Decision 1: notification policy creates events, sinks deliver them

**Choice:** Job observation code emits backend facts; service-level notification policy creates `ProcessJobNotificationEvent`; delivery occurs through `ProcessJobNotificationSink`.

**Rationale:** Backends should not know whether a user is in TUI, daemon attach, Matrix, or another client.

**Implementation:** Event fields include `event_id`, `job_id`, `owner_scope`, `kind`, `backend`, `status`, `matched_pattern_id`, `safe_excerpt`, `log_ref`, `created_at`, and `dedup_key`.

### Decision 2: completion is exactly-once per job policy; readiness is best-effort and bounded

**Choice:** `notify_on_complete` produces one terminal event per job. `watch_patterns` produce bounded readiness events subject to rate limits and suppression.

**Rationale:** Completion is reliably actionable. Pattern matches are hints and can become noisy.

**Implementation:** Store completion-delivered state in metadata. Store readiness counters/windows and suppression state separately from raw logs.

### Decision 3: replay is authorization-filtered

**Choice:** Persist actionable notification events and replay them on reattach only to authorized sessions/scopes.

**Rationale:** Detached clients should not miss completion, but replay must not leak logs/commands across sessions.

## Validation Plan

- `openspec validate define-process-job-notification-events --strict --json`
- Unit tests for event id/dedup key generation.
- Fake sink tests for multi-client delivery.
- Reattach replay tests with authorized and unauthorized scopes.
- Rate-limit/suppression tests for noisy watch patterns.
