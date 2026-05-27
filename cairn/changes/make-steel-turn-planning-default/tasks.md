## Phase 1: Scaffold and baseline

- [x] S1 [serial] r[steel-turn-planning-config-activation.settings-surface.absent-default] r[steel-default-orchestration.policy-selected-default.default-selected] Capture current Steel default state and checked-in bundled profile/script evidence. [evidence=evidence/current-steel-status-2026-05-27.md]
- [x] S2 [serial] r[steel-turn-planning-config-activation.settings-surface.absent-default] r[steel-default-orchestration.policy-selected-default.default-selected] Define the Cairn proposal, design, and delta specs for making `steel.host.plan_turn` the default planner while preserving explicit disable.

## Phase 2: Implementation

- [x] I1 [serial] r[steel-turn-planning-config-activation.settings-surface.absent-default] Add a Rust-owned default resolver so absent `steelTurnPlanning` settings derive the bundled `policy/steel-default-orchestration` profile/script, required `steel.host.plan_turn` capability/ability, and receipt prefix.
- [x] I2 [serial] r[steel-turn-planning-config-activation.settings-surface.explicit-disabled] Preserve `steelTurnPlanning.enabled = false` as an explicit Rust-native opt-out with no Steel-authorship receipt.
- [x] I3 [serial] r[steel-turn-planning-config-activation.settings-surface.explicit-profile] Keep explicit profile/script settings flowing through the same activation helper and overriding the bundled default.
- [x] I4 [serial] r[steel-turn-planning-config-activation.profile-loader.bundled-default] Ensure bundled profile/script loading is source-controlled, BLAKE3 hash-bound in receipts, and constrained to allowed policy/script roots.
- [x] I5 [parallel] r[steel-default-orchestration.policy-selected-default.default-selected] Thread current-session turn resource/capability/UCAN data so default Steel planning can plan only `steel.host.plan_turn` and cannot request unrelated host actions.
- [x] I6 [parallel] r[steel-default-orchestration.rollout-evidence.default-current-seam] Update operator docs to state Steel turn planning is default, how to opt out, which profile/script is bundled, and what receipts prove.

## Phase 3: Verification

- [x] V1 [serial] r[steel-turn-planning-config-activation.settings-surface.absent-default] Add positive tests proving absent settings activate bundled Steel turn planning and emit redacted receipt metadata. [evidence=evidence/steel-default-rails.md]
- [x] V2 [serial] r[steel-turn-planning-config-activation.settings-surface.explicit-disabled] Add negative tests proving explicit disable uses Rust-native planning and emits no Steel-authorship receipt. [evidence=evidence/steel-default-rails.md]
- [x] V3 [serial] r[steel-turn-planning-config-activation.profile-loader.invalid-bundled-default] Add negative tests for missing/malformed/hash-mismatched/over-budget default profile or script failure before Steel execution. [evidence=evidence/steel-default-rails.md]
- [x] V4 [serial] r[steel-default-orchestration.rollout-evidence.default-current-seam] Run focused Steel rails: `./scripts/check-steel-default-orchestration.rs`, `./scripts/check-steel-turn-planning-config-activation.rs`, `./scripts/check-steel-agent-turn-wiring.rs`, `./scripts/check-steel-turn-planning-runtime-smoke.rs`, and `./scripts/check-steel-turn-planning-ucan-authority.rs`. [evidence=evidence/steel-default-rails.md]
- [x] V5 [serial] r[steel-default-orchestration.policy-selected-default.default-selected] r[steel-turn-planning-config-activation.settings-surface.absent-default] Run `mdbook build docs`, `nix run .#cairn -- gate proposal make-steel-turn-planning-default --root .`, `nix run .#cairn -- gate design make-steel-turn-planning-default --root .`, `nix run .#cairn -- gate tasks make-steel-turn-planning-default --root .`, `nix run .#cairn -- validate --root .`, and `git diff --check`. [evidence=evidence/final-validation.md]

## Traceability

- `steel-turn-planning-config-activation.settings-surface.absent-default` -> S1, S2, I1, V1, V5
- `steel-turn-planning-config-activation.settings-surface.explicit-disabled` -> I2, V2
- `steel-turn-planning-config-activation.settings-surface.explicit-profile` -> I3
- `steel-turn-planning-config-activation.profile-loader.bundled-default` -> I4
- `steel-turn-planning-config-activation.profile-loader.invalid-bundled-default` -> V3
- `steel-default-orchestration.policy-selected-default.default-selected` -> S1, S2, I5, V5
- `steel-default-orchestration.rollout-evidence.default-current-seam` -> I6, V4
