# Process job contract extraction evidence

Evidence-ID: extract-process-job-contracts.process-job-contracts
Artifact-Type: command-output-summary
Task-ID: I1,I2,I4
Covers: remaining-coupling-drain.process-job-policy.neutral-contract-owner
Date: 2026-06-05
Status: PARTIAL-PASS

## Implementation summary

- Chose `clankers-tool-host::process_jobs` as the first green neutral owner for process-job tool contracts, avoiding a new workspace crate while keeping the contract below `clankers-runtime` and root shell modules.
- Moved the native-process admission DTOs and pure admission decision function out of `clankers-runtime::process_jobs` into `clankers-tool-host::process_jobs`.
- Kept compatibility reexports from `clankers-runtime::process_jobs` so root/backend code can continue importing the old path while later slices migrate callers.
- Refreshed generated runtime facade and embedded SDK inventories; the migrated admission contract now appears as supported `clankers-tool-host` API instead of yellow runtime-owned structs.

## Relevant output

```text
cargo test -p clankers-tool-host --lib process_jobs
running 1 test
process_jobs::tests::native_admission_accepts_below_limit_and_denies_at_limit ... ok
exit=0

cargo test -p clankers-runtime --lib native_admission_decision
running 1 test
process_jobs::tests::native_admission_decision_is_owned_by_process_job_contracts ... ok
exit=0

cargo test -p clankers-tool-host --lib
running 15 tests
15 passed; 0 failed
exit=0

scripts/check-process-job-profile-kit.rs
process-job-profile-kit checker passed
exit=0

scripts/check-embedded-sdk-api.rs
ok: embedded SDK API inventory covers 669 public items (674 rows)
exit=0

scripts/check-experimental-sdk-port-budget.rs
ok: experimental SDK port budget covers 0 experimental rows; 160 promoted rows
exit=0

scripts/check-brick-inventory-stability.rs
brick-inventory-stability receipt written to target/embedded-sdk-release/brick-inventory-stability-receipt.json
exit=0

scripts/check-runtime-facade-boundary.rs
ok: runtime facade boundary inventories clankers-runtime public API and dependency classifications
exit=0
```

## Remaining work

This is the first extraction step only. Profile, receipt, redaction, retention, notification, and backend capability contracts still need follow-on migration before the process-job contract drain can close.
