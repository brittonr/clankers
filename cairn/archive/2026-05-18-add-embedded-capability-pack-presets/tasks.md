## Phase 1: Spec and API shape

- [x] [serial] Define the named embedded capability-pack preset contract for `embedding_safe`, `read_only`, `networkless_coding`, `project_local_edit`, and `human_approved_shell`. [covers=embedded-composition-kits.capability-packs.no-expansion] [evidence=openspec/changes/archive/2026-05-18-add-embedded-capability-pack-presets/specs/embedded-composition-kits/spec.md]
- [x] [depends:spec] Add or alias product-facing `CapabilityPack` constructors without importing shell/runtime dependencies. [covers=embedded-composition-kits.capability-packs.no-expansion] [evidence=crates/clankers-adapters/src/lib.rs]

## Phase 2: Snapshot and boundary tests

- [x] [depends:api] Add exact snapshot tests for every named preset's ordered capability set. [covers=embedded-composition-kits.capability-packs.no-expansion] [evidence=cargo test -p clankers-adapters --lib capability_pack]
- [x] [depends:api] Add a negative/boundary assertion that safe presets exclude explicit opt-in capabilities while `human_approved_shell` contains only explicit dangerous capabilities behind an opt-in name. [covers=embedded-composition-kits.capability-packs.explicit-danger] [evidence=cargo test -p clankers-adapters --lib capability_pack]

## Phase 3: Docs and acceptance

- [x] [depends:tests] Update embedded SDK docs/API inventory if needed so products see the named presets and danger boundary. [covers=embedded-composition-kits.recipes.crate-guidance] [evidence=docs/src/tutorials/embedded-agent-sdk.md]
- [x] [depends:tests] Ensure `scripts/check-embedded-agent-sdk.sh` runs the focused capability-pack rail. [covers=embedded-composition-kits.acceptance-rail.one-command] [evidence=scripts/check-embedded-agent-sdk.sh]

## Phase 4: Verification and archive

- [x] [depends:acceptance] Run focused adapter tests, embedded SDK acceptance, formatting/diff checks, and OpenSpec validation. [covers=embedded-composition-kits.acceptance-rail.one-command] [evidence=scripts/check-embedded-agent-sdk.sh]
- [x] [depends:verification] Promote/sync the canonical `embedded-composition-kits` spec and archive the change when complete. [evidence=openspec validate add-embedded-capability-pack-presets --strict --json]
