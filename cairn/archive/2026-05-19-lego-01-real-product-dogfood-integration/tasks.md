# Tasks

## 1. Contract

- [x] [serial] [covers=embedded-composition-kits.real-product-dogfood] [evidence=openspec validate lego-01-real-product-dogfood-integration --strict --json] Finalize the delta spec and design for this lego slice.

## 2. Implementation

- [x] [serial] [covers=embedded-composition-kits.real-product-dogfood] [evidence=examples/embedded-product-workbench/dogfood-manifest.json; scripts/check-real-product-dogfood.rs; cargo run --locked --manifest-path examples/embedded-product-workbench/Cargo.toml] Implement the narrowest product-facing brick or evidence rail without adding shell/runtime dependencies to green SDK crates.
- [x] [parallel] [covers=embedded-composition-kits.real-product-dogfood] [evidence=policy/embedded-lego/lego-contracts.json; scripts/check-embedded-lego-contracts.rs] Update Nickel/exported policy coverage when this slice owns declarative policy, manifest shape, capability composition, or runtime-kind contracts.
- [x] [parallel] [covers=embedded-composition-kits.real-product-dogfood] [evidence=scripts/check-real-product-dogfood.rs emits target/embedded-sdk-release/product-dogfood/receipt.json] Add content-addressed evidence for generated policies, fixtures, manifests, transcripts, inventories, or receipts that need drift detection.

## 3. Verification

- [x] [depends:implementation] [covers=embedded-composition-kits.real-product-dogfood] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp ./scripts/check-embedded-agent-sdk.sh] Run the embedded SDK acceptance rail when SDK boundaries, examples, receipts, catalogs, capability packs, or lego policy changed.
- [x] [depends:implementation] [covers=embedded-composition-kits.real-product-dogfood] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo fmt --check && git diff --check] Run formatting and whitespace checks.
- [x] [depends:implementation] [covers=embedded-composition-kits.real-product-dogfood] [evidence=openspec validate embedded-composition-kits --strict --json; 2026-05-19T03:41:36Z] Archive after implementation and canonical spec validation.
