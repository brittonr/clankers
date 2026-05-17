## Phase 1: Storage and contract foundation

- [x] [serial] [covers=durable-process-jobs.interfaces.tool-service-boundary] Define backend-neutral `ProcessJobService`, `ProcessJobBackend`, store, log store, notification sink, and projection traits/DTOs before wiring concrete backends. ✅ completed: 2026-05-17T05:39:45Z; evidence: `cargo test -p clankers-runtime process_jobs -- --nocapture`, `cargo check -p clankers-runtime --tests`, `openspec validate add-durable-process-jobs --strict --json`, `git diff --check`.
- [ ] [serial] [covers=durable-process-jobs.redb.metadata-log-reference] Define process/job record DTOs, status vocabulary, redaction rules, and redb table schema in `clankers-db`; include migration and unknown-future-version fixture coverage.
- [ ] [parallel] [covers=durable-process-jobs.receipts.typed] Define structured receipt/error DTOs for start/list/poll/log/wait/kill/restart/stdin/adopt/GC with stable id, backend kind, typed status, log refs, and unsupported-action codes.
- [ ] [parallel] [covers=durable-process-jobs.identity-ownership.stable-id] Define stable Clankers id, backend reference, owner/session/workspace scope, and cross-session capability model.
- [ ] [parallel] [covers=durable-process-jobs.logs.bounded] Define bounded log reference/chunk/cursor DTOs and native append-only log file retention behavior without storing unbounded output in redb.
- [ ] [parallel] [covers=durable-process-jobs.backends.native-compat] Inventory current `process` tool semantics and write compatibility expectations for shell/direct start, poll/log/wait/kill/stdin/close, and dangerous-command policy.
- [ ] [serial] [depends:durable-process-jobs.interfaces.tool-service-boundary] Add fake store/backend/log/notification sink contract tests that prove tool/service/backend/storage/projection boundaries remain decoupled.
- [ ] [serial] [depends:durable-process-jobs.redb.metadata-log-reference] Add a storage facade that wraps redb access through `Db::blocking(...)` for async tool/controller paths.

## Phase 2: Native durable backend

- [ ] [serial] [depends:phase-1] Extract a backend-neutral process/job service and backend trait boundary while keeping the current native backend behavior as the default path.
- [ ] [serial] [covers=durable-process-jobs.registry.finished-history] Persist native process start/status/completion metadata and log references; make `process list/log/poll` use durable state where available.
- [ ] [serial] [covers=durable-process-jobs.registry.restart-reattach] Implement daemon-start reconciliation for native process records, including running/reattached, `reattached-log-incomplete`, exited, and `lost-after-restart` outcomes.
- [ ] [parallel] [covers=durable-process-jobs.admission.native-limit] Enforce native concurrency/admission limits before spawn, returning typed reject/queue receipts without silently exceeding policy.
- [ ] [parallel] [covers=durable-process-jobs.lifecycle.detach] Ensure agent cancellation, client detach, log-follow stop, and notification unsubscribe do not kill a job unless explicit kill/cancel is authorized.
- [ ] [parallel] [covers=durable-process-jobs.lifecycle.kill-escalation] Implement explicit graceful-kill/escalation semantics and typed terminal outcomes for native process groups.
- [ ] [parallel] [covers=durable-process-jobs.capabilities.observe-only] Add capability checks that permit observe/log separately from start/kill/restart/stdin mutation.
- [ ] [parallel] [covers=durable-process-jobs.capabilities.backend-stdin] Add explicit authorization checks for stdin and non-native backend selection before any backend mutation occurs.

## Phase 3: Durable backend projections

- [ ] [serial] [depends:phase-2] Add fake backend contract tests for list/poll/log/kill/restart/status projection, backend capability matrix, and unsupported-backend/action receipts.
- [ ] [serial] [covers=durable-process-jobs.backends.pueue] Implement pueue backend integration behind availability/config checks, projecting pueue task ids, statuses, logs, groups, and restarts into Clankers receipts.
- [ ] [serial] [covers=durable-process-jobs.backends.systemd] Implement systemd backend integration behind availability/config checks, projecting transient units/scopes, cgroup kill/restart, and journal/log references into Clankers receipts.
- [ ] [parallel] [covers=durable-process-jobs.notifications-delivery.reattach] Persist notification events through the notification sink so detached sessions can receive completion/readiness state on authorized reattach.
- [ ] [parallel] [covers=durable-process-jobs.logs.notifications] Add `notify_on_complete` and bounded `watch_patterns` support with rate limits, one-shot completion delivery, rare readiness-signal delivery, and deterministic tests for noisy pattern suppression.
- [ ] [parallel] [covers=durable-process-jobs.adoption.identity-check] Add optional adoption/import service methods for PID, pueue task id, and systemd unit name with fail-closed identity/capability checks.

## Phase 4: NixOS and TUI integration

- [ ] [serial] [covers=nixos-process-job-config.module.persistence] Extend `nix/modules/clankers-daemon.nix` with process/job persistence options, directory/log configuration, database path wiring, retention/GC policy, and hardening write-path updates.
- [ ] [serial] [covers=nixos-process-job-config.module.pueue] Add optional pueue service/config integration and deterministic module eval tests for groups/concurrency and disabled fallback behavior.
- [ ] [serial] [covers=nixos-process-job-config.module.systemd-limits] Add systemd backend resource-limit options and validation/eval tests for memory, CPU, runtime, writable paths, and kill grace period.
- [ ] [parallel] [covers=durable-process-jobs.project-profiles.resolve] Add validated project job profile parsing that resolves named profiles into backend-neutral `ProcessJobSpec` values without backend-specific execution from config code.
- [ ] [parallel] [covers=durable-process-jobs.receipts.projection] Update process/job TUI/procmon/daemon projection data so native, pueue, and systemd backends appear in one bounded active/completed view with backend labels and shared DTOs.
- [ ] [parallel] [covers=durable-process-jobs.retention.completed-gc] Implement retention/GC command and automatic policy enforcement with typed receipts for removed metadata/logs, skipped active jobs, and failures.

## Phase 5: Verification and rollout

- [ ] [serial] [depends:phase-4] Add restart/reconciliation integration coverage using a temp redb/log dir and a long-lived test child; verify stable id, log reference, and honest lost/reattached status.
- [ ] [serial] [depends:phase-4] Add detach/reattach notification delivery tests covering persisted completion events, multi-client event ids, and deduplication behavior.
- [ ] [serial] [depends:phase-4] Add or refresh NixOS VM smoke coverage for process persistence enabled, daemon restart, list/log status, retention paths, and service directory/hardening behavior where practical.
- [ ] [parallel] [depends:phase-4] Update README/help text for durable `process` backend selection, typed receipts/errors, pueue/systemd availability errors, retention/GC, project job profiles, and NixOS module options.
- [ ] [serial] [depends:phase-5] Run focused checks: redb store tests, service-boundary fake tests, process tool/backend tests, capability/admission tests, notification sink tests, Nix module eval tests, and TUI/procmon DTO tests.
- [ ] [serial] [depends:phase-5] Run final gates: `cargo fmt --check`, `cargo nextest run` or focused workspace test set agreed during implementation, relevant NixOS module/VM checks, `openspec validate add-durable-process-jobs --strict --json`, and `git diff --check`.
