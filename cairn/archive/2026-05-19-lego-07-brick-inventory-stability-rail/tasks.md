# Tasks

## 1. Contract

- [x] [serial] [covers=embedded-composition-kits.brick-contracts] [evidence=openspec validate lego-07-brick-inventory-stability-rail --strict --json] Finalize the delta spec and design for this lego slice. ✅ completed: 2026-05-19T04:36:39Z

## 2. Implementation

- [x] [serial] [covers=embedded-composition-kits.brick-contracts] [evidence=./scripts/check-brick-inventory-stability.rs] Implement the narrowest product-facing brick or evidence rail without adding shell/runtime dependencies to green SDK crates. ✅ completed: 2026-05-19T04:36:39Z
- [x] [parallel] [covers=embedded-composition-kits.brick-contracts] [evidence=policy/embedded-lego/brick-inventory-stability.json] Update Nickel/exported policy coverage when this slice owns declarative policy, manifest shape, capability composition, or runtime-kind contracts. ✅ completed: 2026-05-19T04:36:39Z
- [x] [parallel] [covers=embedded-composition-kits.brick-contracts] [evidence=target/embedded-sdk-release/brick-inventory-stability-receipt.json stable_contract_blake3=c4ce29681a55f64539465eb6597e0cfb38f24ed98b784ca60a4ce130cea29958] Add content-addressed evidence for generated policies, fixtures, manifests, transcripts, inventories, or receipts that need drift detection. ✅ completed: 2026-05-19T04:36:39Z

## 3. Verification

- [x] [depends:implementation] [covers=embedded-composition-kits.brick-contracts] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp ./scripts/check-embedded-agent-sdk.sh] Run the embedded SDK acceptance rail when SDK boundaries, examples, receipts, catalogs, capability packs, or lego policy changed. ✅ completed: 2026-05-19T04:36:39Z
- [x] [depends:implementation] [covers=embedded-composition-kits.brick-contracts] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo fmt --check && git diff --check] Run formatting and whitespace checks. ✅ completed: 2026-05-19T04:36:39Z
- [x] [depends:implementation] [covers=embedded-composition-kits.brick-contracts] [evidence=openspec validate embedded-composition-kits --strict --json] Archive after implementation and canonical spec validation. ✅ completed: 2026-05-19T04:36:39Z
