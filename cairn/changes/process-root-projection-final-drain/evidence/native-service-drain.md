# Native service drain evidence

Evidence-ID: process-root-native-service-drain
Artifact-Type: command-output-summary
Task-ID: V1
Covers: process-root-projection-final-drain.verification
Date: 2026-06-02
Status: PASS

## Commands

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers --no-run
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers tools::process::
```

## Relevant output

```text
cargo test -p clankers --no-run
Finished `test` profile [optimized + debuginfo] target(s) in 56.35s

cargo nextest run -p clankers tools::process::
PASS clankers tools::process::native::tests::native_backend_in_memory_fixture_projects_list_poll_log_and_errors_without_spawning
PASS clankers tools::process::native::tests::native_pid_adoption_uses_metadata_only_receipt_and_fails_closed
PASS clankers tools::process::native::tests::native_process_job_service_garbage_collects_completed_native_records
PASS clankers tools::process::native::tests::native_process_job_service_gc_requires_db_and_rejects_foreign_backend_filter
PASS clankers tools::process::native::tests::native_process_job_service_preserves_default_start_list_wait_flow
PASS clankers tools::process::native::tests::native_process_job_service_restarts_running_entry
Summary: 48 tests run: 48 passed, 1488 skipped
```

## Coverage notes

`ProcessEntry`, `ProcessStatus`, native receipt/status helpers, and `NativeProcessJobService` now live under `src/tools/process/native.rs`. The native-service fixtures moved with that owner; root process tests still cover JSON parsing, backend selection, durable fallback, and compatibility projection.
