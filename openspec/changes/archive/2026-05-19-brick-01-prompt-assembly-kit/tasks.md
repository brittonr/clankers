## Phase 1: Contract and fixture shape

- [x] [serial] [covers=prompt-assembly.prompt-assembly-kit.boundary] [evidence=openspec validate brick-01-prompt-assembly-kit --strict --json] Finalized the proposal, design, and delta spec for `prompt-assembly-kit`. ✅ completed: 2026-05-19T02:08:33Z
- [x] [serial] [covers=prompt-assembly.prompt-assembly-kit.boundary] [evidence=source anchor readback] Identified the minimal source anchors and chose a copyable example plus embedded SDK rail/docs update: `crates/clankers-runtime/src/prompt.rs`, `docs/src/reference/embedding.md`, and `scripts/check-embedded-agent-sdk.sh`. ✅ completed: 2026-05-19T02:08:33Z

## Phase 2: Implementation evidence

- [x] [serial] [covers=prompt-assembly.prompt-assembly-kit.evidence] [evidence=cargo run --locked --manifest-path examples/prompt-assembly-kit/Cargo.toml] Implemented `examples/prompt-assembly-kit/`, a deterministic host-context-only prompt assembly recipe with BLAKE3 receipt hash output. ✅ completed: 2026-05-19T02:08:33Z
- [x] [parallel] [covers=prompt-assembly.prompt-assembly-kit.evidence] [evidence=negative fixture or fail-closed assertion] Added fail-closed assertions for disabled ambient filesystem discovery and redaction assertions for secret-like host/system context. ✅ completed: 2026-05-19T02:08:33Z
- [x] [parallel] [covers=prompt-assembly.prompt-assembly-kit.drift] [evidence=docs/policy/generated inventory update or documented no-op] Documented the checked recipe in `docs/src/reference/embedding.md` and added it to `scripts/check-embedded-agent-sdk.sh`. ✅ completed: 2026-05-19T02:08:33Z

## Phase 3: Validation and archive

- [x] [depends:implementation] [covers=prompt-assembly.prompt-assembly-kit.evidence] [evidence=cargo run --locked --manifest-path examples/prompt-assembly-kit/Cargo.toml] Ran the focused verification; receipt hash `669c85eca33d254d8210c40f67e58931495c508d2bb430ba36d265c08c5de3c5`. ✅ completed: 2026-05-19T02:08:33Z
- [x] [depends:implementation] [covers=prompt-assembly.prompt-assembly-kit.drift] [evidence=cargo fmt --manifest-path examples/prompt-assembly-kit/Cargo.toml && git diff --check] Ran formatting and whitespace checks. ✅ completed: 2026-05-19T02:08:33Z
- [x] [depends:implementation] [covers=prompt-assembly.prompt-assembly-kit.boundary] [evidence=openspec validate prompt-assembly --strict --json] Promoted the spec delta, validated the canonical spec, and prepared the change for archive. ✅ completed: 2026-05-19T02:08:33Z
