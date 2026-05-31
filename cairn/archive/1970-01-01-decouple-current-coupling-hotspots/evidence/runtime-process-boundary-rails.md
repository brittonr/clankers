# Runtime process-job boundary rail evidence

Evidence-ID: runtime-process-boundary-rails
Artifact-Type: command-output-summary
Task-ID: V8
Covers: coupling-hotspot-remediation.runtime-process-boundary
Date: 2026-05-31
Status: PASS

## Commands

```text
./scripts/check-process-job-boundary.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-runtime --lib process_job_tool_request_maps_to_operation_vocabulary
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-runtime --lib process_job_tool_receipt_serialization_golden_fixtures
CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers --lib process_parser_produces_backend_neutral_request_dtos_for_all_actions
CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers --lib native_admission_limit_rejects_at_capacity_with_typed_receipt
```

## Relevant output

```text
ok: process-job boundary rail passed

running 1 test
test process_jobs::tests::process_job_tool_request_maps_to_operation_vocabulary ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 164 filtered out

running 1 test
test process_jobs::tests::process_job_tool_receipt_serialization_golden_fixtures ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 164 filtered out

running 1 test
test tools::process::tests::process_parser_produces_backend_neutral_request_dtos_for_all_actions ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1029 filtered out

running 1 test
test tools::process::tests::native_admission_limit_rejects_at_capacity_with_typed_receipt ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1029 filtered out
```

## Coverage notes

The static rail requires `clankers-runtime::process_jobs` to own backend-neutral request, receipt, operation, status, backend, and `ProcessJobService` contracts without importing DB or native process command APIs. It requires `src/tools/process/adapter.rs` to parse agent JSON into typed `ProcessJobToolRequest` values only, and requires `src/tools/process.rs` to keep native/pueue/systemd backend services, storage conversions, retention/GC adapters, and agent-visible receipt projection outside the runtime contract crate.

The runtime tests cover typed request vocabulary and receipt serialization golden fixtures. The root process tool tests cover JSON-to-typed request parsing through the adapter and fail-closed typed receipt projection for backend admission denial.
