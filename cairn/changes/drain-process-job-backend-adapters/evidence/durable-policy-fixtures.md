# Durable policy fixture evidence

Evidence-ID: durable-policy-fixtures
Artifact-Type: command-output-summary
Task-ID: V2
Covers: process-job-backend-adapters.durable-policy.no-root-tool
Date: 2026-06-02
Status: PASS

## Commands

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers durable_policy_helpers_project
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers tools::process::
./scripts/check-process-job-boundary.rs
./scripts/check-lego-architecture-boundaries.rs
```

## Relevant output

```text
PASS clankers tools::process::durable::tests::durable_policy_helpers_project_redacted_reconciliation_gc_and_notifications_without_root_tool
Summary: 48 tests run: 48 passed, 1488 skipped
ok: process-job boundary rail passed
lego architecture dependency ownership inventory written to target/lego-architecture/dependency-ownership-inventory.json
```

## Coverage notes

The focused durable policy test calls `durable::apply_process_job_retention`, `durable::reconcile_durable_native_process_jobs`, `durable::stored_record_summary`, `durable::append_log_degradation`, `durable::durable_degraded_log_message`, and `durable::evaluate_process_entry_notification` directly. It does not construct `ProcessTool`, a TUI, a daemon actor, pueue, systemd, or a live host process backend. Assertions cover typed GC receipts, log-reference degradation, durable reconciliation state, notification event delivery, and redaction of `raw-token` in projected summaries/events.
