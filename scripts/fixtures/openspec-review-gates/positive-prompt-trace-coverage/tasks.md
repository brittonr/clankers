## Phase 1: Implementation

- [ ] [serial] I1: Update embedded prompt lifecycle state handling.
- [ ] [serial] V1: Add fixture `fixtures/embedded-prompt-lifecycle.json` and helper `assert_embedded_prompt_lifecycle_fixture` for prompt traceability across normal and embedded prompt paths [r[openspec-review-gates.deterministic-verification-tasks]] [covers=openspec-review-gates.deterministic-verification-tasks].
- [ ] [serial] V2: Run command `scripts/check-embedded-prompt-lifecycle.rs fixtures/embedded-prompt-lifecycle.json` before closing the change [r[openspec-review-gates.deterministic-verification-tasks]] [covers=openspec-review-gates.deterministic-verification-tasks].
