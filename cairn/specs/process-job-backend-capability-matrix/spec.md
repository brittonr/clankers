# process-job-backend-capability-matrix Specification

## Purpose

Define the typed process/job backend capability matrix used by Clankers to advertise native, pueue, systemd, and unavailable backend behavior without hard-coded caller assumptions.

## Requirements
### Requirement: Backend capability descriptor [r[process-job-backend-capability-matrix.descriptor]]

The system MUST expose backend process/job capabilities through a typed descriptor rather than hard-coded caller assumptions.

#### Scenario: common descriptor lists operation support [r[process-job-backend-capability-matrix.descriptor.common]]

- GIVEN a process/job backend is registered or selected
- WHEN Clankers queries its capabilities
- THEN the backend MUST return typed support information for stdin, restart, garbage collection, kill tree/control group, log cursors, bounded logs, adoption, resource limits, queueing, priorities, dependencies, live status, completion notifications, and readiness watches
- THEN unknown or unavailable capabilities MUST default to unsupported or degraded rather than assumed supported

### Requirement: Backend-specific capability contracts [r[process-job-backend-capability-matrix.backends]]

The system MUST define native, pueue, and systemd capability expectations explicitly and enforce them through the same descriptor shape.

#### Scenario: native backend advertises interactive process support [r[process-job-backend-capability-matrix.backends.native]]

- GIVEN the native backend is available
- WHEN its capabilities are queried
- THEN it MUST advertise support for Clankers-owned stdin, completed-record garbage collection, and process-group kill where supported by the host
- THEN it MUST report durable queueing, dependency scheduling, and post-crash stdout pipe recovery as unsupported unless implemented separately

#### Scenario: pueue backend advertises queue support [r[process-job-backend-capability-matrix.backends.pueue]]

- GIVEN pueue integration is configured and available
- WHEN its capabilities are queried
- THEN it MUST advertise queue/group behavior supported by pueue
- THEN it MUST report unsupported interactive stdin or resource-limit features that pueue cannot guarantee through Clankers
- THEN unavailable pueue service state MUST be reflected as `backend_unavailable`

#### Scenario: systemd backend advertises supervisor support [r[process-job-backend-capability-matrix.backends.systemd]]

- GIVEN systemd integration is configured on a systemd host
- WHEN its capabilities are queried
- THEN it MUST advertise supported transient unit/scope, control-group kill, journald log reference, and resource-limit features
- THEN it MUST report unsupported interactive stdin or queue dependency behavior unless explicitly implemented
- THEN non-systemd hosts MUST expose systemd as unavailable rather than falling back silently

### Requirement: Unsupported actions fail before mutation [r[process-job-backend-capability-matrix.errors]]

The system MUST validate requested operations against backend capabilities before contacting a backend for mutation.

#### Scenario: unsupported action returns typed receipt [r[process-job-backend-capability-matrix.errors.unsupported-action]]

- GIVEN a caller requests an action unsupported by the selected backend
- WHEN Clankers validates the request
- THEN it MUST return `unsupported_action_for_backend` with backend, action, and capability detail fields
- THEN it MUST NOT partially perform backend mutation before returning the error

### Requirement: Safe capability projection [r[process-job-backend-capability-matrix.projection]]

The system MUST project backend capabilities safely to agents, TUI, and daemon clients without exposing sensitive backend configuration.

#### Scenario: list/status includes safe capability hints [r[process-job-backend-capability-matrix.projection.safe]]

- GIVEN a caller lists or inspects jobs
- WHEN Clankers includes backend capability data
- THEN it MUST include safe operation hints such as whether kill, restart, stdin, logs, and resource limits are supported
- THEN it MUST NOT expose sensitive service paths, environment, or raw backend configuration in the projection
