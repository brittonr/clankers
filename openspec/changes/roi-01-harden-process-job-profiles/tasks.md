## Change Status

Planning scaffold is complete. Implementation tasks remain open; do not archive until the runtime/docs/check changes are implemented and verified.

## Phase 1: Spec and design scaffold

- [x] [serial] I1: Create ROI-ranked OpenSpec scaffold for process/job profile hardening ✅ 0h 20m (started: 2026-05-20T02:44:50Z → completed: 2026-05-20T03:04:50Z)
- [x] [serial] I2: Validate scaffold with strict OpenSpec gate and commit/push the active change ✅ 0h 03m (started: 2026-05-20T03:04:50Z → completed: 2026-05-20T03:07:50Z)

## Phase 2: Functional core hardening

- [ ] [serial] I3: Implement or refine deterministic profile manifest discovery and precedence in the process/job profile core
- [ ] [parallel] I4: Add pure resolution tests proving no backend/store/credential access during profile validation
- [ ] [parallel] I5: Add negative policy tests for disallowed backend, malformed command/program shape, secret-like env keys, resource ceilings, cwd/writable-path policy, and ambiguous manifest sources
- [ ] [parallel] I6: Extend safe profile identity metadata and receipt projection for profile-started jobs while preserving direct-start compatibility

## Phase 3: Integration and docs

- [ ] [depends:I3] I7: Wire profile start requests through the existing backend-neutral process/job service path without backend fallback masking
- [ ] [depends:I6] I8: Update process-job docs and examples to document manifest version, discovery precedence, safe receipt fields, and fail-closed policy classes
- [ ] [depends:I5] I9: Extend `scripts/check-process-job-profile-kit.rs` and any fixtures/policy inventories so code/docs/spec drift is diagnosable

## Phase 4: Verification and closeout

- [ ] [depends:I3] V1: Run focused `cargo nextest run -p clankers-runtime` profile tests
- [ ] [depends:I7] V2: Run focused `cargo nextest run -p clankers` process-tool/profile receipt tests
- [ ] [depends:I9] V3: Run `scripts/check-process-job-profile-kit.rs`
- [ ] [serial] V4: Run `cargo fmt --check` and `git diff --check`
- [ ] [serial] V5: Validate `openspec validate roi-01-harden-process-job-profiles --strict --json`
- [ ] [serial] H1: Sync/archive the OpenSpec only after all implementation and verification tasks are complete

## Verification Coverage

- `r[durable-process-jobs.project-profiles.discovery]` → I3, I4, I8, V1
- `r[durable-process-jobs.project-profiles.side-effect-free]` → I4, I7, V1, V2
- `r[durable-process-jobs.project-profiles.invalid]` → I5, I9, V1, V3
- `r[durable-process-jobs.process-job-profile-kit.boundary]` → I8, I9, V3
- `r[durable-process-jobs.process-job-profile-kit.evidence]` → I4, I6, V1, V2
- `r[durable-process-jobs.process-job-profile-kit.fail-closed]` → I5, V1, V3
- `r[durable-process-jobs.process-job-profile-kit.receipts]` → I6, I7, V2
- `r[durable-process-jobs.process-job-profile-kit.drift]` → I9, V3
