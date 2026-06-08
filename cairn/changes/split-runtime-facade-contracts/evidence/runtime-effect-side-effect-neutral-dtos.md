Artifact-Type: validation-log
Task-ID: I19,V18
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved runtime effect/side-effect classification DTOs to neutral message contracts:

- Added `clanker_message::EffectAbilityClass` for effect handler capability classes.
- Added `clanker_message::SideEffectLevel` for tool descriptor side-effect levels, preserving `requires_confirmation()` and `default_effect_class()` behavior.
- Re-exported both DTOs through `clankers-runtime::effects` / `clankers-runtime::tools` / crate root so existing runtime public API paths remain available.
- Kept `EffectRequest`, `EffectCorrelationId`, tool catalog builders, effect handlers, artifact hashing, redaction policy, and executable runtime behavior in `clankers-runtime`; only reusable enum DTO ownership moved.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
cargo check -p clanker-message -p clankers-runtime -p clankers --tests
cargo test -p clanker-message effect_ability_class_roundtrip_preserves_kebab_case --lib
cargo test -p clanker-message side_effect_level_maps_default_confirmation_and_effect_classes --lib
cargo test -p clankers-runtime tool_catalog_capability_pack_matrix_does_not_expand_dangerous_packs --lib
cargo test -p clankers-runtime tool_descriptor_maps_known_dangerous_tools_to_specific_effect_classes --lib
scripts/check-runtime-facade-boundary.rs --write-inventory
scripts/check-runtime-facade-boundary.rs
cargo -q -Zscript scripts/check-runtime-facade-split.rs
scripts/check-message-contract-boundary.rs
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
cargo test -p clankers --no-run
nix run .#cairn -- gate tasks split-runtime-facade-contracts --root .
nix run .#cairn -- validate --root .
git diff --check
```

All listed commands exited 0.
