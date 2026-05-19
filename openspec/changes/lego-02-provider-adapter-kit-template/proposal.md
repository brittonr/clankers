# Provider adapter kit template

## Why

Turn the product-owned provider adapter example into a copyable kit template with explicit fixtures and outcome mapping.

## What Changes

The change adds template-level DTO/fixture guidance for request conversion, completed/retryable/terminal outcomes, usage accounting, and model capability declarations while keeping clankers-provider/router out of green SDK crates.

## Capabilities

### Modified Capabilities
- `embedded-composition-kits`: advances lego-style product composition while preserving green/yellow/red SDK boundaries.

## Impact

- **Files**: expected changes under `crates/clankers-adapters/`, `examples/`, `policy/embedded-lego/`, `scripts/`, docs, and targeted tests depending on the slice.
- **APIs**: prefer additive typed DTO/helpers; any public brick change requires inventory and migration evidence.
- **Dependencies**: generic SDK crates must not gain daemon, TUI, provider discovery, OAuth, DB/session ownership, plugin supervision, Matrix, iroh/P2P, or built-in tool bundle dependencies.
- **Testing**: verify with focused Rust/example checks plus `scripts/check-embedded-agent-sdk.sh`, `openspec validate embedded-composition-kits --strict --json`, `cargo fmt --check`, and `git diff --check`.
