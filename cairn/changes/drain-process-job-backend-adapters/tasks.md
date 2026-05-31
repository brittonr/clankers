## Phase 1: Backend adapter extraction

- [ ] [serial] I1: Define the process-job backend adapter ownership map for native, pueue, systemd, durable storage, retention/GC, and notification delivery, and add source-rail expectations for each owner. r[process-job-backend-adapters.root-projection.thin-file] [covers=process-job-backend-adapters.root-projection.thin-file]
- [ ] [serial] I2: Extract native process registry/admission/termination/restart policy from `src/tools/process.rs` into a named backend adapter using `clankers-runtime::process_jobs` contracts. r[process-job-backend-adapters.backend-adapters.native] [covers=process-job-backend-adapters.backend-adapters.native]
- [ ] [serial] I3: Extract pueue command/status/log projection behind a fakeable runner trait and typed process-job backend adapter. r[process-job-backend-adapters.backend-adapters.pueue] [covers=process-job-backend-adapters.backend-adapters.pueue]
- [ ] [serial] I4: Extract systemd unit/show/list/log projection behind a fakeable runner trait and typed process-job backend adapter. r[process-job-backend-adapters.backend-adapters.systemd] [covers=process-job-backend-adapters.backend-adapters.systemd]
- [ ] [serial] I5: Move durable reconciliation, retention/GC, log-degradation, and notification policy out of the root tool path into runtime-owned helpers or focused service adapters. r[process-job-backend-adapters.durable-policy] [covers=process-job-backend-adapters.durable-policy]

## Phase 2: Verification

- [ ] [serial] V1: Add focused fake-runner/backend fixtures for native, pueue, and systemd start/list/poll/log/error paths without requiring live host services. r[process-job-backend-adapters.verification.backend-fixtures] [covers=process-job-backend-adapters.verification.backend-fixtures]
- [ ] [serial] V2: Add retention/reconciliation/notification fixtures proving policy lives outside `src/tools/process.rs` and returns typed redacted receipts. r[process-job-backend-adapters.durable-policy.no-root-tool] [covers=process-job-backend-adapters.durable-policy.no-root-tool]
- [ ] [serial] V3: Run process-job focused tests, `cargo check --tests` for touched crates, `./scripts/check-lego-architecture-boundaries.rs`, Cairn gates/validate for this change, and `git diff --check`. r[process-job-backend-adapters.verification.closeout] [covers=process-job-backend-adapters.verification.closeout]
