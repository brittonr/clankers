## Phase 1: State machine

- [x] [serial] [covers=process-job-reconciliation-state-machine.states.vocabulary] Define reconciliation states, log-degradation states, and transition rules. ✅ 24m (started: 2026-05-18T05:48:00Z → completed: 2026-05-18T06:12:02Z; evidence: `cargo test -p clankers-runtime reconciliation --lib`, `cargo check -p clankers-runtime --tests`)
- [ ] [parallel] [covers=process-job-reconciliation-state-machine.identity.pid-reuse] Define native PID/process-group identity checks and fail-closed behavior.
- [ ] [parallel] [covers=process-job-reconciliation-state-machine.backends.external] Define pueue/systemd backend ref reconciliation contracts.

## Phase 2: Service integration

- [ ] [serial] [depends:phase-1] Add backend reconciliation interface and service orchestration on daemon startup.
- [ ] [parallel] [covers=process-job-reconciliation-state-machine.receipts.degraded] Project degraded reconciliation/log states into list/poll/log/kill receipts.
- [ ] [parallel] [covers=process-job-reconciliation-state-machine.storage.stable-id] Preserve stable Clankers IDs while updating backend refs/status/log degradation fields.

## Phase 3: Verification

- [ ] [serial] [depends:phase-2] Add fake backend tests for every reconciliation outcome.
- [ ] [serial] [depends:phase-2] Add native restart/crash-style tests with temp db/log dirs and long-lived child processes where practical.
- [ ] [serial] [depends:phase-2] Run focused reconciliation tests, `openspec validate define-process-job-reconciliation-state-machine --strict --json`, and `git diff --check`.
