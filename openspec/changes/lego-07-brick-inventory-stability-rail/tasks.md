# Tasks

## 1. Contract

- [ ] [serial] [covers=embedded-composition-kits.brick-contracts] [evidence=openspec validate <change> --strict --json] Finalize the delta spec and design for this lego slice.

## 2. Implementation

- [ ] [serial] [covers=embedded-composition-kits.brick-contracts] [evidence=focused Rust/example check] Implement the narrowest product-facing brick or evidence rail without adding shell/runtime dependencies to green SDK crates.
- [ ] [parallel] [covers=embedded-composition-kits.brick-contracts] [evidence=policy/embedded-lego update or documented no-op] Update Nickel/exported policy coverage when this slice owns declarative policy, manifest shape, capability composition, or runtime-kind contracts.
- [ ] [parallel] [covers=embedded-composition-kits.brick-contracts] [evidence=BLAKE3 receipt/hash assertion or documented no-op] Add content-addressed evidence for generated policies, fixtures, manifests, transcripts, inventories, or receipts that need drift detection.

## 3. Verification

- [ ] [depends:implementation] [covers=embedded-composition-kits.brick-contracts] [evidence=scripts/check-embedded-agent-sdk.sh] Run the embedded SDK acceptance rail when SDK boundaries, examples, receipts, catalogs, capability packs, or lego policy changed.
- [ ] [depends:implementation] [covers=embedded-composition-kits.brick-contracts] [evidence=cargo fmt --check && git diff --check] Run formatting and whitespace checks.
- [ ] [depends:implementation] [covers=embedded-composition-kits.brick-contracts] [evidence=openspec validate embedded-composition-kits --strict --json] Archive after implementation and canonical spec validation.
