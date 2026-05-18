## Why

Clankers' embedded SDK now demonstrates product-owned model, tool, provider-adapter, and session-store seams. The next lego gap is permission policy: embedded products need stable, named capability-pack presets that explain what powers an embedded agent receives by default and that fail loudly when a supposedly safe preset expands.

The current `CapabilityPack` evidence exists, but the names are too generic for product embedding guidance (`tool-user`, `operator`) and do not match the canonical `embedded-composition-kits` requirement names. A small OpenSpec change should make the product-facing preset vocabulary explicit, provide exact capability snapshots, and keep dangerous packs opt-in.

## What Changes

- **Named presets**: Expose product-facing embedded capability-pack constructors for `embedding_safe`, `read_only`, `networkless_coding`, `project_local_edit`, and `human_approved_shell`.
- **Explicit policy data**: Convert each preset into deterministic, inspectable capability-policy data using the existing embedded capability vocabulary.
- **Snapshot rail**: Add focused tests that assert the exact allowed capability set for every preset so accidental expansion fails.
- **Danger boundary**: Add negative/boundary coverage showing dangerous packs include explicit opt-in capabilities while safe defaults do not.
- **Docs/acceptance**: Document the preset names and ensure the embedded SDK acceptance rail runs the focused capability-pack checks.

## Capabilities

### Modified Capabilities

- `embedded-composition-kits.capability-packs`: Promotes the named product-facing presets from broad spec language into checked API/test evidence.
- `embedded-composition-kits.acceptance-rail`: Ensures one-command embedded readiness verifies capability-pack snapshot boundaries.

## Impact

- **Files**: `crates/clankers-adapters/src/lib.rs`, embedded SDK docs/API inventory if public items change, and `scripts/check-embedded-agent-sdk.sh` if the rail needs a focused test step.
- **APIs**: Adds or aliases named constructors on `CapabilityPack`; no runtime shell, daemon, provider, DB, or TUI dependencies are introduced.
- **Testing**: Verify with `cargo test -p clankers-adapters --lib capability_pack`, `scripts/check-embedded-agent-sdk.sh`, OpenSpec validation, formatting, and `git diff --check`.
