## Phase 1: Scaffold and evidence

- [x] S1 [serial] r[openspec-review-gates.spec-stage-omission-prevention.strong-constraint-spec] Preserve a sanitized metrics snapshot for the selected repeated omission category. [evidence=evidence/metrics-snapshot-2026-05-27.md]
- [x] S2 [serial] r[openspec-review-gates.spec-stage-omission-prevention.strong-constraint-spec] Define the delta spec and design for strong proposal/design constraint preservation before checker implementation.

## Phase 2: Fixture-backed implementation

- [x] I1 [serial] r[openspec-review-gates.spec-stage-omission-prevention.strong-constraint-spec] Inspect `scripts/check-openspec-review-gates.rs`, current strong-text helpers, existing spec-stage fixtures, and `docs/src/reference/openspec-review-gates.md` before editing the checker.
- [x] I2 [serial] r[openspec-review-gates.spec-stage-omission-prevention.strong-constraint-spec] Add `missing-strong-constraint-spec` to the focused review-gate checker for sanitized strong lifecycle constraints that are omitted or weakened in delta specs; diagnostics name `source_artifact` and reject weak same-noun optional coverage.
- [x] I3 [parallel] r[openspec-review-gates.spec-stage-omission-prevention.strong-constraint-spec] Add negative sanitized fixtures where proposal/design require generated artifact hygiene, required local verification, source preservation, capability-boundary preservation, or a forbidden delivery path while the delta spec omits or weakens the constraint.
- [x] I4 [parallel] r[openspec-review-gates.spec-stage-omission-prevention.strong-constraint-spec-satisfied] Add a positive sanitized fixture where the delta spec preserves the same strong constraint with normative requirement or scenario text.
- [x] I5 [parallel] r[openspec-review-gates.spec-stage-omission-prevention.strong-constraint-spec] r[openspec-review-gates.spec-stage-omission-prevention.strong-constraint-spec-satisfied] Update operator guidance so authors know that strong proposal/design constraints need equivalent normative delta spec coverage and cannot be weakened into optional generic evidence.

## Phase 3: Verification

- [x] V1 [serial] r[openspec-review-gates.spec-stage-omission-prevention.strong-constraint-spec] r[openspec-review-gates.spec-stage-omission-prevention.strong-constraint-spec-satisfied] Run `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-openspec-review-gates.rs`. [evidence=evidence/review-gate-fixtures.md]
- [x] V2 [serial] r[openspec-review-gates.spec-stage-omission-prevention.strong-constraint-spec] r[openspec-review-gates.spec-stage-omission-prevention.strong-constraint-spec-satisfied] Run `mdbook build docs`, `nix run .#cairn -- gate proposal harden-review-evidence-gates --root .`, `nix run .#cairn -- gate design harden-review-evidence-gates --root .`, `nix run .#cairn -- gate tasks harden-review-evidence-gates --root .`, `nix run .#cairn -- validate --root .`, and `git diff --check`. [evidence=evidence/final-validation.md]

## Traceability

- `openspec-review-gates.review-metrics-regression-rail.snapshot-selects-category` -> S1
- `openspec-review-gates.review-metrics-regression-rail.guidance-and-wiring` -> I5, V2
- `openspec-review-gates.spec-stage-omission-prevention.strong-constraint-spec` -> S2, I1, I2, I3, V1
- `openspec-review-gates.spec-stage-omission-prevention.strong-constraint-spec-satisfied` -> I4, V1
