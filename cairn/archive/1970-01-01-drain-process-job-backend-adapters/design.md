# Design: Drain Process Job Backend Adapters

## Summary

This change continues the process-job decoupling by making the root tool a projection over typed backend services. The reusable process-job contracts stay in `clankers-runtime::process_jobs`; concrete host integrations may live in focused root-edge modules while their policy is covered by service-level tests.

This is the process-tool counterpart to `neutral-tool-service-context`: neutral services shrink the agent/tool execution surface, while this change shrinks the root `process` tool itself into backend selection and receipt projection. The two changes can proceed independently, but both should keep reusable policy out of root/TUI/daemon assembly.

## Initial inventory targets

I1 should identify the owner for each policy cluster currently living in or reachable from `src/tools/process.rs`:

- native backend: registry/admission/start/stop/restart/adopt/reconciliation;
- pueue backend: command construction, status/list parsing, log projection, unavailable degraded receipts;
- systemd backend: unit construction, show/list/log parsing, unsupported degraded receipts;
- durable storage: record identity, reconciliation, retention/GC, log degradation, redaction;
- notifications: completion/failure delivery, notification suppression, redacted receipts.

## Decisions

### 1. Extract by backend/service cluster

Move one cluster at a time: native process registry/admission/termination, pueue command/status/log projection, systemd unit/show/list projection, durable record reconciliation, retention/GC, and notification delivery. Each cluster gets a named owner module and focused tests before the next cluster moves.

### 2. Keep runners fakeable

Pueue and systemd adapters should depend on small runner traits rather than invoking `Command` inline in business logic. Tests should use fake runner output for task lists, logs, failures, and unsupported operations.

### 3. Runtime owns reusable policy; root owns host wiring

Redaction, identity, capability descriptors, retention decisions, durable reconciliation, and typed receipt helpers belong with runtime process-job contracts. Root-edge modules may launch native processes or call host CLIs, but they should call runtime helpers for policy decisions.

### 4. Source rail enforces monolith shrinkage

The architecture rail should inspect `src/tools/process.rs` and named adapter modules. It should allow parsing/wiring/projection in the root tool and reject backend parser/state/storage policy being reintroduced there.

## Validation plan

- Unit fixtures for each extracted backend adapter using fake runners or in-memory registries.
- Runtime process-job fixtures for redaction, retention, durable reconciliation, and unsupported-action receipts.
- Source-boundary inventory that names the owner for native, pueue, systemd, durable, retention, and notification clusters.
- Existing process tool behavior tests and broad `cargo check --tests` after each extraction slice.
