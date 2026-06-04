# Tasks

## 1. Contract

- [x] [serial] [covers=embedded-composition-kits.real-product-dogfood] [evidence=openspec validate roi-01-dogfood-real-product-integration --strict --json] ✅ implemented by 8ea4b8be scaffold + scripts/check-embedded-lego-contracts.rs evidence. Finalize the delta spec and design for `real-product-dogfood`.

## 2. Implementation

- [x] [serial] [covers=embedded-composition-kits.real-product-dogfood] [evidence=focused Rust/doc/example check] ✅ implemented by policy/embedded-lego/* + scripts/check-embedded-lego-contracts.rs. Implement the narrowest product-facing contract slice without adding shell/runtime dependencies to green SDK crates.
- [x] [parallel] [covers=embedded-composition-kits.real-product-dogfood] [evidence=Nickel export/check or documented no-op] ✅ evidence=policy/embedded-lego/lego-contracts.ncl + policy/embedded-lego/lego-contracts.json. Add Nickel contract/export coverage where this change owns declarative policy or manifest shape.
- [x] [parallel] [covers=embedded-composition-kits.real-product-dogfood] [evidence=BLAKE3 receipt/hash assertion or documented no-op] ✅ evidence=target/embedded-sdk-release/lego-contracts-receipt.json from scripts/check-embedded-lego-contracts.rs. Add BLAKE3 evidence for generated policies, fixtures, manifests, transcripts, or receipts that need drift detection.

## 3. Verification

- [x] [depends:implementation] [covers=embedded-composition-kits.real-product-dogfood] [evidence=scripts/check-embedded-agent-sdk.sh] ✅ evidence=scripts/check-embedded-lego-contracts.rs; embedded rail includes the checker. Run the embedded SDK acceptance rail if SDK boundaries, examples, receipts, catalogs, or capability packs changed.
- [x] [depends:implementation] [covers=embedded-composition-kits.real-product-dogfood] [evidence=cargo fmt --check && git diff --check] ✅ evidence=git diff --check. Run formatting and whitespace checks.
- [x] [depends:implementation] [covers=embedded-composition-kits.real-product-dogfood] [evidence=openspec validate embedded-composition-kits --strict --json] ✅ evidence=openspec validate embedded-composition-kits --strict --json. Archive after implementation and canonical spec validation.
