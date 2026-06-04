# Tasks

## 1. Contract

- [x] [serial] [covers=embedded-composition-kits.capability-pack-composition] [evidence=openspec validate lego-05-capability-pack-composition --strict --json] Finalize the delta spec and design for this lego slice.

## 2. Implementation

- [x] [serial] [covers=embedded-composition-kits.capability-pack-composition] [evidence=scripts/check-capability-pack-composition.rs] Implement the narrowest product-facing brick or evidence rail without adding shell/runtime dependencies to green SDK crates.
- [x] [parallel] [covers=embedded-composition-kits.capability-pack-composition] [evidence=policy/embedded-lego/capability-pack-composition.json; policy/embedded-lego/lego-contracts.json] Update Nickel/exported policy coverage when this slice owns declarative policy, manifest shape, capability composition, or runtime-kind contracts.
- [x] [parallel] [covers=embedded-composition-kits.capability-pack-composition] [evidence=target/embedded-sdk-release/capability-pack-composition-receipt.json] Add content-addressed evidence for generated policies, fixtures, manifests, transcripts, inventories, or receipts that need drift detection.

## 3. Verification

- [x] [depends:implementation] [covers=embedded-composition-kits.capability-pack-composition] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp ./scripts/check-embedded-agent-sdk.sh] Run the embedded SDK acceptance rail when SDK boundaries, examples, receipts, catalogs, capability packs, or lego policy changed.
- [x] [depends:implementation] [covers=embedded-composition-kits.capability-pack-composition] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo fmt --check && git diff --check] Run formatting and whitespace checks.
- [x] [depends:implementation] [covers=embedded-composition-kits.capability-pack-composition] [evidence=openspec validate embedded-composition-kits --strict --json] Archive after implementation and canonical spec validation.

Completed: 2026-05-19T04:14:26Z
