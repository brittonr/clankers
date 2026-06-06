# Process job contract extraction evidence

Evidence-ID: extract-process-job-contracts.process-job-contracts
Artifact-Type: command-output-summary
Task-ID: I1,I2,I3,I4,I5,I6,I7,I8,I9,I10,I11,I12,V1,V2
Covers: remaining-coupling-drain.process-job-policy.neutral-contract-owner
Date: 2026-06-05
Status: PASS

## Implementation summary

- Chose `clankers-tool-host::process_jobs` as the green neutral owner for process-job tool contracts, avoiding a new workspace crate while keeping the contract below `clankers-runtime` and root shell modules.
- Moved native-process admission DTOs and pure admission decision logic into `clankers-tool-host::process_jobs`.
- Moved safe profile receipt metadata constants/DTOs into `clankers-tool-host::process_jobs`.
- Moved backend-neutral `ProcessJobResourcePolicy`, backend refs, event ids, backend kind labels, operation vocabulary, process status vocabulary, caller/cwd authorization DTOs, backend capabilities/hints, log reference/range/overflow DTOs, retention class, notification policy/kind, notification decision/observation, and redaction policy into `clankers-tool-host::process_jobs`.
- Moved process id/identity, process-job tool request DTOs, summaries, receipt/error envelopes, tool-result envelopes, completed-job retention metadata/eligibility/GC receipt DTOs, log chunks, and notification event DTOs into `clankers-tool-host::process_jobs`.
- Replaced moved public chrono timestamps with neutral `ProcessJobTimestamp` unix-second DTOs so `clankers-tool-host` remains inside the green SDK/FCIS boundary.
- Kept compatibility reexports from `clankers-runtime::process_jobs` so existing root/backend imports continue compiling while later slices migrate callers.
- Kept runtime receipt projection as a compatibility extension over the moved backend capability DTOs; the extension now returns the moved `ProcessJobReceipt` type.
- Refreshed generated runtime facade and embedded SDK inventories; migrated process-job contracts now appear as supported `clankers-tool-host` API instead of yellow runtime-owned structs.

## Relevant output

```text
cargo test -p clankers-tool-host --lib process_jobs
running 19 tests
process_jobs::tests::process_job_identity_and_id_helpers_are_stable ... ok
process_jobs::tests::tool_request_maps_to_operation_vocabulary ... ok
process_jobs::tests::receipt_errors_and_tool_result_envelopes_are_backend_neutral ... ok
process_jobs::tests::retention_receipts_and_notification_events_are_backend_neutral ... ok
19 passed; 0 failed
exit=0

cargo test -p clankers-tool-host --lib
running 33 tests
33 passed; 0 failed
exit=0

cargo test -p clankers-runtime --lib process_job_tool_receipt_envelope_keeps_common_fields_and_payloads_separate
running 1 test
process_jobs::tests::process_job_tool_receipt_envelope_keeps_common_fields_and_payloads_separate ... ok
exit=0

cargo test -p clankers-runtime --lib retention_policy_classifies_metadata_lifetimes_and_active_protection
running 1 test
process_jobs::tests::retention_policy_classifies_metadata_lifetimes_and_active_protection ... ok
exit=0

cargo test -p clankers-runtime --lib notification_decisions_and_persistence_redact_secret_excerpts
running 1 test
process_jobs::tests::notification_decisions_and_persistence_redact_secret_excerpts ... ok
exit=0

cargo check -p clankers --tests
Finished `dev` profile [optimized + debuginfo]
exit=0

scripts/check-embedded-sdk-api.rs
ok: embedded SDK API inventory covers 995 public items (1000 rows)
exit=0

scripts/check-experimental-sdk-port-budget.rs
ok: experimental SDK port budget covers 0 experimental rows; 160 promoted rows
exit=0

scripts/check-runtime-facade-boundary.rs
ok: runtime facade boundary inventories clankers-runtime public API and dependency classifications
exit=0

scripts/check-message-contract-boundary.rs
ok: message contract boundary rail passed
exit=0

scripts/check-brick-inventory-stability.rs
brick-inventory-stability receipt written to target/embedded-sdk-release/brick-inventory-stability-receipt.json
exit=0

scripts/check-process-job-profile-kit.rs
process-job-profile-kit checker passed
exit=0

scripts/check-root-controller-runtime-adapters.rs
root-controller-runtime-adapters receipt written to target/embedded-sdk-release/root-controller-runtime-adapters-receipt.json
exit=0

nix run .#cairn -- validate --root .
valid: true
exit=0

nix run .#cairn -- gate tasks extract-process-job-contracts --root .
verdict: PASS
exit=0

git diff --check
exit=0
```

## Remaining work

Process-job DTO ownership for this Cairn package is complete. Runtime/root adapter call sites still import through compatibility reexports in many places; those can be migrated gradually after the DTO owner is stable.
