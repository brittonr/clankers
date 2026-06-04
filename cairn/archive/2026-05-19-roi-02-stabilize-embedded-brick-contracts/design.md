# Design: Stabilize embedded brick contracts

## Context

The canonical `embedded-composition-kits` spec already defines adapter bricks, product kits, tool catalogs, capability packs, executable recipes, release receipts, and product-workbench dogfood. This change adds one narrower next-step contract from the lego-readiness backlog.

## Decision: Nickel at authoring and policy boundaries

Nickel SHOULD be used where Clankers needs typed, mergeable, human-authored contracts: crate boundary policy, capability-pack composition, catalog manifests, launch policies, product dogfood manifests, and optional product-owned schema examples. Rust SDK crates consume exported typed data, generated fixtures, or native Rust DTOs.

## Decision: BLAKE3 for evidence, receipts, and drift detection

BLAKE3 SHOULD hash evidence artifacts that need deterministic drift detection: generated API inventories, exported Nickel policies, normalized manifests, fixture matrices, dogfood receipts, sanitized transcripts, and release-readiness reports. Hashes are evidence pointers, not authorization by themselves.

## Decision: Keep FCIS boundaries intact

Reusable bricks stay in shell-free crates and examples. App-edge concerns remain product-owned until multiple integrations justify promotion. Any public API stabilization must be backed by checked examples or receipts.

## Risks

- **Premature API promotion:** mitigated by dogfood-first tasks and explicit non-goals.
- **Policy/runtime coupling:** mitigated by keeping Nickel out of runtime SDK dependencies.
- **Hashing sensitive evidence:** mitigated by requiring sanitized/redacted committed fixtures before BLAKE3 receipt inclusion.

## Verification plan

Validate this OpenSpec strictly, then implement with the smallest focused Rust/doc/example checks plus `scripts/check-embedded-agent-sdk.sh` when SDK boundaries change.
