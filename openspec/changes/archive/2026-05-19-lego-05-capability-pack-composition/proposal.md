# Capability pack composition

## Why

Let products compose named capability packs predictably while preserving explicit danger boundaries.

## What Changes

The change defines deterministic merge ordering, conflict diagnostics, approval-policy handling, exact capability snapshots, and Nickel-authored pack policy exports consumed as Rust data.

## Capabilities

### Modified Capabilities
- `embedded-composition-kits`: advances lego-style product composition while preserving green/yellow/red SDK boundaries.

## Impact

- **Files**: expected changes under `crates/clankers-adapters/`, `examples/`, `policy/embedded-lego/`, `scripts/`, docs, and targeted tests depending on the slice.
- **APIs**: prefer additive typed DTO/helpers; any public brick change requires inventory and migration evidence.
- **Dependencies**: generic SDK crates must not gain daemon, TUI, provider discovery, OAuth, DB/session ownership, plugin supervision, Matrix, iroh/P2P, or built-in tool bundle dependencies.
- **Testing**: verify with focused Rust/example checks plus `scripts/check-embedded-agent-sdk.sh`, `openspec validate embedded-composition-kits --strict --json`, `cargo fmt --check`, and `git diff --check`.
