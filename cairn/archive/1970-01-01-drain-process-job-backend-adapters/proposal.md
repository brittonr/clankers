# Change: Drain Process Job Backend Adapters

## Problem

`src/tools/process.rs` is still a root-shell hotspot. The agent-visible `process` tool now parses typed requests, but the same root file still owns native process state, pueue CLI projection, systemd projection, durable reconciliation, retention/GC, and receipt formatting. That makes reusable process-job behavior hard to test without the root crate and encourages future backend policy to land in the product shell.

## Goals

- Keep the root `process` tool as JSON parsing, backend selection, service wiring, and typed receipt projection only.
- Move backend-specific native, pueue, and systemd policy into named adapters behind `clankers-runtime::process_jobs` service contracts or focused edge modules.
- Move durable storage reconciliation, retention, and log-degradation policy behind typed process-job service helpers with focused tests.
- Add architecture rails that fail when reusable backend/storage policy is reintroduced into `src/tools/process.rs`.

## Non-goals

- Do not change the user-facing `process` tool schema or receipts except to add safe typed capability details where already supported.
- Do not require pueue or systemd to be installed for deterministic tests.
- Do not collapse all process-job work into a new crate in one step; adapters may remain product-edge modules if reusable policy is owned by runtime contracts.

## Proposed scope

Extract the current native, pueue, systemd, durable-record, retention, and notification clusters one at a time. Each extraction should leave a narrow call path from `ProcessTool` to `ProcessJobService`/backend adapters, and each adapter should have a fake-runner fixture that proves behavior without spawning real shell services.

The first slice should be an ownership map/source rail before extraction. That map should make `src/tools/process.rs` accountable only for request parsing, backend selection, service wiring, and final receipt projection, with every backend/storage policy cluster assigned to a named owner.

## Verification

Focused validation should include process-job unit fixtures, fake pueue/systemd runner fixtures, durable retention/reconciliation fixtures, the lego architecture boundary rail, `cargo check --tests` for touched crates, Cairn gates, and `git diff --check`.
