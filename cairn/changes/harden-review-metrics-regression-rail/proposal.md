# Proposal: Harden review metrics regression rail

## Why

Clankers already has a project-local OpenSpec review-gate checker, but review metrics still show high-count repeated omission classes from prior gates and done reviews. The dominant classes are task-stage omissions around deterministic fixtures, prompt/template traceability, and human/oracle checkpoints. Those findings should become a durable regression rail instead of another round of one-off artifact edits.

## What Changes

- Add a metrics-derived Cairn slice for extending `scripts/check-openspec-review-gates.rs` and its sanitized fixtures from repeated review metrics.
- Preserve a compact, secret-free evidence snapshot under this change so reviewers can see why the rail exists without reopening raw transcripts.
- Require the future implementation to keep fixture diagnostics, operator guidance, and flake/check wiring aligned.
- Keep this repo-local: do not patch generic Cairn/OpenSpec core until the local checker proves the rule shape.

## Capabilities

### Modified Capabilities
- `openspec-review-gates`: the project-local review-gate rail gains a metrics-regression loop that turns repeated omission findings into deterministic fixtures and diagnostics.

## Impact

- **Files likely affected**: `scripts/check-openspec-review-gates.rs`, `scripts/fixtures/openspec-review-gates/**`, `docs/src/reference/openspec-review-gates.md`, `flake.nix` if a new check target is needed.
- **Evidence**: sanitized metrics snapshot in `cairn/changes/harden-review-metrics-regression-rail/evidence/`.
- **Testing**: focused checker run, docs build, Cairn gates/validation, and `git diff --check`.
- **Non-goals**: no live provider probes, no raw hidden prompt/transcript material, no generic Cairn/OpenSpec-core change, no credentials or account identifiers.
