## Context

The durable process/job design intentionally supports multiple backends. Native is interactive and can own pipes. Pueue is a durable queue. Systemd is a supervisor/cgroup/log manager. Their feature sets overlap but are not identical.

## Goals / Non-Goals

**Goals:**

- Define one backend capability descriptor shape.
- Make unsupported operations fail before backend mutation.
- Keep backend capability checks in the service layer.
- Let TUI/daemon/API surfaces show available operations without knowing backend internals.

**Non-Goals:**

- Do not force backends to emulate unsupported features.
- Do not make pueue/systemd mandatory dependencies.
- Do not specify exact pueue/systemd CLI invocations here.

## Decisions

### Decision 1: capabilities are data, not scattered match statements

**Choice:** Each backend exposes a `BackendCapabilities` DTO with booleans/enums for operations and feature quality.

**Rationale:** Callers and tests need a stable contract. Scattered backend checks drift and hide unsupported behavior.

**Implementation:** Include support for `stdin`, `restart`, `kill_tree`, `log_cursor`, `bounded_log`, `adoption`, `resource_limits`, `queueing`, `priority`, `dependencies`, `live_status`, `completion_notification`, and `readiness_watch`.

### Decision 2: service validates actions against capabilities

**Choice:** `ProcessJobService` checks requested action/resource policy against the matrix before invoking backend mutation.

**Rationale:** This keeps backend adapters thin and makes typed errors deterministic.

### Decision 3: capability details are projected safely

**Choice:** Receipts and TUI summaries may expose safe capability data for a job/backend, but not raw backend configuration that could reveal paths/secrets.

## Validation Plan

- `openspec validate define-process-job-backend-capability-matrix --strict --json`
- Unit tests for `BackendCapabilities` serialization and defaults.
- Service tests with fake backends covering unsupported actions.
- Native/pueue/systemd adapter tests verifying advertised capabilities match behavior.
