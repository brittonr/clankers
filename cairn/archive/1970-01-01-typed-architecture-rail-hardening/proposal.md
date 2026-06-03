# Change: Typed Architecture Rail Hardening

## Problem

`scripts/check-lego-architecture-boundaries.rs` still relies heavily on exact string-presence anchors. Those anchors catch ownership drift, but they can also fail harmless refactors and require manual archaeology when diagnostics do not point to typed owners.

## Goals

- Replace one brittle source-anchor cluster with AST, Cargo metadata, generated ownership manifests, or behavior fixtures.
- Improve diagnostics to name source, target owner, and replacement path.
- Document any exact-string fallback that remains necessary.

## Non-goals

- Do not weaken ownership guarantees to make refactors easier.
- Do not rustfmt the large cargo-script rail unless broad formatting churn is intended.
- Do not replace behavior coverage with source-only checks.

## Proposed scope

Pick a cluster of `require_contains` anchors, such as process ownership, agent ports, controller projection, slash parity, or provider/router request shape, and convert it to typed inventory or deterministic fixture checks.

## Verification

Focused validation should include the architecture rail itself, any moved fixture tests, Cairn gates, and `git diff --check`.
