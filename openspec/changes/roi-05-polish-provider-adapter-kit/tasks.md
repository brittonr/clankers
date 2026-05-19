# Tasks

## 1. Contract

- [ ] [serial] [covers=embedded-composition-kits.provider-adapter-kit] [evidence=openspec validate roi-05-polish-provider-adapter-kit --strict --json] Finalize the delta spec and design for `provider-adapter-kit`.

## 2. Implementation

- [ ] [serial] [covers=embedded-composition-kits.provider-adapter-kit] [evidence=focused Rust/doc/example check] Implement the narrowest product-facing contract slice without adding shell/runtime dependencies to green SDK crates.
- [ ] [parallel] [covers=embedded-composition-kits.provider-adapter-kit] [evidence=Nickel export/check or documented no-op] Add Nickel contract/export coverage where this change owns declarative policy or manifest shape.
- [ ] [parallel] [covers=embedded-composition-kits.provider-adapter-kit] [evidence=BLAKE3 receipt/hash assertion or documented no-op] Add BLAKE3 evidence for generated policies, fixtures, manifests, transcripts, or receipts that need drift detection.

## 3. Verification

- [ ] [depends:implementation] [covers=embedded-composition-kits.provider-adapter-kit] [evidence=scripts/check-embedded-agent-sdk.sh] Run the embedded SDK acceptance rail if SDK boundaries, examples, receipts, catalogs, or capability packs changed.
- [ ] [depends:implementation] [covers=embedded-composition-kits.provider-adapter-kit] [evidence=cargo fmt --check && git diff --check] Run formatting and whitespace checks.
- [ ] [depends:implementation] [covers=embedded-composition-kits.provider-adapter-kit] [evidence=openspec validate embedded-composition-kits --strict --json] Archive after implementation and canonical spec validation.
