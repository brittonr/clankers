# Native Process GC Review Closeout 2026-05-27

Artifact-Type: oracle-checkpoint
Task-ID: done-review-native-process-gc-2026-05-27
Covers: process-job-backend-capability-matrix.descriptor, process-job-backend-capability-matrix.backends.native, process-job-retention-gc.receipts.typed, durable-process-jobs.retention.completed-gc
Question: Does native process GC require capability-matrix spec alignment, and is the Cairn validation blocker resolved?
Owner: Clankers maintainer/operator (`brittonr`)
Reviewed-Evidence: `crates/clankers-runtime/src/process_jobs.rs`, `src/tools/process.rs`, `docs/src/reference/process-jobs.md`, `cairn/specs/process-job-backend-capability-matrix/spec.md`, `cairn/specs/cairn-policy-schema-compat/spec.md`, `cairn-policy/generated/cairn-policy.json`, visible command transcript from 2026-05-27
Decision: Accepted. Adding `supports_garbage_collect` to `ProcessJobBackendCapabilities` changes the backend capability matrix, so the capability-matrix spec must explicitly mention garbage collection. The spec now requires typed support information for garbage collection and states that the native backend advertises completed-record garbage collection. The Cairn generated-policy schema blocker is resolved by adding `steel_orchestration_policy` to `cairn-policy/generated/cairn-policy.json` and documenting that required top-level policy object in `cairn/specs/cairn-policy-schema-compat/spec.md`.
Follow-Up: None for the former `policy missing field steel_orchestration_policy` blocker. Pinned and external Cairn validation both pass after the policy artifact refresh.

## Validation evidence

The following commands were rerun in dedicated visible tool calls after the spec alignment update:

```text
+ cargo fmt --check
```

Result: passed with exit code 0.

```text
+ git diff --check
```

Result: passed with exit code 0.

```text
+ cargo test -p clankers-runtime backend_capability_defaults_cover_native_pueue_and_systemd_contracts --lib
running 1 test
test process_jobs::tests::backend_capability_defaults_cover_native_pueue_and_systemd_contracts ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 145 filtered out; finished in 0.00s
```

```text
+ cargo test -p clankers tools::process::tests::native_process_job_service --lib
running 5 tests
test tools::process::tests::native_process_job_service_gc_requires_db_and_rejects_foreign_backend_filter ... ok
test tools::process::tests::native_process_job_service_garbage_collects_completed_native_records ... ok
test tools::process::tests::native_process_job_service_preserves_default_start_list_wait_flow ... ok
test tools::process::tests::native_process_job_service_redacts_receipts_and_persisted_metadata ... ok
test tools::process::tests::native_process_job_service_restarts_running_entry ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 981 filtered out; finished in 0.11s
```

```text
+ cargo test -p clankers tools::process::tests::process_gc --lib
running 4 tests
test tools::process::tests::process_gc_only_deletes_native_logs_under_configured_temp_dir ... ok
test tools::process::tests::process_gc_removes_expired_completed_records_logs_and_skips_active_jobs ... ok
test tools::process::tests::process_gc_active_native_jobs_survive_age_count_and_log_pressure ... ok
test tools::process::tests::process_gc_backend_filter_preserves_unselected_backend_records ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 982 filtered out; finished in 0.01s
```

## Cairn validation blocker resolution

Historical failed command before policy refresh:

```text
+ nix run .#cairn -- validate --root .
error: failed to parse policy cairn-policy/generated/cairn-policy.json: policy missing field steel_orchestration_policy
```

The policy artifact now includes `steel_orchestration_policy`, and both pinned and external Cairn validate successfully:

```text
nix run .#cairn -- validate --root .
{
  "change_issues": [],
  "changes": 0,
  "issues": [],
  "layout": "cairn",
  "policy": "cairn-default",
  "spec_issues": [],
  "specs_validated": 105,
  "valid": true
}
```

```text
nix run path:/home/brittonr/git/cairn#cairn -- validate --root .
{
  "change_issues": [],
  "changes": 0,
  "issues": [],
  "layout": "cairn",
  "policy": "cairn-default",
  "spec_issues": [],
  "specs_validated": 105,
  "valid": true
}
```
