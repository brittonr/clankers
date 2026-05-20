## Change Status

Planning scaffold is complete. Implementation tasks remain open; do not archive until the gate/template changes are implemented and verified.

## Phase 1: Spec and design scaffold

- [x] [serial] I1: Create ROI-ranked OpenSpec scaffold for metrics-derived gate omission prevention [covers=openspec-review-gates.metrics-derived-omission-prevention.safe-snapshot] ✅ 0h 20m (started: 2026-05-20T14:19:57Z → completed: 2026-05-20T14:39:57Z)
- [x] [serial] I2: Validate scaffold with strict OpenSpec validation and commit/push the active change [covers=openspec-review-gates.*] ✅ 0h 05m (started: 2026-05-20T14:39:57Z → completed: 2026-05-20T14:44:57Z)
  - Evidence: `openspec validate roi-01-harden-openspec-gate-omission-prevention --strict --json` passed; `git diff --check` passed. `openspec validate --all --strict --json` still has pre-existing legacy spec failures unrelated to this active change, while the new change itself passed.

## Phase 2: Gate fixtures and diagnostics

- [ ] [serial] I3: Inventory the current OpenSpec gate/check implementation and choose the narrow Rust/script seam for fixture-backed omission diagnostics [covers=openspec-review-gates.metrics-derived-omission-prevention.task-fixtures]
- [ ] [parallel] I4: Add negative fixtures for vague task coverage of deterministic request/stream/retry contracts [covers=openspec-review-gates.deterministic-verification-tasks.vague-task]
- [ ] [parallel] I5: Add positive fixtures proving concrete fixture/command-backed tasks satisfy deterministic verification obligations [covers=openspec-review-gates.deterministic-verification-tasks.fixture-task]
- [ ] [parallel] I6: Add negative and positive fixtures for `H#` oracle checkpoint evidence, including missing/prose-only rejection [covers=openspec-review-gates.oracle-checkpoints.repeated-human-omission,openspec-review-gates.oracle-checkpoints.prose-only-rejected]

## Phase 3: Guidance and drift rails

- [ ] [depends:I3] I7: Update OpenSpec authoring guidance/templates to require contract-specific deterministic verification tasks for request shape, stream boundaries, retry policy, security/redaction policy, receipt fields, and discovery visibility [covers=openspec-review-gates.deterministic-verification-tasks]
- [ ] [depends:I6] I8: Update oracle-checkpoint guidance so repeated human-routed findings require an `H#` task and checked-in `Artifact-Type: oracle-checkpoint` evidence [covers=openspec-review-gates.oracle-checkpoints]
- [ ] [depends:I7] I9: Add or update a drift check that keeps metrics-derived fixtures, diagnostics, and guidance examples aligned [covers=openspec-review-gates.metrics-derived-omission-prevention.task-fixtures]

## Phase 4: Verification and closeout

- [ ] [depends:I4] V1: Run the focused gate/check fixture tests and record the command plus diagnostics in closeout [covers=openspec-review-gates.metrics-derived-omission-prevention.task-fixtures,openspec-review-gates.deterministic-verification-tasks]
- [ ] [depends:I9] V2: Run the guidance/drift check proving templates and diagnostics stay aligned [covers=openspec-review-gates.metrics-derived-omission-prevention.task-fixtures]
- [ ] [serial] V3: Run `openspec validate roi-01-harden-openspec-gate-omission-prevention --strict --json` [covers=openspec-review-gates.*]
- [ ] [serial] V4: Run `cargo fmt --check` if Rust changes are made, plus `git diff --check` [covers=openspec-review-gates.*]
- [ ] [serial] H1: Review metrics-derived scope against `openspec/AGENTS.md` and confirm no human/oracle checkpoint claim is closed by prose alone [covers=openspec-review-gates.oracle-checkpoints] [evidence=openspec/changes/roi-01-harden-openspec-gate-omission-prevention/evidence/review-metrics-snapshot.md]

## Verification Coverage

- `r[openspec-review-gates.metrics-derived-omission-prevention]` → I1, I3, I4, I9, V1, V2
- `r[openspec-review-gates.metrics-derived-omission-prevention.task-fixtures]` → I3, I4, I9, V1, V2
- `r[openspec-review-gates.metrics-derived-omission-prevention.safe-snapshot]` → I1, H1
- `r[openspec-review-gates.deterministic-verification-tasks]` → I4, I5, I7, V1
- `r[openspec-review-gates.deterministic-verification-tasks.vague-task]` → I4, V1
- `r[openspec-review-gates.deterministic-verification-tasks.fixture-task]` → I5, V1
- `r[openspec-review-gates.oracle-checkpoints]` → I6, I8, H1
- `r[openspec-review-gates.oracle-checkpoints.repeated-human-omission]` → I6, I8, H1
- `r[openspec-review-gates.oracle-checkpoints.prose-only-rejected]` → I6, V1, H1
