# Tasks

## 1. Contract

- [x] [serial] [covers=embedded-composition-kits.provider-adapter-kit] [evidence=openspec validate lego-02-provider-adapter-kit-template --strict --json] Finalize the delta spec and design for this lego slice.

## 2. Implementation

- [x] [serial] [covers=embedded-composition-kits.provider-adapter-kit] [evidence=examples/embedded-provider-adapter/fixtures/provider-adapter-fixtures.json; examples/embedded-provider-adapter/src/main.rs; scripts/check-provider-adapter-kit.rs] Implement the narrowest product-facing brick or evidence rail without adding shell/runtime dependencies to green SDK crates.
- [x] [parallel] [covers=embedded-composition-kits.provider-adapter-kit] [evidence=policy/embedded-lego/lego-contracts.json; scripts/check-embedded-lego-contracts.rs] Update Nickel/exported policy coverage when this slice owns declarative policy, manifest shape, capability composition, or runtime-kind contracts.
- [x] [parallel] [covers=embedded-composition-kits.provider-adapter-kit] [evidence=scripts/check-provider-adapter-kit.rs emits target/embedded-sdk-release/provider-adapter-kit-receipt.json] Add content-addressed evidence for generated policies, fixtures, manifests, transcripts, inventories, or receipts that need drift detection.

## 3. Verification

- [x] [depends:implementation] [covers=embedded-composition-kits.provider-adapter-kit] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp ./scripts/check-embedded-agent-sdk.sh] Run the embedded SDK acceptance rail when SDK boundaries, examples, receipts, catalogs, capability packs, or lego policy changed.
- [x] [depends:implementation] [covers=embedded-composition-kits.provider-adapter-kit] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo fmt --check && git diff --check] Run formatting and whitespace checks.
- [x] [depends:implementation] [covers=embedded-composition-kits.provider-adapter-kit] [evidence=openspec validate embedded-composition-kits --strict --json; 2026-05-19T03:45:50Z] Archive after implementation and canonical spec validation.
