## Phase 1: Retention model

- [x] [serial] [covers=process-job-retention-gc.policy.classes] Define retention classes, metadata/log/event/tombstone lifetimes, and eligibility rules. ✅ 5m (started: 2026-05-18T06:34:39Z → completed: 2026-05-18T06:40:03Z; evidence: `retention_policy_classifies_metadata_lifetimes_and_active_protection`, `cargo check -p clankers-runtime --tests`)
- [x] [parallel] [covers=process-job-retention-gc.policy.active-protection] Define active/running/unreconciled job protection rules. ✅ 5m (started: 2026-05-18T06:34:39Z → completed: 2026-05-18T06:40:03Z; evidence: `ProcessJobRetentionClass::protects_active_state`, `retention_policy_classifies_metadata_lifetimes_and_active_protection`, `cargo check -p clankers-runtime --tests`)
- [x] [parallel] [covers=process-job-retention-gc.logs.overflow] Define max bytes/line/chunk, truncation counters, disk-full behavior, and degraded-log states. ✅ 5m (started: 2026-05-18T06:34:39Z → completed: 2026-05-18T06:40:03Z; evidence: `ProcessJobLogOverflowPolicy::classify_write`, `retention_policy_classifies_metadata_lifetimes_and_active_protection`, `cargo check -p clankers-runtime --tests`)

## Phase 2: GC service and integration

- [x] [serial] [depends:phase-1] Implement retention eligibility computation as pure/testable logic. ✅ 2m (started: 2026-05-18T06:40:03Z → completed: 2026-05-18T06:42:13Z; evidence: `ProcessJobRetentionPolicy::eligibility_for_summary`, `retention_policy_classifies_metadata_lifetimes_and_active_protection`, `cargo check -p clankers-runtime --tests`)
- [x] [parallel] [covers=process-job-retention-gc.receipts.typed] Add typed GC receipts and process/job API action shape. ✅ 84m (started: 2026-05-18T06:42:13Z → completed: 2026-05-18T08:06:42Z; evidence: `process_job_tool_receipt_serialization_golden_fixtures`, `process_gc_removes_expired_completed_records_logs_and_skips_active_jobs`, `cargo check -p clankers-runtime --tests`, `cargo check --tests`)
- [ ] [parallel] [covers=process-job-retention-gc.logs.missing] Add graceful missing-log and backend-log-reference degradation behavior.
- [ ] [parallel] [covers=process-job-retention-gc.nixos.integration] Add NixOS module options/tests for log/state directories and retention defaults.

## Phase 3: Verification

- [ ] [serial] [depends:phase-2] Add temp-dir native log GC tests and active-job skip tests.
- [ ] [serial] [depends:phase-2] Add disk-full/output-overflow/truncation fixture tests where practical.
- [ ] [serial] [depends:phase-2] Run focused retention/GC tests, Nix module eval tests, `openspec validate define-process-job-retention-gc --strict --json`, and `git diff --check`.
