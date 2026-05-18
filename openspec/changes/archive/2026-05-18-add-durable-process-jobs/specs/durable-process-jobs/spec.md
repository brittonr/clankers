## ADDED Requirements

### Requirement: Durable process and job registry [r[durable-process-jobs.registry]]

The system MUST persist safe metadata for agent-started long-running processes and jobs so users can list, inspect, log, and reconcile them after daemon restart without relying on an in-memory registry alone.

#### Scenario: process handle survives daemon restart [r[durable-process-jobs.registry.restart-reattach]]

- GIVEN the agent starts a long-running native process through the `process` tool
- AND the process is still running when the Clankers daemon exits or restarts
- WHEN the daemon starts again and opens the process registry
- THEN it MUST recover the process handle metadata with the original stable process/job identifier
- THEN `process list` MUST report the recovered process as running or reattached when the OS process group can be verified
- THEN missing or unverifiable OS processes MUST be reported as `lost-after-restart` rather than silently omitted

#### Scenario: finished process remains inspectable [r[durable-process-jobs.registry.finished-history]]

- GIVEN an agent-started process exits while the daemon is running
- WHEN the user later lists or inspects recent process history
- THEN Clankers MUST return the exit status, elapsed time, backend identifier, safe command preview, and log reference until retention removes it
- THEN it MUST NOT require scanning unbounded in-memory output to answer the query

### Requirement: redb-backed process metadata [r[durable-process-jobs.redb]]

The system MUST store process/job metadata in the existing `clankers-db` redb database using versioned typed records and explicit migration/fallback behavior.

#### Scenario: metadata uses redb and bounded log references [r[durable-process-jobs.redb.metadata-log-reference]]

- GIVEN a process or durable job is started
- WHEN Clankers persists its registry entry
- THEN it MUST write typed redb metadata including schema version, stable id, backend kind, command preview, cwd policy, process group or backend unit/task id, timestamps, status, resource policy, and log file identifiers
- THEN it MUST store large stdout/stderr in bounded append-only log files or backend log references rather than unbounded redb values
- THEN redb records MUST contain no raw secrets, headers, or unredacted environment values

#### Scenario: database migration is safe [r[durable-process-jobs.redb.migration]]

- GIVEN an existing Clankers database has no process/job tables or has an older process/job schema
- WHEN Clankers opens the database
- THEN migration MUST create or upgrade the process/job tables without breaking existing audit, memory, session, history, usage, registry, or tool-result tables
- THEN unknown future process/job schema versions MUST be skipped or projected safely instead of causing daemon startup failure

### Requirement: Process/job backend abstraction [r[durable-process-jobs.backends]]

The system MUST route long-running execution through a backend abstraction that preserves the current native process behavior while enabling durable queue/supervisor backends.

#### Scenario: native backend preserves current process tool semantics [r[durable-process-jobs.backends.native-compat]]

- GIVEN a caller uses existing `process` actions with no backend override
- WHEN the native backend is selected
- THEN shell and direct-exec start modes, `poll`, `log`, bounded `wait`, `kill`, `write`, `submit`, and `close` MUST continue to work compatibly
- THEN dangerous shell commands MUST remain blocked or confirmed according to the existing bash safety policy

#### Scenario: pueue backend exposes durable queue state [r[durable-process-jobs.backends.pueue]]

- GIVEN pueue integration is enabled and available
- WHEN the agent starts a process/job with backend `pueue`
- THEN Clankers MUST create or map to a pueue task with stable Clankers metadata
- THEN list/poll/log/kill/restart MUST project pueue task status and logs into Clankers process/job receipts
- THEN backend-unavailable errors MUST be explicit and non-destructive

#### Scenario: systemd backend exposes supervised units [r[durable-process-jobs.backends.systemd]]

- GIVEN systemd integration is enabled on a systemd host
- WHEN the agent starts a process/job with backend `systemd`
- THEN Clankers MUST create or map to a transient unit or scope with stable Clankers metadata
- THEN kill/restart/log/status MUST operate on the unit or scope and its control group rather than only the launcher PID
- THEN non-systemd hosts MUST fail closed with a clear unsupported-backend receipt

### Requirement: Unified log and notification model [r[durable-process-jobs.logs]]

The system MUST provide bounded, safe, backend-neutral log access and completion/readiness notifications for long-running processes and jobs.

#### Scenario: logs are bounded and stream-compatible [r[durable-process-jobs.logs.bounded]]

- GIVEN a long-running process emits stdout and stderr for an extended period
- WHEN a user or agent calls `poll` or `log`
- THEN Clankers MUST return bounded incremental or ranged log chunks with stream labels and total/range metadata
- THEN full output MUST remain accessible by log reference while retention permits
- THEN output truncation MUST be explicit in the receipt

#### Scenario: readiness and completion notifications are rate-limited [r[durable-process-jobs.logs.notifications]]

- GIVEN a process/job is started with `notify_on_complete = true`
- WHEN the process/job reaches a terminal status
- THEN Clankers MUST emit exactly one completion notification with process/job id, backend kind, exit status, elapsed time, and safe log excerpt/reference
- THEN the agent/session MUST be able to continue other work before that terminal notification arrives
- GIVEN a process/job is started with bounded `watch_patterns`
- WHEN output matches a configured readiness pattern
- THEN Clankers MUST emit a rare readiness notification with process/job id, matched pattern identity, and safe log excerpt/reference
- THEN noisy repeated matches MUST be rate-limited and eventually suppressed or downgraded to final completion notification behavior
- THEN watch-pattern notifications MUST NOT be required for normal completion delivery

### Requirement: NixOS process/job configuration [r[nixos-process-job-config.module]]

The NixOS module MUST expose declarative options for process/job persistence, backend selection, logs, retention, and resource limits without requiring users to hand-write service units for the common Clankers daemon setup.

#### Scenario: module configures persistence and logs [r[nixos-process-job-config.module.persistence]]

- GIVEN `services.clankers-daemon.processManagement.enable = true`
- WHEN the NixOS system is built
- THEN the module MUST create required state/log directories with daemon ownership
- THEN it MUST pass explicit environment/config to the daemon for redb path, process registry persistence, log directory, retention limits, and selected default backend
- THEN hardening MUST preserve daemon access to configured writable paths

#### Scenario: module configures optional pueue integration [r[nixos-process-job-config.module.pueue]]

- GIVEN the module enables a pueue backend
- WHEN the system is built
- THEN it MUST install/configure the required pueue service or clearly require an existing user service
- THEN configured groups and concurrency limits MUST be materialized deterministically
- THEN disabling pueue MUST remove Clankers reliance on pueue without breaking native backend operation

#### Scenario: module configures systemd resource limits [r[nixos-process-job-config.module.systemd-limits]]

- GIVEN the module enables the systemd backend with default process/job limits
- WHEN Clankers starts a systemd-backed job
- THEN the job MUST receive configured limits such as memory, CPU quota, runtime maximum, writable paths, working directory, and kill grace period where supported
- THEN unsupported options MUST be rejected at configuration or startup rather than ignored silently

### Requirement: Long-running process capabilities [r[durable-process-jobs.capabilities]]

The system MUST expose explicit capability classes for long-running process/job operations so local and remote sessions can authorize read-only observation separately from mutation and execution.

#### Scenario: observe-only peer cannot mutate jobs [r[durable-process-jobs.capabilities.observe-only]]

- GIVEN a remote or capability-scoped session has only process/job observe permissions
- WHEN it lists processes or reads bounded logs
- THEN Clankers MAY return safe metadata and redacted log chunks
- WHEN it attempts to start, kill, restart, or write stdin
- THEN Clankers MUST deny the request with a capability error

#### Scenario: backend and stdin require explicit authorization [r[durable-process-jobs.capabilities.backend-stdin]]

- GIVEN a session has permission to start short native commands but not durable backend or stdin permissions
- WHEN it requests a pueue/systemd backend or attempts `write`/`submit`
- THEN Clankers MUST deny the missing ability without starting or mutating the target process/job

### Requirement: Decoupled process/job interfaces [r[durable-process-jobs.interfaces]]

The system MUST expose durable process/job behavior through backend-neutral interfaces and typed DTOs so tool handlers, TUI views, daemon transports, storage, notification delivery, and backend implementations remain independently testable and replaceable.

#### Scenario: tool API depends on a backend-neutral service [r[durable-process-jobs.interfaces.tool-service-boundary]]

- GIVEN the `process` tool receives a start, list, poll, log, kill, restart, stdin, or close request
- WHEN it executes the request
- THEN it MUST call a `ProcessJobService`/backend-neutral interface using typed request DTOs rather than directly invoking native child, pueue, systemd, redb, or TUI code from the tool parser
- THEN request parsing, capability validation, backend dispatch, persistence, and presentation MUST be separable enough to test with fake backends and fake stores

#### Scenario: backend implementations do not own UI or storage policy [r[durable-process-jobs.interfaces.backend-boundary]]

- GIVEN native, pueue, or systemd backend code starts or observes a job
- WHEN it returns state to Clankers
- THEN it MUST return backend-neutral receipts plus explicit backend detail fields
- THEN it MUST NOT write TUI widgets, daemon session messages, or redb records directly except through injected interfaces owned by the service layer

#### Scenario: notification delivery is behind a sink interface [r[durable-process-jobs.interfaces.notification-sink]]

- GIVEN a job emits completion or readiness events
- WHEN notification policy decides an event should be delivered
- THEN delivery MUST go through a notification sink interface that can target attached TUI clients, daemon event streams, persisted session replay, or future bridges without coupling backends to those transports

### Requirement: Stable IDs, ownership, and session scope [r[durable-process-jobs.identity-ownership]]

The system MUST assign stable Clankers process/job identifiers and record ownership/scope metadata so durable jobs can be safely observed and mutated across daemon restarts, sessions, and remote clients.

#### Scenario: Clankers ID remains stable across backend IDs [r[durable-process-jobs.identity-ownership.stable-id]]

- GIVEN Clankers starts a native process, pueue task, or systemd unit
- WHEN backend-specific identifiers such as PID, pueue task id, or systemd unit name change or are reconciled
- THEN the Clankers process/job id MUST remain stable until retention removes the record
- THEN receipts MUST include both the stable Clankers id and safe backend reference when available

#### Scenario: ownership gates cross-session mutation [r[durable-process-jobs.identity-ownership.scope]]

- GIVEN a process/job belongs to a session, user, workspace, or daemon-global scope
- WHEN another session or remote peer attempts to list, read logs, kill, restart, or write stdin
- THEN Clankers MUST enforce configured ownership and capability policy before returning data or mutating the job
- THEN denials MUST identify the missing scope/capability without leaking secret command or log content

### Requirement: Structured receipt contract [r[durable-process-jobs.receipts]]

The system MUST return machine-readable receipts for process/job operations so agents, TUI, daemon clients, and future bridges do not parse human text to understand state.

#### Scenario: every operation returns typed status and references [r[durable-process-jobs.receipts.typed]]

- GIVEN a caller invokes start, list, poll, log, wait, kill, restart, write, submit, close, adopt, or garbage-collect operations
- WHEN Clankers returns a result or error
- THEN the receipt MUST include operation, stable id when applicable, backend kind, status, timestamps or elapsed time where known, log cursor/reference where applicable, capability/unsupported-backend errors as typed codes, and a bounded human summary
- THEN unsupported backend actions such as stdin on non-interactive backends MUST return `unsupported_action_for_backend` rather than silently succeeding or failing as generic text

#### Scenario: TUI and daemon projections share DTOs [r[durable-process-jobs.receipts.projection]]

- GIVEN process/job state is shown in TUI, returned to an agent, or sent over daemon attach/remote streams
- WHEN state is projected for that surface
- THEN each surface MUST derive from shared process/job DTOs or explicit projection adapters
- THEN backend-specific fields MUST remain optional details rather than changing the common schema per backend

### Requirement: Notification delivery persistence [r[durable-process-jobs.notifications-delivery]]

The system MUST deliver completion/readiness notifications exactly according to policy even when clients detach, reconnect, or multiple clients observe the same session.

#### Scenario: detached session receives persisted completion on reattach [r[durable-process-jobs.notifications-delivery.reattach]]

- GIVEN a process/job starts with `notify_on_complete = true`
- AND all clients detach before the job reaches a terminal status
- WHEN the job completes and a client later reattaches to the owning session or authorized scope
- THEN Clankers MUST expose the completion notification from persisted notification state or session replay
- THEN the notification MUST still include stable id, backend kind, terminal status, elapsed time, and safe log excerpt/reference

#### Scenario: multi-client delivery is deduplicated by event id [r[durable-process-jobs.notifications-delivery.dedup]]

- GIVEN multiple clients are attached to the same session or authorized scope
- WHEN a readiness or completion event is delivered
- THEN Clankers MUST assign a stable notification event id
- THEN each delivery target MUST be able to deduplicate the event without suppressing delivery to other authorized targets

### Requirement: Retention and garbage collection [r[durable-process-jobs.retention]]

The system MUST enforce configured retention for process/job metadata, logs, and notification history without deleting active job state or leaving unbounded orphaned files.

#### Scenario: completed job retention removes metadata and logs safely [r[durable-process-jobs.retention.completed-gc]]

- GIVEN completed process/job records and log files exceed configured age, count, or size limits
- WHEN retention runs automatically or by explicit GC request
- THEN Clankers MUST remove or tombstone eligible metadata and delete associated native log files or release backend log references
- THEN active/running jobs MUST NOT be removed by completed-job retention
- THEN GC receipts MUST report removed records, removed log bytes, skipped active jobs, and failures as typed fields

#### Scenario: missing logs degrade gracefully [r[durable-process-jobs.retention.missing-log]]

- GIVEN a metadata record points at a log file or backend cursor that retention or external cleanup removed
- WHEN a caller lists, polls, or logs the job
- THEN Clankers MUST return safe metadata with a typed `log_unavailable` or `log_retained_elsewhere` detail rather than failing the entire registry query

### Requirement: Admission control and resource policy [r[durable-process-jobs.admission]]

The system MUST apply queue/concurrency/resource admission rules before starting long-running work so configured limits are not silently bypassed.

#### Scenario: native concurrency limit is explicit [r[durable-process-jobs.admission.native-limit]]

- GIVEN the native backend has reached its configured concurrent process limit
- WHEN a caller requests another native process
- THEN Clankers MUST either reject the request with a typed `concurrency_limit_exceeded` receipt or enqueue it only if an explicit queue policy is configured
- THEN it MUST NOT start beyond the configured limit silently

#### Scenario: per-job resource overrides are bounded by policy [r[durable-process-jobs.admission.resource-ceiling]]

- GIVEN a caller requests memory, CPU, runtime, cwd, or writable-path overrides
- WHEN the request exceeds configured daemon/NixOS ceilings or lacks required capability
- THEN Clankers MUST reject the request before contacting the backend
- THEN accepted resource policy MUST be reflected in the start receipt and persisted metadata

### Requirement: Lifecycle, detach, and termination semantics [r[durable-process-jobs.lifecycle]]

The system MUST distinguish agent cancellation, client detach, notification unsubscribe, and process/job termination so long-running work is not killed accidentally.

#### Scenario: detaching from a job does not kill it [r[durable-process-jobs.lifecycle.detach]]

- GIVEN a long-running process/job is active
- WHEN the agent turn ends, a client detaches, or a caller stops following logs
- THEN Clankers MUST leave the job running unless an explicit kill/cancel operation with sufficient capability is issued
- THEN the job MUST remain listable by authorized scopes until completion and retention

#### Scenario: kill escalates predictably [r[durable-process-jobs.lifecycle.kill-escalation]]

- GIVEN a caller requests kill on a native, pueue, or systemd job
- WHEN graceful termination does not complete within configured grace period
- THEN Clankers MUST escalate according to backend capability, such as process group kill for native or control-group kill for systemd
- THEN receipts MUST distinguish `killed`, `failed_to_kill`, `orphaned`, and `unsupported_action_for_backend`

### Requirement: Adoption and external job import [r[durable-process-jobs.adoption]]

The system MUST keep any explicit adoption of external native processes, pueue tasks, or systemd units behind safety checks and backend-specific interface methods; adoption MAY be omitted for backends that cannot verify identity safely.

#### Scenario: adopted job passes identity checks [r[durable-process-jobs.adoption.identity-check]]

- GIVEN a caller requests adoption of an existing PID, pueue task id, or systemd unit name
- WHEN Clankers verifies backend identity, command/workdir ownership where available, and required capabilities
- THEN it MAY create a stable Clankers process/job record with status `adopted`
- THEN it MUST record that lifecycle/log guarantees may differ from Clankers-started jobs

#### Scenario: unsafe adoption fails closed [r[durable-process-jobs.adoption.fail-closed]]

- GIVEN an adoption request targets a missing, PID-reused, unauthorized, or unverifiable process/job
- WHEN Clankers evaluates the request
- THEN it MUST reject the request with a typed receipt and MUST NOT silently attach control to the target

### Requirement: Project job profiles [r[durable-process-jobs.project-profiles]]

The system MUST resolve any supported named project job profiles through validated configuration and backend-neutral specs so common long-running tasks can be started reproducibly without coupling project config to backend implementations.

#### Scenario: named profile resolves to backend-neutral start spec [r[durable-process-jobs.project-profiles.resolve]]

- GIVEN a project defines a named job profile such as `verify`, `nextest`, or `devServer`
- WHEN a caller starts that profile
- THEN Clankers SHOULD resolve it into the same backend-neutral `ProcessJobSpec` used by direct start requests
- THEN backend selection, resource policy, notification policy, cwd, environment policy, and capabilities MUST be validated before backend dispatch

#### Scenario: invalid profile is rejected before execution [r[durable-process-jobs.project-profiles.invalid]]

- GIVEN a project job profile contains an unsupported backend, disallowed writable path, unsafe environment entry, or resource value exceeding policy
- WHEN Clankers loads or starts the profile
- THEN it MUST reject the profile with typed validation errors and MUST NOT execute any command from it
