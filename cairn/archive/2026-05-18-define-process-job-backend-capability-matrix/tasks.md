## Phase 1: Matrix contract

- [x] [serial] [covers=process-job-backend-capability-matrix.descriptor.common] Define `BackendCapabilities` DTO and operation/feature vocabulary. ✅ completed: 2026-05-18T04:20:11Z; evidence: `ProcessJobBackendCapabilities`, `ProcessJobSafeCapabilityHints`, `ProcessJobOperation`, focused runtime tests.
- [x] [parallel] [covers=process-job-backend-capability-matrix.backends.native] Specify native backend capability defaults. ✅ completed: 2026-05-18T04:20:11Z; evidence: `ProcessJobBackendCapabilities::native()` and adapter/runtime tests.
- [x] [parallel] [covers=process-job-backend-capability-matrix.backends.pueue] Specify pueue backend capability defaults and unavailable-service behavior. ✅ completed: 2026-05-18T04:20:11Z; evidence: `ProcessJobBackendCapabilities::pueue()`, `ProcessJobBackendCapabilities::unavailable(ProcessJobBackendKind::Pueue, ...)`, unavailable receipt tests.
- [x] [parallel] [covers=process-job-backend-capability-matrix.backends.systemd] Specify systemd backend capability defaults and non-systemd host behavior. ✅ completed: 2026-05-18T04:20:11Z; evidence: `ProcessJobBackendCapabilities::systemd()`, `ProcessJobBackendCapabilities::unavailable(ProcessJobBackendKind::Systemd, ...)`, unavailable receipt tests.

## Phase 2: Enforcement

- [x] [serial] [depends:phase-1] Add service-layer validation against backend capabilities before backend mutation. ✅ completed: 2026-05-18T04:20:11Z; evidence: `unsupported_backend_receipt()`/`unsupported_detail()` and `service_validation_can_fail_closed_before_backend_mutation`.
- [x] [parallel] [covers=process-job-backend-capability-matrix.errors.unsupported-action] Add typed `unsupported_action_for_backend` receipts with backend/action/capability details. ✅ completed: 2026-05-18T04:20:11Z; evidence: `ProcessJobError.capability_detail` and unsupported/unavailable receipt tests.
- [x] [parallel] [covers=process-job-backend-capability-matrix.projection.safe] Project safe backend capabilities into list/status/TUI DTOs. ✅ completed: 2026-05-18T04:20:11Z; evidence: `ProcessJobProjectionItem.capability_hints` and `list_projection_includes_safe_capability_hints_only`.

## Phase 3: Verification

- [x] [serial] [depends:phase-2] Add fake backend contract tests for every matrix field and unsupported action. ✅ completed: 2026-05-18T04:20:11Z; evidence: `fake_backend_capability_matrix_and_unavailable_receipts_are_explicit`.
- [x] [serial] [depends:phase-2] Add native/pueue/systemd adapter capability tests where practical. ✅ completed: 2026-05-18T04:20:11Z; evidence: `backend_capability_defaults_cover_native_pueue_and_systemd_contracts`, `cargo test -p clankers --lib process:: -- --nocapture --test-threads=1`.
- [x] [serial] [depends:phase-2] Run focused backend capability tests, `openspec validate define-process-job-backend-capability-matrix --strict --json`, and `git diff --check`. ✅ completed: 2026-05-18T04:20:11Z; evidence: `cargo fmt --check`; `cargo test -p clankers-runtime process_jobs -- --nocapture`; `CARGO_TARGET_DIR=target/process-job-capability-check cargo check -p clankers --tests`; `openspec validate define-process-job-backend-capability-matrix --strict --json`; `git diff --check`; `cargo test -p clankers --lib process:: -- --nocapture --test-threads=1`.
