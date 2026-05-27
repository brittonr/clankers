# Tasks

## Phase 1: Patch schema and authority boundary

- [ ] I1 [serial] r[steel-self-mutation-policy.host-functions.orchestration-patch-proposal] Define `clankers.steel.orchestration-patch.v1` with intent, target paths, expected before hashes, patch hash, gate list, activation policy, and rollback metadata.
- [ ] I2 [serial] r[steel-self-mutation-policy.host-functions.orchestration-patch-proposal] Add Rust parsing and validation for orchestration patch proposals before any write.
- [ ] I3 [serial] r[steel-self-mutation-policy.host-functions.authority-kernel-checkpoint] Detect and deny automatic authority-kernel changes such as new host calls, wider budgets, new UCAN abilities, broader path roots, provider/network/credential access, direct git commit or push, or required-gate removal.

## Phase 2: Isolated apply, activation, and rollback

- [ ] I4 [serial] r[steel-self-mutation-policy.receipts-and-preflight.isolated-apply] Apply candidate pack mutations only in an isolated worktree or staging area with expected before-hash checks.
- [ ] I5 [serial] r[steel-self-mutation-policy.receipts-and-preflight.orchestration-pack-receipt] Emit redacted receipts with old/new pack hashes, patch hashes, gate result hashes, activation decisions, and rollback references.
- [ ] I6 [serial] r[steel-self-mutation-policy.verification-and-rollback.next-turn-activation] Activate mutated packs only on explicit reload or a later turn after gates pass.
- [ ] I7 [serial] r[steel-self-mutation-policy.verification-and-rollback.orchestration-rollback] Add rollback that verifies current post-apply hash and backup hash before restoring pack files.

## Phase 3: Docs and verification

- [ ] I8 [parallel] r[steel-self-mutation-policy.verification-fixtures.orchestration-pack] Add operator docs for dry-run mutation, isolated apply, next-turn activation, denied authority changes, receipts, and rollback.
- [ ] V1 [serial] r[steel-self-mutation-policy.verification-fixtures.orchestration-pack] Run focused fixtures/checker for valid update, path escape, stale before hash, authority widening, required gate removal, failed validation, malformed schema, and stale rollback. [evidence=evidence/planned-verification.md]
- [ ] V2 [serial] r[steel-self-mutation-policy.host-functions.authority-kernel-checkpoint] Run docs build, Cairn gates, `cairn validate`, and diff checks. [evidence=evidence/planned-verification.md]
