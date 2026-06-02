# Backend fixture evidence

Evidence-ID: process-backend-fixtures
Artifact-Type: command-output-summary
Task-ID: V1
Covers: process-job-backend-adapters.verification.backend-fixtures
Date: 2026-06-02
Status: PASS

## Commands

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers native_backend_in_memory_fixture
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers durable_policy_helpers_project
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers tools::process::
```

## Relevant output

```text
PASS clankers tools::process::native::tests::native_backend_in_memory_fixture_projects_list_poll_log_and_errors_without_spawning
PASS clankers tools::process::durable::tests::durable_policy_helpers_project_redacted_reconciliation_gc_and_notifications_without_root_tool
Summary: 48 tests run: 48 passed, 1488 skipped
```

## Coverage notes

The native fixture constructs an in-memory `ProcessEntry` and exercises `NativeProcessJobService` list, poll, log, unsupported-start, unknown-id, and stdin-error paths without spawning a live native process. Existing fake-runner tests cover pueue and systemd start, list, poll, log, kill, restart, adoption, disabled, and unavailable paths through `PueueProcessJobService` and `SystemdProcessJobService` without requiring live host services.
