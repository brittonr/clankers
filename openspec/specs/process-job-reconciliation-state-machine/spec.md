# process-job-reconciliation-state-machine Specification

## Purpose
This specification defines the safe process/job reconciliation state machine used after daemon restarts or crashes, including identity checks, backend-specific adoption contracts, stable ID preservation, and degraded receipt projection for incomplete logs or unavailable backends.

## Requirements
### Requirement: Reconciliation state vocabulary [r[process-job-reconciliation-state-machine.states]]

The system MUST represent daemon restart/crash reconciliation with explicit process/job and log degradation states.

#### Scenario: state vocabulary is typed [r[process-job-reconciliation-state-machine.states.vocabulary]]

- GIVEN persisted process/job records are loaded during daemon startup
- WHEN reconciliation runs
- THEN Clankers MUST classify each record into typed states such as `running`, `reattached`, `reattached_log_incomplete`, `exited`, `lost_after_restart`, `backend_unavailable`, `orphaned`, or `identity_mismatch`
- THEN it MUST NOT silently omit unreconciled records from list/status results until retention policy removes them

### Requirement: Identity verification before reattach [r[process-job-reconciliation-state-machine.identity]]

The system MUST verify native or backend identity before treating a persisted record as reattached or running.

#### Scenario: PID reuse fails closed [r[process-job-reconciliation-state-machine.identity.pid-reuse]]

- GIVEN a persisted native process record contains PID or process-group metadata
- WHEN daemon startup finds a live OS process with the same PID or group
- THEN Clankers MUST verify identity using available start time, process group, command/workdir fingerprint, or equivalent host facts before reattaching
- THEN unverifiable or mismatched targets MUST become `identity_mismatch` or `lost_after_restart` rather than being adopted silently

### Requirement: Backend-specific reconciliation contracts [r[process-job-reconciliation-state-machine.backends]]

The system MUST reconcile native, pueue, and systemd records through backend interfaces that project into the common state vocabulary.

#### Scenario: external backend reconciliation is explicit [r[process-job-reconciliation-state-machine.backends.external]]

- GIVEN a persisted pueue task id or systemd unit reference exists
- WHEN reconciliation queries the configured backend
- THEN Clankers MUST map backend state into the common reconciliation vocabulary
- THEN unavailable backend services MUST produce `backend_unavailable` without deleting metadata or falling back to another backend silently

### Requirement: Stable IDs and degraded receipts [r[process-job-reconciliation-state-machine.receipts]]

The system MUST preserve stable Clankers IDs while exposing reconciliation and log degradation in receipts.

#### Scenario: degraded status appears in receipts [r[process-job-reconciliation-state-machine.receipts.degraded]]

- GIVEN a job is reattached with incomplete logs, lost after restart, or blocked by backend unavailability
- WHEN a caller lists, polls, logs, or kills the job
- THEN Clankers MUST include stable id, reconciliation state, backend kind, backend reference when safe, and log degradation detail in the typed receipt
- THEN unsupported operations for the degraded state MUST return typed errors rather than generic failures

#### Scenario: stable id survives reconciliation [r[process-job-reconciliation-state-machine.storage.stable-id]]

- GIVEN reconciliation updates backend state for a persisted job
- WHEN the record is saved
- THEN the original stable Clankers id MUST remain unchanged
- THEN backend refs and status fields MAY be updated only after identity checks pass

