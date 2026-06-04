## Context

Persisting a job record is not the same as continuing to own all process resources after a crash. Native pipe handles may be lost, PIDs may be reused, and external backend services may be unavailable. Reconciliation must be conservative and explicit.

## Goals / Non-Goals

**Goals:**

- Define exact reconciliation state vocabulary.
- Prevent silent adoption of PID-reused or mismatched backend targets.
- Preserve stable Clankers IDs while updating backend-derived state.
- Make degraded logs/status visible in receipts.

**Non-Goals:**

- Do not resurrect processes that exited during downtime.
- Do not promise exact exit status for native processes lost during daemon crash unless the backend can prove it.
- Do not reconcile arbitrary external processes not started/adopted by Clankers.

## Decisions

### Decision 1: reconciliation is an interface method

**Choice:** Each backend implements reconciliation from persisted metadata to a `ReconciliationOutcome`.

**Rationale:** Native PID checks, pueue task inspection, and systemd unit inspection differ but must project into one state machine.

### Decision 2: identity must be verified before reattach

**Choice:** Native reconciliation verifies process group/PID identity with start time or other host facts where available; pueue/systemd verify backend refs. Ambiguous matches fail closed.

**Rationale:** PID reuse and backend name collisions are worse than losing a handle.

### Decision 3: logs can be degraded independently from process state

**Choice:** A job may be `reattached` while logs are `incomplete`, `unavailable`, or backend-referenced.

**Rationale:** Status and log ownership fail differently after restart.

## Validation Plan

- `openspec validate define-process-job-reconciliation-state-machine --strict --json`
- Unit tests for state transitions and projection receipts.
- Native restart tests using temp redb/log dirs.
- Fake backend tests for exited/lost/unavailable/identity-mismatch outcomes.
