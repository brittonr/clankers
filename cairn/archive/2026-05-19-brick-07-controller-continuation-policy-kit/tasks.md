## Phase 1: Contract and fixture shape

- [x] [serial] [covers=controller-continuation-policy.controller-continuation-policy-kit.boundary] [evidence=openspec validate brick-07-controller-continuation-policy-kit --strict --json] Finalize the proposal, design, and delta spec for `controller-continuation-policy-kit`.
- [x] [serial] [covers=controller-continuation-policy.controller-continuation-policy-kit.boundary] [evidence=source anchor readback: crates/clankers-controller/src/auto_test.rs, crates/clankers-controller/src/lib.rs] Identify the minimal source anchors and decide whether the brick is an example, policy/manifest, generated inventory, receipt validator, focused test, or a combination.

## Phase 2: Implementation evidence

- [x] [serial] [covers=controller-continuation-policy.controller-continuation-policy-kit.evidence] [evidence=cargo test -p clankers-controller controller_continuation_policy_kit_prioritizes_follow_ups_and_rejects_stale_effects] Implement the narrowest deterministic brick evidence for `controller-continuation-policy-kit` with at least one positive path.
- [x] [parallel] [covers=controller-continuation-policy.controller-continuation-policy-kit.evidence] [evidence=controller_continuation_policy_kit_prioritizes_follow_ups_and_rejects_stale_effects stale effect-id assertion] Add one fail-closed, denial, drift, or redaction case for the brick.
- [x] [parallel] [covers=controller-continuation-policy.controller-continuation-policy-kit.drift] [evidence=docs/src/reference/request-lifecycle.md; scripts/check-controller-continuation-policy-kit.rs; scripts/check-embedded-agent-sdk.sh] Update docs, policy, generated inventory, or receipt schemas that advertise the brick.

## Phase 3: Validation and archive

- [x] [depends:implementation] [covers=controller-continuation-policy.controller-continuation-policy-kit.evidence] [evidence=2026-05-19T03:06:36Z: ./scripts/check-controller-continuation-policy-kit.rs; cargo test -p clankers-controller controller_continuation_policy_kit_prioritizes_follow_ups_and_rejects_stale_effects] Run the focused verification for `controller-continuation-policy-kit` and capture the command in the archive note.
- [x] [depends:implementation] [covers=controller-continuation-policy.controller-continuation-policy-kit.drift] [evidence=2026-05-19T03:06:36Z: cargo fmt --check && git diff --check] Run formatting and whitespace checks.
- [x] [depends:implementation] [covers=controller-continuation-policy.controller-continuation-policy-kit.boundary] [evidence=2026-05-19T03:06:36Z: openspec validate controller-continuation-policy --strict --json] Promote the spec delta, validate the canonical spec, and archive the change when complete.
