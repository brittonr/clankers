# process-job-tool-api Specification

## Purpose
TBD - created by archiving change define-process-job-tool-api. Update Purpose after archive.
## Requirements
### Requirement: Backend-neutral process/job request API [r[process-job-tool-api.requests]]

The system MUST parse process/job tool requests into backend-neutral typed request DTOs before validation, service orchestration, backend dispatch, storage, notification delivery, or UI projection.

#### Scenario: start request carries durable options [r[process-job-tool-api.requests.start-options]]

- GIVEN a caller starts a process/job
- WHEN the request includes backend, notification, watch pattern, resource, owner scope, retention, log, profile, or admission options
- THEN Clankers MUST represent those options in a typed backend-neutral request DTO
- THEN backend-specific modules MUST NOT parse raw tool JSON directly

#### Scenario: existing native defaults remain compatible [r[process-job-tool-api.compat.native-default]]

- GIVEN a caller uses existing `process` actions without durable-only fields
- WHEN the request is parsed and executed
- THEN Clankers MUST preserve current native behavior for start, list, poll, log, wait, kill, write, submit, and close
- THEN any new receipt fields MUST be additive or projected without breaking existing human-readable summaries

### Requirement: BLAKE3-native process/job identity [r[process-job-tool-api.identity]]

The system MUST use BLAKE3-derived public process/job identifiers for Clankers-owned durable job identity, and MUST keep backend-native locators separate from that public identity.

#### Scenario: public job id derives from canonical envelope [r[process-job-tool-api.identity.blake3-native]]

- GIVEN Clankers creates or adopts a process/job that needs a public `ProcessJobId`
- WHEN the job identity is materialized for receipts, notifications, persistence, reconciliation, retention, or UI projection
- THEN Clankers MUST derive the public id from a canonical, versioned identity envelope using BLAKE3
- THEN the envelope MUST include an explicit domain/version and enough sanitized identity inputs to avoid cross-backend or cross-owner collisions
- THEN deterministic fixture tests MUST pin the canonical envelope bytes and encoded id for representative native, pueue, and systemd jobs

#### Scenario: backend locators are not public stable ids [r[process-job-tool-api.identity.backend-ref-separation]]

- GIVEN a backend exposes a native locator such as a PID, pueue task id, or systemd unit name
- WHEN Clankers returns receipts, events, persisted records, or projections
- THEN the public `id` field MUST contain the BLAKE3-derived `ProcessJobId`
- THEN backend locators MUST appear only in a structured `backend_ref` or backend detail field
- THEN Clankers MUST NOT use raw sequential ids, PIDs, pueue task ids, or systemd unit names as the canonical public stable identity for new jobs

### Requirement: Structured process/job receipts [r[process-job-tool-api.receipts]]

The system MUST return machine-readable process/job receipts and errors with stable fields that can be consumed by agents, TUI, daemon clients, and future bridges.

#### Scenario: common receipt shape is present [r[process-job-tool-api.receipts.common-shape]]

- GIVEN any process/job operation completes or fails
- WHEN Clankers returns the result
- THEN the receipt MUST include operation, success flag, stable id when applicable, backend kind when applicable, typed status or error code, safe summary, and operation-specific payload fields
- THEN receipts MUST NOT require parsing human text to find ids, statuses, log cursors, or capability failures

#### Scenario: projections derive from shared DTOs [r[process-job-tool-api.receipts.projection]]

- GIVEN a receipt is displayed in agent text, TUI, daemon attach, or remote attach
- WHEN that surface renders the result
- THEN it MUST derive from shared receipt DTOs or explicit projection adapters
- THEN backend-specific details MUST remain optional structured fields rather than changing the common schema per backend

### Requirement: Typed unsupported-action and policy errors [r[process-job-tool-api.errors]]

The system MUST report backend, capability, and policy failures with typed error codes before any unsafe or unsupported mutation occurs.

#### Scenario: unsupported backend action is explicit [r[process-job-tool-api.errors.unsupported-action]]

- GIVEN a caller requests an action the selected backend does not support, such as stdin on a non-interactive backend
- WHEN Clankers validates the request
- THEN it MUST return `unsupported_action_for_backend` with backend and action fields
- THEN it MUST NOT silently ignore the action or convert it into an unrelated generic failure

#### Scenario: policy denial precedes backend dispatch [r[process-job-tool-api.errors.policy-before-dispatch]]

- GIVEN a request lacks capability, exceeds admission/resource policy, or selects an unavailable backend
- WHEN Clankers evaluates the request
- THEN it MUST return a typed denial receipt before contacting the backend
- THEN the denial MUST avoid leaking raw command, environment, or log secrets
