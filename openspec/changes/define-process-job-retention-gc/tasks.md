## Phase 1: Retention model

- [x] [serial] [covers=process-job-retention-gc.policy.classes] Define retention classes, metadata/log/event/tombstone lifetimes, and eligibility rules. ✅ 5m (started: 2026-05-18T06:34:39Z → completed: 2026-05-18T06:40:03Z; evidence: `retention_policy_classifies_metadata_lifetimes_and_active_protection`, `cargo check -p clankers-runtime --tests`)
- [x] [parallel] [covers=process-job-retention-gc.policy.active-protection] Define active/running/unreconciled job protection rules. ✅ 5m (started: 2026-05-18T06:34:39Z → completed: 2026-05-18T06:40:03Z; evidence: `ProcessJobRetentionClass::protects_active_state`, `retention_policy_classifies_metadata_lifetimes_and_active_protection`, `cargo check -p clankers-runtime --tests`)
- [x] [parallel] [covers=process-job-retention-gc.logs.overflow] Define max bytes/line/chunk, truncation counters, disk-full behavior, and degraded-log states. ✅ 5m (started: 2026-05-18T06:34:39Z → completed: 2026-05-18T06:40:03Z; evidence: `ProcessJobLogOverflowPolicy::classify_write`, `retention_policy_classifies_metadata_lifetimes_and_active_protection`, `cargo check -p clankers-runtime --tests`)

## Phase 2: GC service and integration

- [x] [serial] [depends:phase-1] Implement retention eligibility computation as pure/testable logic. ✅ 2m (started: 2026-05-18T06:40:03Z → completed: 2026-05-18T06:42:13Z; evidence: `ProcessJobRetentionPolicy::eligibility_for_summary`, `retention_policy_classifies_metadata_lifetimes_and_active_protection`, `cargo check -p clankers-runtime --tests`)
- [x] [parallel] [covers=process-job-retention-gc.receipts.typed] Add typed GC receipts and process/job API action shape. ✅ 84m (started: 2026-05-18T06:42:13Z → completed: 2026-05-18T08:06:42Z; evidence: `process_job_tool_receipt_serialization_golden_fixtures`, `process_gc_removes_expired_completed_records_logs_and_skips_active_jobs`, `cargo check -p clankers-runtime --tests`, `cargo check --tests`)
- [x] [parallel] [covers=process-job-retention-gc.logs.missing] Add graceful missing-log and backend-log-reference degradation behavior. ✅ 25m (started: 2026-05-18T08:06:42Z → completed: 2026-05-18T08:31:55Z; evidence: `missing_native_log_degrades_list_poll_and_log_without_hiding_metadata`, `backend_log_reference_degrades_to_durable_metadata_when_backend_read_fails`, `durable_degraded_records_project_into_poll_log_and_kill_results`)
- [x] [parallel] [covers=process-job-retention-gc.nixos.integration] Add NixOS module options/tests for log/state directories and retention defaults. ✅ 21m (started: 2026-05-18T13:26:43Z → completed: 2026-05-18T13:47:43Z; evidence: `nixos-module-process-persistence`, `nixos-module-process-pueue`, `nixos-module-process-systemd-limits`, `nix eval .#checks.x86_64-linux.vm-module-daemon.drvPath`, `cargo check --tests`)

## Phase 3: Verification

- [x] [serial] [depends:phase-2] Add temp-dir native log GC tests and active-job skip tests. ✅ 6m (started: 2026-05-18T13:48:00Z → completed: 2026-05-18T13:54:11Z; evidence: `process_gc_only_deletes_native_logs_under_configured_temp_dir`, `process_gc_active_native_jobs_survive_age_count_and_log_pressure`, `process_gc_removes_expired_completed_records_logs_and_skips_active_jobs`)
- [ ] [serial] [depends:phase-2] Add disk-full/output-overflow/truncation fixture tests where practical.
- [ ] [serial] [depends:phase-2] Run focused retention/GC tests, Nix module eval tests, `openspec validate define-process-job-retention-gc --strict --json`, and `git diff --check`.
