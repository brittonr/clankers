# Declarative tool catalog manifest

## Why

Make tool catalogs feel like snap-together product blocks: manifest data validates fail-closed, exports normalized metadata, then feeds EmbeddedToolCatalog.

## What Changes

The change defines a parser-neutral manifest/export pipeline, normalized BLAKE3 evidence, clear diagnostics, and runtime-neutral metadata that never starts tool runtimes during loading.

## Capabilities

### Modified Capabilities
- `embedded-composition-kits`: advances lego-style product composition while preserving green/yellow/red SDK boundaries.

## Impact

- **Files**: expected changes under `crates/clankers-adapters/`, `examples/`, `policy/embedded-lego/`, `scripts/`, docs, and targeted tests depending on the slice.
- **APIs**: prefer additive typed DTO/helpers; any public brick change requires inventory and migration evidence.
- **Dependencies**: generic SDK crates must not gain daemon, TUI, provider discovery, OAuth, DB/session ownership, plugin supervision, Matrix, iroh/P2P, or built-in tool bundle dependencies.
- **Testing**: verify with focused Rust/example checks plus `scripts/check-embedded-agent-sdk.sh`, `openspec validate embedded-composition-kits --strict --json`, `cargo fmt --check`, and `git diff --check`.
