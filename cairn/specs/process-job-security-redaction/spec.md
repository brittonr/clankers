# process-job-security-redaction Specification

## Purpose
TBD - created by archiving change define-process-job-security-redaction. Update Purpose after archive.
## Requirements
### Requirement: Safe process/job metadata persistence [r[process-job-security-redaction.metadata]]

The system MUST persist only safe, bounded, redacted process/job metadata in redb and MUST keep raw secrets out of durable metadata records.

#### Scenario: command and environment are persisted as safe previews [r[process-job-security-redaction.metadata.safe-preview]]

- GIVEN a process/job start request includes command text, argv, cwd, environment, headers, or backend-specific launch fields
- WHEN Clankers persists process/job metadata
- THEN it MUST store only bounded safe previews, redacted summaries, policy identifiers, backend refs, and log refs
- THEN it MUST NOT store raw secret environment values, authorization headers, tokens, or unredacted full command strings in redb metadata

#### Scenario: metadata tests reject secret persistence [r[process-job-security-redaction.metadata.no-redb-secrets]]

- GIVEN fixture requests contain token-like values in argv, env, headers, or command strings
- WHEN records are serialized for redb
- THEN tests MUST assert those raw values do not appear in serialized metadata
- THEN safe replacement text such as `[REDACTED]` or omitted fields MUST be used instead

### Requirement: Capability-gated log access [r[process-job-security-redaction.logs]]

The system MUST treat process/job log bytes and excerpts as sensitive data and gate access separately from list/observe metadata.

#### Scenario: observe-only cannot read raw logs [r[process-job-security-redaction.logs.capability-gated]]

- GIVEN a client has process/job observe permission but lacks log-read permission
- WHEN it lists jobs or receives status receipts
- THEN Clankers MAY return safe metadata and redacted summaries
- WHEN it requests bounded or raw logs
- THEN Clankers MUST deny the request with a typed capability error

#### Scenario: raw log access requires explicit capability [r[process-job-security-redaction.logs.raw-access]]

- GIVEN a client requests raw or minimally redacted stdout/stderr content
- WHEN the request is evaluated
- THEN Clankers MUST require explicit raw-log capability and owner/scope authorization
- THEN denied requests MUST NOT leak partial log contents in the error receipt

### Requirement: Safe notification and receipt excerpts [r[process-job-security-redaction.notifications]]

The system MUST bound and redact any process/job log excerpt or command preview included in receipts, notifications, replay records, or TUI/daemon projections.

#### Scenario: notification excerpt is redacted before persistence [r[process-job-security-redaction.notifications.safe-excerpt]]

- GIVEN a completion or readiness notification includes a log excerpt
- WHEN the event is created or persisted for replay
- THEN Clankers MUST run centralized redaction and bounding before storing or delivering the excerpt
- THEN the event MUST include a log reference for authorized follow-up instead of embedding unbounded output

#### Scenario: replay does not restore raw secret excerpts [r[process-job-security-redaction.notifications.redacted-replay]]

- GIVEN a notification event is persisted while clients are detached
- WHEN an authorized client reattaches and receives replay
- THEN the replayed event MUST contain the same bounded redacted excerpt or safe reference
- THEN it MUST NOT reconstruct raw secret content from logs unless the client explicitly requests logs with sufficient capability

### Requirement: Centralized redaction interface [r[process-job-security-redaction.interfaces]]

The system MUST centralize process/job redaction behind an interface used by persistence, receipts, notification events, and projection adapters.

#### Scenario: backend adapters do not implement divergent redaction [r[process-job-security-redaction.interfaces.centralized]]

- GIVEN native, pueue, or systemd backend adapters return backend facts to the process/job service
- WHEN those facts are persisted or emitted publicly
- THEN Clankers MUST apply shared redaction helpers in the service/projection layer
- THEN backend-specific code MUST NOT define separate incompatible redaction behavior for public receipts or metadata
