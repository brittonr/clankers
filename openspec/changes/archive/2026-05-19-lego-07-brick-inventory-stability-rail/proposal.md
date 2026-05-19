# Brick inventory stability rail

## Why

Harden the public brick inventory so advertised green SDK entrypoints cannot drift without docs, tests, and receipt updates.

## What Changes

The change turns the embedded API inventory into a semver-facing brick stability rail with explicit supported/unsupported classification, migration-note requirements, and receipt hashes.

## Capabilities

### Modified Capabilities
- `embedded-composition-kits`: advances lego-style product composition while preserving green/yellow/red SDK boundaries.

## Impact

- **Files**: expected changes under `crates/clankers-adapters/`, `examples/`, `policy/embedded-lego/`, `scripts/`, docs, and targeted tests depending on the slice.
- **APIs**: prefer additive typed DTO/helpers; any public brick change requires inventory and migration evidence.
- **Dependencies**: generic SDK crates must not gain daemon, TUI, provider discovery, OAuth, DB/session ownership, plugin supervision, Matrix, iroh/P2P, or built-in tool bundle dependencies.
- **Testing**: verify with focused Rust/example checks plus `scripts/check-embedded-agent-sdk.sh`, `openspec validate embedded-composition-kits --strict --json`, `cargo fmt --check`, and `git diff --check`.
