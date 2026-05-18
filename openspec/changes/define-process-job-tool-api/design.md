## Context

The durable process/job change introduces stable IDs, backend abstraction, notification policy, retention, profiles, and admission control. Those concepts need one canonical API contract so the agent, TUI, daemon attach, and future bridges all agree on request and receipt fields.

## Goals / Non-Goals

**Goals:**

- Preserve current `process` actions for native background processes.
- Add typed request DTOs for durable options without tying callers to native/pueue/systemd internals.
- Add typed receipts and errors that every public surface can project.
- Make public process/job IDs BLAKE3-native and deterministic without exposing backend-native locators as stable identity.
- Keep parser, validator, service, backend, and presentation code separable.

**Non-Goals:**

- Do not implement the durable backends in this change.
- Do not require a new `jobs` tool before the existing `process` tool is migrated.
- Do not expose raw secret env/argv/log bytes in receipts.

## Decisions

### Decision 1: process tool parses into backend-neutral request DTOs

**Choice:** Add typed request structs such as `StartJobRequest`, `ListJobsRequest`, `LogJobRequest`, `KillJobRequest`, `StartProfileRequest`, and `GcJobsRequest`.

**Rationale:** The tool parser should validate syntax and capability-relevant fields, then call `ProcessJobService`. It should not spawn children, read redb, invoke pueue/systemd, or format TUI state directly.

**Implementation:** Keep shell/direct execution compatibility in start requests, but normalize optional fields: `backend`, `notify_on_complete`, `watch_patterns`, `resource_policy`, `owner_scope`, `retention_class`, `profile`, `log_policy`, and `admission_policy`.

### Decision 2: every public result is a typed receipt

**Choice:** Define receipt structs with common fields and operation-specific payloads.

**Rationale:** Agents and clients should not parse human text to determine status, ids, log cursors, or errors.

**Implementation:** Common fields include `operation`, `ok`, `id`, `backend`, `status`, `owner_scope`, `started_at`, `completed_at`, `elapsed_ms`, `log_ref`, `cursor`, `error_code`, `backend_detail`, and `summary`.

### Decision 3: public process/job identity is BLAKE3-native

**Choice:** `ProcessJobId` is derived from a canonical, versioned identity envelope using BLAKE3. Backend-native locators are carried separately in `backend_ref`.

**Rationale:** Sequential IDs (`proc_1`) and backend task IDs (`pueue_7`, systemd unit names, PIDs) are not durable public identity. They can leak host details, collide across daemon restarts/backends, and force reconciliation, notifications, retention, and redaction to depend on unstable backend-specific strings.

**Implementation:** Define one canonical envelope with explicit version/domain fields, backend kind, owner scope, profile/request identity when available, sanitized command/workdir/log identity inputs, and creation nonce where deterministic replay is not otherwise safe. Serialize the envelope with a pinned canonical form, derive `blake3(envelope)`, encode with a stable prefix such as `proc_b3_<digest>`, and keep `backend_ref` as the only place for `pid:<pid>`, `pueue:<task_id>`, or `systemd:<unit>` locators.

### Decision 4: unsupported behavior is explicit

**Choice:** Backend capability mismatch returns typed codes such as `unsupported_action_for_backend`, `backend_unavailable`, `capability_denied`, `concurrency_limit_exceeded`, and `log_unavailable`.

**Rationale:** Different backends will not support stdin, restart, adoption, or log cursors equally. Failures must be deterministic and reviewable.

## Validation Plan

- `openspec validate define-process-job-tool-api --strict --json`
- Golden tests for request parsing into DTOs.
- Golden tests for receipt serialization.
- Golden tests for canonical BLAKE3 `ProcessJobId` derivation and backend-ref separation.
- Native compatibility tests for existing `process` actions.
- Negative tests for unsupported backend/action combinations and capability denial.
