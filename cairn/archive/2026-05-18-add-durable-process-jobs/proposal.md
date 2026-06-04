## Why

Clankers currently has two useful but separate long-running execution paths: daemon-backed agent sessions and the agent-visible `process` tool. The `process` tool is good for servers, watchers, and long tests while a daemon is alive, but its registry is in-memory, stdout/stderr are memory-backed, and daemon restart or crash loses the user's handle on still-running work. Users also reach for external tools such as pueue for durable builds/tests, but Clankers does not expose a first-class backend abstraction or NixOS configuration for that workflow.

## What Changes

- **Durable process registry**: Persist agent-started process/job metadata and bounded log references so process handles survive daemon restart when the underlying OS process or backend job still exists.
- **redb-backed metadata**: Use the existing `clankers-db`/redb storage layer for typed process/job records, schema migration, and safe query/list behavior. Keep large stdout/stderr in append-only log files referenced from redb rather than storing unbounded logs inside redb.
- **Backend abstraction**: Introduce a process/job backend boundary that can support the current native child-process manager plus durable backends such as pueue and systemd transient units.
- **NixOS declarative support**: Extend the NixOS module with options for process persistence, log retention, backend selection, resource limits, and optional pueue/systemd integration.
- **Unified UX**: Surface native processes, durable jobs, and external backend state through consistent list/poll/log/kill/restart receipts and TUI process/job panel data.
- **Decoupled interfaces**: Keep tool parsing, service orchestration, backend adapters, redb metadata, log storage, notification sinks, and TUI/daemon projections behind explicit interfaces with fake-backed tests.
- **Structured ownership and receipts**: Define stable Clankers IDs, owner/session/workspace scope, typed receipts/errors, backend capability matrix, and unsupported-action behavior.
- **Retention and lifecycle policy**: Add retention/GC, detach-vs-kill semantics, admission/resource limits, and optional safe adoption/project job profiles.
- **Background completion/readiness signals**: Add first-class `notify_on_complete` and bounded `watch_patterns` semantics so agents and attached clients can keep working while long-running processes stream to logs and emit rare, actionable notifications.

## Capabilities

### New Capabilities

- `durable-process-jobs`: Durable management of long-running agent-started work across daemon restarts with safe logs, recovery, and backend abstraction.
- `nixos-process-job-config`: Declarative NixOS configuration for Clankers daemon process/job persistence and backend services.

### Modified Capabilities

- `typed-durable-session-ledger`: Process/job lifecycle facts should reuse safe typed persistence and redaction conventions rather than adding ad-hoc unversioned state.
- `effect-ability-runtime`: Long-running process/job operations require explicit capability classes for start, stdin, kill, logs, backend selection, and resource limits.

## Impact

- **Files likely affected**: `src/tools/process.rs`, `src/tools/procmon.rs`, `crates/clankers-procmon`, `crates/clankers-db`, `crates/clankers-runtime/src/tools.rs`, `crates/clanker-tui-types`, `crates/clankers-tui`, daemon attach/event DTOs, process/job service/backend modules, `nix/modules/clankers-daemon.nix`, and focused daemon restart/NixOS VM tests.
- **APIs**: The `process` tool keeps existing actions but gains durable IDs/statuses and optional backend/resource parameters; a future `jobs` alias/tool may wrap queue-specific semantics. Public surfaces should return typed process/job receipts from shared DTOs rather than backend- or UI-specific text.
- **Dependencies**: Prefer existing `redb` via `clankers-db`; optional runtime dependencies on pueue/systemd must be feature/config gated and fail clearly when unavailable.
- **Testing**: Add redb schema tests, service-interface tests with fake stores/backends/notification sinks, native recovery tests, backend contract tests with fakes, and NixOS module/VM checks for declarative service/log/resource configuration.
