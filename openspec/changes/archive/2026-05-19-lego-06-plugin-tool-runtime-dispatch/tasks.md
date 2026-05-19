# Tasks

## 1. Contract

- [x] [serial] [covers=embedded-composition-kits.plugin-tool-runtime-separation] [evidence=openspec validate lego-06-plugin-tool-runtime-dispatch --strict --json] Finalize the delta spec and design for this lego slice. ✅ (completed: 2026-05-19T04:29:07Z)

## 2. Implementation

- [x] [serial] [covers=embedded-composition-kits.plugin-tool-runtime-separation] [evidence=cargo test -p clankers-plugin --lib plugin_runtime_dispatch_kit_keeps_non_extism_out_of_wasm_loader] Implement the narrowest product-facing brick or evidence rail without adding shell/runtime dependencies to green SDK crates. ✅ (completed: 2026-05-19T04:29:07Z)
- [x] [parallel] [covers=embedded-composition-kits.plugin-tool-runtime-separation] [evidence=policy/embedded-lego/plugin-runtime-dispatch.json + policy/embedded-lego/lego-contracts.{json,ncl}] Update Nickel/exported policy coverage when this slice owns declarative policy, manifest shape, capability composition, or runtime-kind contracts. ✅ (completed: 2026-05-19T04:29:07Z)
- [x] [parallel] [covers=embedded-composition-kits.plugin-tool-runtime-separation] [evidence=scripts/check-plugin-runtime-dispatch.rs] Add content-addressed evidence for generated policies, fixtures, manifests, transcripts, inventories, or receipts that need drift detection. ✅ (completed: 2026-05-19T04:29:07Z)

## 3. Verification

- [x] [depends:implementation] [covers=embedded-composition-kits.plugin-tool-runtime-separation] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp ./scripts/check-embedded-agent-sdk.sh] Run the embedded SDK acceptance rail when SDK boundaries, examples, receipts, catalogs, capability packs, or lego policy changed. ✅ (completed: 2026-05-19T04:29:07Z)
- [x] [depends:implementation] [covers=embedded-composition-kits.plugin-tool-runtime-separation] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo fmt --check && git diff --check] Run formatting and whitespace checks. ✅ (completed: 2026-05-19T04:29:07Z)
- [x] [depends:implementation] [covers=embedded-composition-kits.plugin-tool-runtime-separation] [evidence=openspec validate embedded-composition-kits --strict --json] Archive after implementation and canonical spec validation. ✅ (completed: 2026-05-19T04:29:07Z)
