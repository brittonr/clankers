# Proposal: harden design omission gate

## Why

Review metrics show repeated design-stage omission findings after the task-stage omission gate was hardened. The highest-volume remaining design failures are not implementation bugs; they are underspecified `design.md` artifacts that omit concrete reasoning-signature retention, retry-policy bounds, or a scenario-complete verification plan.

## What Changes

- Extend `scripts/check-cairn-review-gates.rs` with deterministic design-stage completeness categories.
- Add paired sanitized fixtures that reject vague design prose and accept concrete design coverage.
- Document the new diagnostics in `docs/src/reference/cairn-review-gates.md`.
- Add Cairn requirements for design-stage omission prevention.

## Non-Goals

- No live provider smoke, credential use, or qwen/aspen2 readiness run is required for this pure deterministic gate.
- No broad rewrite of legacy Cairn history.

## Verification

Run the focused review-gate fixture script, formatting, docs build, Cairn gates/validate, and diff checks before sync/archive and before commit.
