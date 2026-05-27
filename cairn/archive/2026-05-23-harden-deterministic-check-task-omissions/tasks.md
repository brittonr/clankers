## Phase 1: Scope and evidence

- [x] [serial] S1: Record sanitized review metrics evidence selecting `omission|tasks|deterministic-check` as the next review-gate hardening category [r[cairn-review-gates.deterministic-verification-tasks]] [evidence=evidence/metrics-snapshot-2026-05-23.md]

## Phase 2: Implementation

- [x] [serial] I1: Add `missing-deterministic-check-artifact-task` to the focused review-gate checker [r[cairn-review-gates.deterministic-verification-tasks]]
- [x] [serial] I2: Add positive and negative deterministic-check fixture coverage that proves vague tasks fail and concrete fixture/helper/command tasks pass [r[cairn-review-gates.deterministic-verification-tasks]]
- [x] [serial] I3: Update operator guidance and accepted spec text for the required deterministic-check task shape [r[cairn-review-gates.deterministic-verification-tasks]]

## Phase 3: Verification

- [x] [serial] V1: Run `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-cairn-review-gates.rs` [r[cairn-review-gates.deterministic-verification-tasks]]
- [x] [serial] V2: Run `mdbook build docs` [r[cairn-review-gates.deterministic-verification-tasks]]
- [x] [serial] V3: Run Cairn proposal, design, tasks gates plus `nix run .#cairn -- validate --root .` [r[cairn-review-gates.deterministic-verification-tasks]]
- [x] [serial] V4: Run `git diff --check` and staged diff checks before commit [r[cairn-review-gates.deterministic-verification-tasks]]
