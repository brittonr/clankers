## Context

The embedded SDK work has converged on a small set of green crates and executable examples. Product teams need a durable readiness artifact that can be attached to a release note, internal integration ticket, or downstream audit without replaying the entire conversation/history.

## Decisions

1. **Receipt is generated, not checked in by default.** The helper writes to `target/embedded-sdk-release/receipt.json` unless `--output <path>` is supplied. This keeps working trees clean during normal acceptance runs while still producing a concrete artifact for downstream release evidence.
2. **Receipt hashes source evidence, not build output.** The helper hashes docs, canonical spec, acceptance scripts, and standalone example manifests/source files with BLAKE3. This makes the receipt stable and reviewable without depending on target-directory build products.
3. **Receipt names boundaries explicitly.** The JSON includes green crates, yellow app-edge surfaces, and red exclusions so product embedders can see what is supported versus what must stay outside generic SDK dependencies.
4. **Acceptance rail emits the receipt.** `scripts/check-embedded-agent-sdk.sh` calls the helper after structural checks and before/alongside executable examples, so every lego readiness run leaves machine-readable evidence.
5. **No new library API.** This is a release-evidence seam, not a generic storage/tool/provider abstraction. It should not move shell/runtime concepts into SDK crates.

## Verification Plan

- Run `scripts/emit-embedded-sdk-release-receipt.rs --output target/embedded-sdk-release/test-receipt.json` and inspect that JSON contains commit/status metadata, verification commands, boundary classifications, and BLAKE3 artifact entries.
- Run `scripts/check-embedded-agent-sdk.sh` to prove the receipt helper is wired into the maintained acceptance rail.
- Run focused OpenSpec validation for this change and for the canonical `embedded-composition-kits` spec after archive.
- Run `cargo fmt --check` and `git diff --check`.

## Risks

- **Overclaiming readiness:** The receipt is evidence for the maintained embedded rail, not a promise that daemon/TUI/provider runtime crates are generic SDK APIs.
- **Dirty worktree confusion:** The receipt records `git status --short --branch` rather than requiring cleanliness, so development runs remain useful. Release consumers should capture the receipt from a clean committed checkout.
- **Artifact drift:** Hashing the docs/spec/scripts/examples makes drift visible. If new embedded examples become part of the supported surface, the receipt artifact list must be updated with the acceptance rail.
