# Process job contract extraction evidence

Evidence-ID: extract-process-job-contracts.process-job-contracts
Artifact-Type: command-output-summary
Task-ID: I1,I2,I3,I4,I5,I6,I7,I8,I9,I10
Covers: remaining-coupling-drain.process-job-policy.neutral-contract-owner
Date: 2026-06-05
Status: PARTIAL-PASS

## Implementation summary

- Chose `clankers-tool-host::process_jobs` as the first green neutral owner for process-job tool contracts, avoiding a new workspace crate while keeping the contract below `clankers-runtime` and root shell modules.
- Moved the native-process admission DTOs and pure admission decision function out of `clankers-runtime::process_jobs` into `clankers-tool-host::process_jobs`.
- Moved safe profile receipt metadata constants/DTOs out of `clankers-runtime::process_jobs` into `clankers-tool-host::process_jobs`.
- Moved backend-neutral `ProcessJobResourcePolicy` out of `clankers-runtime::process_jobs` into `clankers-tool-host::process_jobs`.
- Moved backend references, notification event ids, backend kind labels, and operation vocabulary out of `clankers-runtime::process_jobs` into `clankers-tool-host::process_jobs`.
- Moved backend-neutral process status vocabulary and labels out of `clankers-runtime::process_jobs` into `clankers-tool-host::process_jobs`.
- Moved caller/cwd authorization DTOs and backend capability/hint descriptors out of `clankers-runtime::process_jobs` into `clankers-tool-host::process_jobs`.
- Moved backend-neutral log stream/reference/cursor/range, log overflow disposition, retention class, notification policy/kind, and notification decision/observation DTOs out of `clankers-runtime::process_jobs` into `clankers-tool-host::process_jobs`.
- Moved redaction constants and `ProcessJobRedactionPolicy` out of `clankers-runtime::process_jobs` into `clankers-tool-host::process_jobs`.
- Kept compatibility reexports from `clankers-runtime::process_jobs` so root/backend code can continue importing the old path while later slices migrate callers.
- Kept runtime receipt projection as a compatibility extension over the moved backend capability DTOs because `ProcessJobReceipt` remains runtime-owned in this partial slice.
- Preserved runtime notification-event redaction with a `ProcessJobNotificationRedactionTarget` compatibility trait while `ProcessJobNotificationEvent` remains runtime-owned.
- Refreshed generated runtime facade and embedded SDK inventories; migrated admission/profile/resource/id-operation/status/capability/log/notification/redaction contracts now appear as supported `clankers-tool-host` API instead of yellow runtime-owned structs.

## Relevant output

```text
cargo test -p clankers-tool-host --lib process_jobs
running 15 tests
process_jobs::tests::backend_capabilities_advertise_supported_operations ... ok
process_jobs::tests::backend_kind_and_operation_labels_are_stable ... ok
process_jobs::tests::caller_scope_and_capabilities_authorize_by_owner_and_operation ... ok
process_jobs::tests::cwd_policy_is_plain_backend_neutral_data ... ok
process_jobs::tests::log_overflow_policy_classifies_truncation_and_disk_pressure ... ok
process_jobs::tests::log_reference_cursor_and_range_are_plain_backend_neutral_data ... ok
process_jobs::tests::native_admission_accepts_below_limit_and_denies_at_limit ... ok
process_jobs::tests::notification_decision_and_observation_are_backend_neutral_data ... ok
process_jobs::tests::notification_policy_bounds_watch_patterns_without_dispatch ... ok
process_jobs::tests::process_job_status_terminal_and_labels_are_stable ... ok
process_jobs::tests::profile_receipt_metadata_projects_from_safe_metadata ... ok
process_jobs::tests::redaction_policy_bounds_and_redacts_sensitive_contract_fields ... ok
process_jobs::tests::resource_policy_is_plain_backend_neutral_data ... ok
process_jobs::tests::retention_class_identifies_active_state ... ok
process_jobs::tests::safe_capability_hints_project_non_sensitive_booleans ... ok
exit=0

cargo test -p clankers-runtime --lib native_admission_decision
running 1 test
process_jobs::tests::native_admission_decision_is_owned_by_process_job_contracts ... ok
exit=0

cargo test -p clankers-tool-host --lib
running 29 tests
29 passed; 0 failed
exit=0

cargo test -p clankers-runtime --lib log_overflow_policy
running 2 tests
process_jobs::tests::log_overflow_policy_fixture_serialization_is_stable ... ok
process_jobs::tests::log_overflow_policy_fixtures_cover_truncation_and_disk_pressure ... ok
exit=0

cargo test -p clankers-runtime --lib redaction_policy_bounds_previews_and_redacts_sensitive_metadata
running 1 test
process_jobs::tests::redaction_policy_bounds_previews_and_redacts_sensitive_metadata ... ok
exit=0

cargo test -p clankers-runtime --lib notification_decisions_and_persistence_redact_secret_excerpts
running 1 test
process_jobs::tests::notification_decisions_and_persistence_redact_secret_excerpts ... ok
exit=0

scripts/check-process-job-profile-kit.rs
process-job-profile-kit checker passed
exit=0

cargo test -p clankers-runtime --lib process_job_profile_kit_validates_manifest_policy_identity_and_redaction
running 1 test
process_jobs::tests::process_job_profile_kit_validates_manifest_policy_identity_and_redaction ... ok
exit=0

cargo test -p clankers-runtime --lib profile_policy_rejects_paths_resources_and_unsupported_manifest_versions
running 1 test
process_jobs::tests::profile_policy_rejects_paths_resources_and_unsupported_manifest_versions ... ok
exit=0

cargo check -p clankers --tests
Finished `dev` profile [optimized + debuginfo]
exit=0

scripts/check-embedded-sdk-api.rs
ok: embedded SDK API inventory covers 812 public items (817 rows)
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

This is still a partial extraction. Common id/identity contracts, receipt envelopes, full retention policy envelopes, and notification event contracts still need follow-on migration before the process-job contract drain can close.
