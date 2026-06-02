## ADDED Requirements

### Requirement: Process tool stays a projection [r[process-job-backend-adapters.root-projection]]

The agent-visible root `process` tool MUST parse JSON into typed process-job requests, select an injected backend service, and project typed receipts. It MUST NOT own reusable backend state machines, host CLI parsers, durable retention policy, or notification policy inline.

#### Scenario: Root file contains only projection logic [r[process-job-backend-adapters.root-projection.thin-file]]
- GIVEN the process tool source is inspected
- WHEN backend-specific start/list/log/kill/restart/adopt/GC behavior is required
- THEN `src/tools/process.rs` MUST call named backend/service owners rather than implementing that policy inline
- AND the diagnostic MUST name the expected owner for any policy that remains in the root file

### Requirement: Backend adapters are typed and fakeable [r[process-job-backend-adapters.backend-adapters]]

Native, pueue, and systemd process-job backends MUST be implemented behind typed service or adapter boundaries that can be exercised with fake runners or in-memory registries.

#### Scenario: Native backend owns native lifecycle policy [r[process-job-backend-adapters.backend-adapters.native]]
- GIVEN a native process job is started, stopped, restarted, adopted, or reconciled
- WHEN the behavior is tested
- THEN the test MUST exercise a native backend owner rather than a monolithic root tool helper
- AND admission, termination, and durable summary policy MUST use runtime process-job DTOs

#### Scenario: Pueue backend owns pueue projection [r[process-job-backend-adapters.backend-adapters.pueue]]
- GIVEN pueue task JSON or log text is projected into process-job receipts
- WHEN deterministic fixtures run
- THEN parsing and command projection MUST live behind a pueue backend owner with a fake runner
- AND pueue unavailable or unsupported behavior MUST return typed degraded receipts before mutation

#### Scenario: Systemd backend owns systemd projection [r[process-job-backend-adapters.backend-adapters.systemd]]
- GIVEN systemd unit output is projected into process-job receipts
- WHEN deterministic fixtures run
- THEN parsing and command projection MUST live behind a systemd backend owner with a fake runner
- AND non-systemd or unsupported behavior MUST return typed degraded receipts before mutation

### Requirement: Durable policy is runtime-owned [r[process-job-backend-adapters.durable-policy]]

Durable record reconciliation, retention/GC filtering, log-degradation, redaction, and notification decisions MUST be owned by runtime process-job services or focused service adapters, not by the agent-visible root tool.

#### Scenario: Durable policy tests do not construct the root tool [r[process-job-backend-adapters.durable-policy.no-root-tool]]
- GIVEN retention, reconciliation, log degradation, or notification behavior is verified
- WHEN focused tests run
- THEN they MUST call runtime/service helpers directly or through a fake process-job service
- AND they MUST NOT require `ProcessTool`, a TUI, a daemon actor, or live host process backends

### Requirement: Process adapter validation is deterministic [r[process-job-backend-adapters.verification]]

Verification MUST combine fake backend fixtures, source-boundary rails, and existing process tool parity tests.

#### Scenario: Backend fixtures and rails prove ownership [r[process-job-backend-adapters.verification.backend-fixtures]]
- GIVEN the drain is complete
- WHEN validation runs
- THEN native, pueue, systemd, durable, retention, and notification behavior MUST each have focused deterministic coverage
- AND architecture rails MUST fail if reusable backend/storage policy returns to `src/tools/process.rs`

#### Scenario: Closeout preserves process tool behavior [r[process-job-backend-adapters.verification.closeout]]
- GIVEN user-facing process tool behavior is preserved
- WHEN closeout validation runs
- THEN focused process tests, architecture rails, Cairn gates/validate, and broad compile checks MUST pass or have explicit checked evidence for environmental limitations
