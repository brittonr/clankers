Artifact-Type: implementation-seam-inventory
Task-ID: I3
Covers: openspec-review-gates.metrics-derived-omission-prevention.task-fixtures
Captured-At: 2026-05-20T14:28:40Z
Reviewer: Hermes Agent

## Inventory

Current in-repo OpenSpec validation has two layers:

1. Vendored OpenSpec core verification in `vendor/openspec/src/core/verify.rs`.
   - `verify_from_content(tasks_content, has_specs_dir)` only checks task completion counts and whether a delta specs directory exists.
   - `verify_basic(change_dir)` only reads `tasks.md`, checks for `specs/`, and calls `verify_from_content`.
   - This is intentionally generic/vendor-shaped and does not read proposal/design/spec content.

2. Project-local drift rails in `scripts/check-*.rs`.
   - These are Rust cargo-script checks with deterministic string/fixture assertions.
   - Recent examples include `scripts/check-process-job-profile-kit.rs`, which checks runtime tests, docs, canonical specs, and drift-rail references together.
   - They are narrow, repo-owned, and easy to wire into docs/tasks without changing generic vendored OpenSpec behavior.

## Chosen seam

Add a project-local Rust script seam:

`./scripts/check-openspec-review-gates.rs`

The checker should stay outside `vendor/openspec` for the first implementation. It should consume small sanitized fixture directories under an OpenSpec-owned path and validate:

- vague deterministic-check tasks fail with a diagnostic naming the missing contract class;
- concrete fixture/command-backed tasks satisfy the same contract;
- repeated human/oracle findings without an `H#` task and `Artifact-Type: oracle-checkpoint` evidence fail;
- valid `H#` + checked-in oracle checkpoint evidence passes.

## Why this seam

- Lowest blast radius: no vendored OpenSpec API or CLI changes required.
- Matches existing Clankers pattern for release/readiness rails (`scripts/check-*-kit.rs`).
- Allows fixture-backed diagnostics before deciding whether to upstream or integrate into `vendor/openspec` later.
- Keeps repeated review-metrics policy repo-specific, where `openspec/AGENTS.md` and Clankers task conventions already live.

## Rejected seams

- Modify `vendor/openspec/src/core/verify.rs` now: rejected because the generic verifier currently has no proposal/design/spec inputs and changing its API would broaden scope.
- Add only prose guidance: rejected because the OpenSpec explicitly requires deterministic prevention fixtures.
- Depend on live `review_metrics_promotions` during every gate: rejected because gate fixtures should be deterministic and safe offline.

## Follow-up implementation shape

1. Add fixture directories under `openspec/changes/roi-01-harden-openspec-gate-omission-prevention/fixtures/openspec-review-gates/`.
2. Add `scripts/check-openspec-review-gates.rs` with pure parsing/classification helpers and table-driven fixture assertions.
3. Update authoring guidance to name the checker and contract classes.
4. Verify with `./scripts/check-openspec-review-gates.rs`, `openspec validate roi-01-harden-openspec-gate-omission-prevention --strict --json`, and `git diff --check`.
