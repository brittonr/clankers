# Plugin/tool runtime dispatch separation

## Why

Make runtime kinds explicit swappable lego blocks instead of routing non-Extism tools through plugin/WASM loaders.

## What Changes

The change defines a dispatch matrix for Extism, stdio, built-in, and product-owned executors; checked launch-policy metadata; and BLAKE3 evidence for runtime-kind fixtures.

## Capabilities

### Modified Capabilities
- `embedded-composition-kits`: advances lego-style product composition while preserving green/yellow/red SDK boundaries.

## Impact

- **Files**: expected changes under `crates/clankers-adapters/`, `examples/`, `policy/embedded-lego/`, `scripts/`, docs, and targeted tests depending on the slice.
- **APIs**: prefer additive typed DTO/helpers; any public brick change requires inventory and migration evidence.
- **Dependencies**: generic SDK crates must not gain daemon, TUI, provider discovery, OAuth, DB/session ownership, plugin supervision, Matrix, iroh/P2P, or built-in tool bundle dependencies.
- **Testing**: verify with focused Rust/example checks plus `scripts/check-embedded-agent-sdk.sh`, `openspec validate embedded-composition-kits --strict --json`, `cargo fmt --check`, and `git diff --check`.
