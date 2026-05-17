## Context

Clankers already has useful ingredients for durable long-running work:

- `process` provides agent-visible start/list/poll/log/wait/kill/stdin actions, but stores registry and output in memory.
- `procmon` samples tool-spawned processes for TUI visibility, CPU/RSS, children, and history.
- `clankers-db` already uses redb for persistent typed stores and provides `Db::blocking(...)` so synchronous redb operations do not block async runtime threads.
- The NixOS module already runs `clankers daemon start` as a hardened systemd service with state/runtime directories.
- Users already use pueue for long builds/tests; systemd transient units are a natural NixOS-native supervisor for durable local jobs.

The design should avoid rebuilding a full scheduler inside Clankers. Clankers should own the agent UX, safety/capability policy, metadata, receipts, and backend-neutral projection while delegating queue/supervision to pueue or systemd where configured.

## Goals / Non-Goals

**Goals:**

- Preserve existing `process` tool compatibility for native short/medium child processes.
- Make process/job handles and safe metadata durable across daemon restart.
- Use redb via `clankers-db` for typed metadata and schema migration.
- Keep large logs out of redb; store file paths/backend log cursors/references in redb.
- Add a backend abstraction for native, pueue, and systemd implementations.
- Keep tool parsing, service orchestration, backend execution, storage, notification delivery, and TUI/daemon projections decoupled behind explicit interfaces.
- Add NixOS module options for persistence, backend choice, logs, pueue/systemd integration, and resource limits.
- Add explicit background notification semantics for completion and rare readiness patterns without requiring the agent to block on `wait` or repeatedly poll.
- Add stable ownership/session scope, structured receipts, retention/GC, admission control, lifecycle/detach semantics, optional adoption, and project job profiles.
- Add capability classes that distinguish observe/log from start/kill/stdin/backend mutation.

**Non-Goals:**

- Do not implement a distributed scheduler or cluster runner.
- Do not require pueue or systemd on non-NixOS/non-systemd hosts.
- Do not store unbounded stdout/stderr in redb.
- Do not bypass existing dangerous-command confirmation/sandbox policy.
- Do not replace session JSONL/automerge persistence in this change.
- Do not promise exact recovery of arbitrary orphaned processes that were never started/adopted by Clankers.

## Decisions

### Decision 1: redb stores metadata; logs stay append-only files or backend references

**Choice:** Add process/job tables to `clankers-db` for safe typed metadata, but write stdout/stderr to bounded append-only log files for native jobs and store backend log references for pueue/systemd jobs.

**Rationale:** redb is already in the dependency graph and gives transactional typed metadata with migration support. It is not the right place for unbounded streaming logs. Separating metadata from log bytes keeps queries fast, avoids database bloat, and makes retention predictable.

**Implementation:** Add a `process_jobs` store under `crates/clankers-db` with records roughly shaped as:

```rust
struct ProcessJobRecord {
    schema_version: u32,
    id: String,
    backend: BackendKind,
    command_preview: String,
    cwd: Option<PathPolicy>,
    status: ProcessJobStatus,
    started_at: SystemTime,
    updated_at: SystemTime,
    completed_at: Option<SystemTime>,
    os_pid: Option<u32>,
    process_group: Option<i32>,
    backend_ref: Option<String>,
    log_ref: LogRef,
    resource_policy: ResourcePolicy,
    capability_summary: CapabilitySummary,
}
```

Use `Db::blocking(...)` for writes/reads from async tool paths. Redact or omit raw env and full command if it may contain secrets; store a bounded command preview plus content-addressed/full log references only when safe.

### Decision 2: introduce service, storage, backend, notification, and projection interfaces before adding pueue/systemd behavior

**Choice:** Factor the current native implementation behind a backend-neutral service boundary first, then add fake-backed tests, pueue projection, and systemd projection. The tool parser should depend on a service interface, not concrete native/pueue/systemd/redb/TUI modules.

**Rationale:** The current `process` tool mixes tool parameter parsing, child spawning, registry state, log buffering, and kill semantics. Interface boundaries preserve compatibility while making backend behavior testable without live pueue/systemd services and preventing new backend code from coupling directly to UI, daemon transport, or redb internals.

**Implementation:** Prefer a functional-core interface split similar to:

```rust
trait ProcessJobService {
    async fn start(&self, request: StartJobRequest) -> Result<ProcessJobReceipt>;
    async fn list(&self, filter: ProcessJobFilter) -> Result<Vec<ProcessJobSummary>>;
    async fn poll(&self, id: ProcessJobId, cursor: LogCursor) -> Result<ProcessJobPoll>;
    async fn log(&self, id: ProcessJobId, range: LogRange) -> Result<LogChunk>;
    async fn kill(&self, id: ProcessJobId, mode: KillMode) -> Result<KillReceipt>;
    async fn restart(&self, id: ProcessJobId, mode: RestartMode) -> Result<ProcessJobReceipt>;
    async fn write_stdin(&self, id: ProcessJobId, data: Bytes, newline: bool) -> Result<WriteReceipt>;
    async fn close_stdin(&self, id: ProcessJobId) -> Result<CloseReceipt>;
    async fn adopt(&self, request: AdoptJobRequest) -> Result<ProcessJobReceipt>;
}

trait ProcessJobBackend {
    async fn start(&self, spec: ProcessJobSpec) -> Result<BackendStart>;
    async fn observe(&self, backend_ref: BackendRef) -> Result<BackendStatus>;
    async fn log(&self, backend_ref: BackendRef, range: LogRange) -> Result<LogChunk>;
    async fn kill(&self, backend_ref: BackendRef, mode: KillMode) -> Result<KillReceipt>;
    async fn capabilities(&self) -> BackendCapabilities;
}

trait ProcessJobStore { /* redb-backed metadata and notification event records */ }
trait ProcessJobLogStore { /* native append-only logs and backend log references */ }
trait ProcessJobNotificationSink { /* attached clients, daemon events, replay ledger */ }
trait ProcessJobProjection { /* agent/TUI/daemon DTO projections */ }
```

Keep backend-neutral DTOs in a small core module; keep native child management, pueue CLI/API calls, systemd calls, redb access, notification delivery, and TUI formatting in thin shell modules. Tests should be able to exercise `ProcessJobService` with fake backend/store/log/sink implementations.

### Decision 3: native recovery is reconciliation, not magic process resurrection

**Choice:** On daemon restart, native jobs are reconciled by checking persisted PID/process-group identity and marking records as running/reattached, exited, or `lost-after-restart`.

**Rationale:** A daemon cannot reattach to stdout pipes it no longer owns after a crash, and it cannot know final status for every orphan. Honest state is more valuable than pretending the original live stream is intact.

**Implementation:** For graceful daemon restarts, close/flush log files and update status. For crash recovery, verify the process group where possible; if live, expose status and kill support but mark log streaming as possibly incomplete after the last flushed offset. If not live, mark lost with last-known metadata.

### Decision 4: pueue and systemd are optional durable backends

**Choice:** Add optional backend implementations that project pueue tasks and systemd transient units/scopes into the same Clankers receipts.

**Rationale:** pueue is excellent for human-oriented durable queues. systemd is excellent for NixOS-native supervision, cgroups, resource limits, and journald logs. Clankers should use them rather than building all queue/supervisor features itself.

**Implementation:** Backend availability is detected/configured. Unsupported backend requests return structured errors. Tests use fake command runners; NixOS VM tests cover systemd module wiring and pueue service integration where practical.

### Decision 5: Background notifications are explicit one-shot/rare signals, not live context streaming

**Choice:** Add `notify_on_complete` for exactly-once terminal notifications and `watch_patterns` for rare readiness signals, while keeping continuous output in logs/poll streams rather than pushing every line into the active agent context.

**Rationale:** Long-running commands should not block the agent and should not spam the conversation. Completion is always useful as a one-shot signal; readiness patterns are useful only for rare events such as server startup. Normal output belongs in bounded logs that the user/agent can pull when needed.

**Implementation:** Store notification policy with the process/job record. Terminal backend status triggers one completion delivery. Pattern matches use rate limits and suppression after repeated noisy windows. If `notify_on_complete` and `watch_patterns` conflict, completion delivery remains authoritative and watch delivery is best-effort/rate-limited.

### Decision 6: receipts, notifications, retention, and ownership are service-layer policy

**Choice:** The service layer owns stable Clankers IDs, session/user/workspace ownership, structured receipt construction, retention/GC decisions, notification policy, and admission checks. Backends only expose capabilities and backend facts.

**Rationale:** Durable jobs cross UI, daemon, remote attach, storage, and backend boundaries. If ownership, notification, retention, or receipt logic lives inside a backend, adding pueue/systemd will fork semantics and make security review difficult.

**Implementation:** Persist owner scope, backend reference, notification event ids, retention class, and accepted resource policy in metadata. Run capability/admission checks before backend dispatch. Deliver notifications through `ProcessJobNotificationSink`, with persisted event records for detach/reattach replay. Use typed receipt DTOs for all public surfaces and projection adapters for TUI/daemon text.

### Decision 7: NixOS module owns declarative defaults, not every per-job policy

**Choice:** The NixOS module configures daemon-level defaults and backend services; per-job overrides are validated at runtime against those defaults and capabilities.

**Rationale:** Nix is ideal for reproducible daemon/service configuration, directory ownership, service hardening, and default resource policies. Individual agent jobs still need runtime parameters based on the current task.

**Implementation:** Extend `services.clankers-daemon` with a `processManagement` subtree and optional `jobs.groups`/resource default options. The module sets env/config paths and creates tmpfiles/read-write paths. Runtime rejects per-job overrides that exceed configured ceilings or missing capabilities. Project job profiles, if added, should compile to the same backend-neutral `ProcessJobSpec` as direct tool requests rather than invoking backends directly.

## NixOS Option Sketch

```nix
services.clankers-daemon = {
  processManagement = {
    enable = true;
    backend = "native"; # "native" | "pueue" | "systemd"
    persistMetadata = true;
    databasePath = "/var/lib/clankers/.clankers/agent/clankers.db";
    logDir = "/var/log/clankers/processes";
    maxLogBytes = "256M";
    retentionDays = 14;
    defaultTimeout = "2h";
    killGracePeriod = "10s";
  };

  jobs = {
    pueue.enable = false;
    groups = {
      build.parallel = 1;
      test.parallel = 2;
      io.parallel = 4;
    };
  };

  limits = {
    maxConcurrentProcesses = 8;
    defaultMemoryMax = "8G";
    defaultCPUQuota = "400%";
  };
};
```

Exact option names may change during implementation, but final names must be documented and covered by module tests.

## Risks / Trade-offs

**redb blocking async runtime** → Use `Db::blocking(...)` and focused tests that process tool paths do not perform direct synchronous redb I/O on hot async paths.

**Database bloat from logs** → Store only metadata and log references in redb; enforce log rotation/retention outside redb.

**Backend semantic mismatch** → Define backend-neutral receipts with backend-specific detail fields rather than pretending pueue/systemd/native expose identical capabilities.

**Recovery overclaiming** → Use explicit statuses such as `lost-after-restart` and `reattached-with-incomplete-log` when exact state cannot be proven.

**NixOS hardening blocking logs/state** → Module tests must verify `ReadWritePaths`, `StateDirectory`, `LogsDirectory`/tmpfiles, and service user ownership match configured paths.

**Capability bypass via durable backend** → Route all start/kill/stdin/restart/backend selection through the runtime capability gate before contacting the backend.

## Validation Plan

- `openspec validate add-durable-process-jobs --strict --json`
- Unit tests for redb process/job store migration, record insertion/update/query, and unknown-future-version fallback.
- Native backend compatibility tests for existing `process` actions.
- Native restart/reconciliation tests using a temp db/log dir and a long-lived test child.
- Fake backend contract tests for pueue/systemd projection semantics and unsupported-backend errors.
- Capability tests for observe-only vs start/kill/stdin/backend permissions.
- NixOS module eval tests for default/native config, pueue-enabled config, systemd backend limits, directory ownership, and hardening paths.
- If practical, one NixOS VM smoke that starts the daemon with process persistence enabled, starts a durable job, restarts daemon, and verifies list/log status.
