# Tasks

## Phase 1: Profile and discovery

- [x] I1 [serial] r[steel-repo-evolution-packs.discovery.default-deny] Add settings/path discovery that leaves repo-local Steel evolution inactive when `.clankers/steel/evolution-profile.ncl` is absent.
- [x] I2 [serial] r[steel-repo-evolution-packs.discovery.validate-before-activation] Define the Nickel pack contract and Rust exported-data structs for profile schema, script bindings, budgets, allowed host calls, receipt roots, and fallback policy.
- [x] I3 [serial] r[steel-repo-evolution-packs.discovery.hash-bound-reload] Add hash-bound activation/reload receipts for profile and script changes.

## Phase 2: Host ABI and planning

- [x] I4 [serial] r[steel-repo-evolution-packs.host-abi.typed-calls] Add the first Rust-owned host ABI surface for context reads, patch proposals, gate requests, receipt recording, and human checkpoints.
- [x] I5 [serial] r[steel-repo-evolution-packs.host-abi.unknown-denied] Reject unknown, widened, or ambient host authority requests before Steel execution or host effects.
- [x] I6 [serial] r[steel-repo-evolution-packs.typed-evolution-plan.accepted] Parse `clankers.steel.evolution-plan.v1` plans into Rust-owned action requests.
- [x] I7 [serial] r[steel-repo-evolution-packs.typed-evolution-plan.malformed] Add fail-closed malformed/unsupported/over-budget plan handling with stable issue codes.

## Phase 3: Receipts, docs, and verification

- [x] I8 [parallel] r[steel-repo-evolution-packs.receipts.activation] r[steel-repo-evolution-packs.receipts.plan] Emit deterministic redacted activation and plan receipts.
- [x] I9 [parallel] r[steel-repo-evolution-packs.verification.docs] Document pack layout, Nickel profile contract, no-recompile reload behavior, host ABI, receipts, and non-authorities.
- [x] V1 [serial] r[steel-repo-evolution-packs.verification.fixtures] Run focused fixtures/checker for absent, valid, malformed, hash-mismatched, path-escaped, unknown-host-call, and over-budget repo-local Steel packs. [evidence=evidence/verification.md]
- [x] V2 [serial] r[steel-repo-evolution-packs.verification.docs] Run docs build, Cairn gates, `cairn validate`, and diff checks. [evidence=evidence/verification.md]
