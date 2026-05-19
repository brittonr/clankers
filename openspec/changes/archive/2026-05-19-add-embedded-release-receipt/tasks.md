## Phase 1: Receipt contract

- [x] [serial] Define the embedded SDK release receipt fields: commit/status metadata, verification commands, boundary classifications, and BLAKE3 artifact entries. [covers=embedded-composition-kits.acceptance-rail.release-receipt] [evidence=scripts/emit-embedded-sdk-release-receipt.rs]
- [x] [depends:receipt-contract] Add deterministic artifact collection for embedded SDK docs, generated API inventory, canonical spec, acceptance scripts, and standalone embedded examples. [covers=embedded-composition-kits.acceptance-rail.release-receipt.artifacts] [evidence=scripts/emit-embedded-sdk-release-receipt.rs]

## Phase 2: Acceptance rail and docs

- [x] [depends:receipt-helper] Wire the receipt helper into `scripts/check-embedded-agent-sdk.sh` without changing SDK crate runtime dependencies. [covers=embedded-composition-kits.acceptance-rail.one-command] [evidence=scripts/check-embedded-agent-sdk.sh]
- [x] [parallel] Document how product embedders capture and interpret the receipt, including green/yellow/red boundaries and clean-checkout release guidance. [covers=embedded-composition-kits.acceptance-rail.release-receipt] [evidence=docs/src/tutorials/embedded-agent-sdk.md]

## Phase 3: Verification and archive

- [x] [depends:acceptance-docs] Run the receipt helper, `scripts/check-embedded-agent-sdk.sh`, `cargo fmt --check`, `git diff --check`, and OpenSpec validation. [covers=embedded-composition-kits.acceptance-rail.release-receipt] [evidence=target/embedded-sdk-release/test-receipt.json]
- [x] [depends:verification] Promote/sync the `embedded-composition-kits` spec delta and archive this change after all tasks are complete. [evidence=openspec validate add-embedded-release-receipt --strict --json]
