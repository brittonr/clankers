# Process-job shell ports evidence

Evidence-ID: trait-seam-refactor-roadmap.process-job-shell-ports
Artifact-Type: command-output-summary
Task-ID: V5
Covers: remaining-coupling-drain.trait-seam-refactors.process-job-shell-ports
Date: 2026-06-06
Status: PASS

## Implementation summary

- Added `ProcessJobCommandRunner` and `TokioProcessJobCommandRunner` in `src/tools/process.rs` as a shared command-execution shell port.
- Updated pueue and systemd CLI runners to delegate process spawning/error projection to the shared port while preserving backend-specific command construction, status parsing, capabilities, retention, notification, redaction, and durable policy ownership.
- Existing pueue/systemd fake runner seams remain backend-specific test doubles above the shared shell command port.

## Commands completed

```text
env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' cargo test --lib backend_projects
env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' cargo test --lib process_job_service
env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' cargo test --lib durable_policy_helpers_project_redacted_reconciliation_gc_and_notifications_without_root_tool
```

## Relevant output

```text
running 2 tests
test tools::process::tests::pueue_backend_projects_status_logs_and_mutations_without_hard_seam ... ok
test tools::process::tests::systemd_backend_projects_transient_units_logs_and_mutations_without_hard_seam ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 1047 filtered out; finished in 0.00s
exit=0

running 5 tests
test tools::process::native::tests::native_process_job_service_gc_requires_db_and_rejects_foreign_backend_filter ... ok
test tools::process::native::tests::native_process_job_service_garbage_collects_completed_native_records ... ok
test tools::process::native::tests::native_process_job_service_preserves_default_start_list_wait_flow ... ok
test tools::process::tests::native_process_job_service_redacts_receipts_and_persisted_metadata ... ok
test tools::process::native::tests::native_process_job_service_restarts_running_entry ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 1044 filtered out; finished in 0.11s
exit=0

running 1 test
test tools::process::durable::tests::durable_policy_helpers_project_redacted_reconciliation_gc_and_notifications_without_root_tool ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1048 filtered out; finished in 0.04s
exit=0
```
