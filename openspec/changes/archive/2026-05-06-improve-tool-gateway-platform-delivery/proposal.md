## Why

Hermes exposes strong toolset controls and platform-aware delivery. Clankers needs a unified capability surface so TUI, daemon, remote attach, scheduled tasks, and platform bridges agree on tool availability and media/file delivery behavior.

## What Changes

- Define shared toolset enablement and disabled-tool policy across modes.
- Add platform-aware delivery receipts for files, media, and scheduled outputs.
- Keep daemon/remote attach parity with standalone local state.

## Out of Scope

- Leaking platform tokens or delivery destinations in session metadata.
- Mode-specific ad hoc tool policy that bypasses shared construction.

## Capabilities

### New Capabilities
- `tool-gateway-platform-delivery` follow-up behavior for improve tool gateway platform delivery.

### Modified Capabilities
- `tool-gateway-platform-delivery` gains implementation-ready requirements for the next Hermes parity slice.

## Impact

- **Files**: OpenSpec artifacts first; implementation tasks identify expected Rust/docs touch points.
- **APIs**: May add CLI flags, tool schemas, settings fields, or daemon/session messages as described in the design.
- **Testing**: Targeted unit/integration checks plus `cargo check --tests` for touched crates.
