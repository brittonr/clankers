# Tasks

## 1. Contract

- [ ] [serial] [covers=embedded-composition-kits.plugin-tool-runtime-separation] [evidence=openspec validate roi-07-separate-plugin-tool-runtimes --strict --json] Finalize the delta spec and design for `plugin-tool-runtime-separation`.

## 2. Implementation

- [ ] [serial] [covers=embedded-composition-kits.plugin-tool-runtime-separation] [evidence=focused Rust/doc/example check] Implement the narrowest product-facing contract slice without adding shell/runtime dependencies to green SDK crates.
- [ ] [parallel] [covers=embedded-composition-kits.plugin-tool-runtime-separation] [evidence=Nickel export/check or documented no-op] Add Nickel contract/export coverage where this change owns declarative policy or manifest shape.
- [ ] [parallel] [covers=embedded-composition-kits.plugin-tool-runtime-separation] [evidence=BLAKE3 receipt/hash assertion or documented no-op] Add BLAKE3 evidence for generated policies, fixtures, manifests, transcripts, or receipts that need drift detection.

## 3. Verification

- [ ] [depends:implementation] [covers=embedded-composition-kits.plugin-tool-runtime-separation] [evidence=scripts/check-embedded-agent-sdk.sh] Run the embedded SDK acceptance rail if SDK boundaries, examples, receipts, catalogs, or capability packs changed.
- [ ] [depends:implementation] [covers=embedded-composition-kits.plugin-tool-runtime-separation] [evidence=cargo fmt --check && git diff --check] Run formatting and whitespace checks.
- [ ] [depends:implementation] [covers=embedded-composition-kits.plugin-tool-runtime-separation] [evidence=openspec validate embedded-composition-kits --strict --json] Archive after implementation and canonical spec validation.
