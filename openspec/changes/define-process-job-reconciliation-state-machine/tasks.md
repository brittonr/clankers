## Phase 1: State machine

- [x] [serial] [covers=process-job-reconciliation-state-machine.states.vocabulary] Define reconciliation states, log-degradation states, and transition rules. ✅ 24m (started: 2026-05-18T05:48:00Z → completed: 2026-05-18T06:12:02Z; evidence: `cargo test -p clankers-runtime reconciliation --lib`, `cargo check -p clankers-runtime --tests`)
- [x] [parallel] [covers=process-job-reconciliation-state-machine.identity.pid-reuse] Define native PID/process-group identity checks and fail-closed behavior. ✅ 25m (started: 2026-05-18T05:48:00Z → completed: 2026-05-18T06:13:00Z; evidence: `native_identity_reconciliation_fails_closed_on_pid_reuse_or_ambiguous_identity` in `cargo test -p clankers-runtime reconciliation --lib`)
- [x] [parallel] [covers=process-job-reconciliation-state-machine.backends.external] Define pueue/systemd backend ref reconciliation contracts. ✅ 2m (started: 2026-05-18T06:13:00Z → completed: 2026-05-18T06:15:21Z; evidence: `external_backend_reconciliation_*` in `cargo test -p clankers-runtime reconciliation --lib`, `cargo check -p clankers-runtime --tests`)

## Phase 2: Service integration

- [x] [serial] [depends:phase-1] Add backend reconciliation interface and service orchestration on daemon startup. ✅ 7m (started: 2026-05-18T06:15:21Z → completed: 2026-05-18T06:22:21Z; evidence: `startup_reconciliation_updates_nonterminal_jobs_and_skips_terminal_records` in `cargo test -p clankers-runtime reconciliation --lib`, existing daemon startup call in `src/modes/agent_setup.rs`, `cargo check --tests`)
- [x] [parallel] [covers=process-job-reconciliation-state-machine.receipts.degraded] Project degraded reconciliation/log states into list/poll/log/kill receipts. ✅ 7m (started: 2026-05-18T06:22:21Z → completed: 2026-05-18T06:29:15Z; evidence: `durable_degraded_records_project_into_poll_log_and_kill_results` in `cargo test -p clankers durable_ -- --nocapture`, `cargo check --tests`)
- [x] [parallel] [covers=process-job-reconciliation-state-machine.storage.stable-id] Preserve stable Clankers IDs while updating backend refs/status/log degradation fields. ✅ 1m (started: 2026-05-18T06:29:15Z → completed: 2026-05-18T06:30:18Z; evidence: `process_restart_reconciliation_preserves_stable_id_log_ref_and_reports_lost_status` in `cargo test -p clankers process_restart_reconciliation_preserves_stable_id_log_ref_and_reports_lost_status`)

## Phase 3: Verification

- [ ] [serial] [depends:phase-2] Add fake backend tests for every reconciliation outcome.
- [ ] [serial] [depends:phase-2] Add native restart/crash-style tests with temp db/log dirs and long-lived child processes where practical.
- [ ] [serial] [depends:phase-2] Run focused reconciliation tests, `openspec validate define-process-job-reconciliation-state-machine --strict --json`, and `git diff --check`.
