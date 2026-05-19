## Why

Clankers has several lego-style embedded SDK recipes, capability packs, and acceptance checks, but there is not yet one deterministic product-facing receipt that summarizes what is safe to embed, which artifacts were checked, and which shell/runtime boundaries remain excluded. Without a receipt, every downstream product integration has to reconstruct readiness from docs, scripts, commits, and OpenSpec history.

This change adds a small release-evidence seam for embedded SDK consumers: a generated JSON receipt with commit/status metadata, hashed SDK artifacts, supported crate guidance, explicit boundary exclusions, and the commands that back readiness claims.

## What Changes

- Add a deterministic `scripts/emit-embedded-sdk-release-receipt.rs` helper that writes a JSON receipt under `target/embedded-sdk-release/` by default.
- Include BLAKE3 hashes and byte sizes for the embedded SDK guide, generated API inventory, canonical composition spec, acceptance scripts, and standalone embedded examples.
- Include green SDK crates, app-edge/yellow boundaries, red exclusions, and the maintained verification commands in the receipt.
- Wire the helper into `scripts/check-embedded-agent-sdk.sh` so the existing one-command rail emits release evidence whenever lego readiness is checked.
- Update docs/spec/tasks so product embedders know to capture this receipt after the acceptance rail before claiming readiness.

## Capabilities

### Modified Capabilities

- `embedded-composition-kits.acceptance-rail`: Adds machine-readable release receipt evidence to the existing embedded SDK acceptance command.
- `embedded-composition-kits.recipes`: Records standalone recipe artifacts in the release receipt so product-facing lego examples are traceable.

## Impact

- **Files**: `scripts/emit-embedded-sdk-release-receipt.rs`, `scripts/check-embedded-agent-sdk.sh`, `docs/src/tutorials/embedded-agent-sdk.md`, and `openspec/specs/embedded-composition-kits/spec.md` through archive sync.
- **APIs**: No Rust library API change. The new script is an evidence/helper surface.
- **Dependencies**: The receipt helper may use script-local `blake3`/`serde_json`; it must not add runtime dependencies to SDK crates.
- **Testing**: Verify with the receipt helper, `scripts/check-embedded-agent-sdk.sh`, `openspec validate add-embedded-release-receipt --strict --json`, `openspec validate embedded-composition-kits --strict --json` after archive, `cargo fmt --check`, and `git diff --check`.
