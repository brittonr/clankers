## Why

A first-pass ACP stdio adapter is not enough for Hermes-like editor integration. Clankers should map editor prompts, diffs, terminal requests, and tool activity onto normal session/controller surfaces.

## What Changes

- Extend ACP serve to attach/create real clankers sessions.
- Expose safe tool activity, diffs, and terminal/workspace capability negotiation.
- Keep editor transport separate from model-callable tools.

## Out of Scope

- Editor-origin requests bypassing confirmation or tool policy.
- Logging raw editor buffers or prompts in metadata.

## Capabilities

### New Capabilities
- `acp-ide-integration` follow-up behavior for complete acp ide session integration.

### Modified Capabilities
- `acp-ide-integration` gains implementation-ready requirements for the next Hermes parity slice.

## Impact

- **Files**: OpenSpec artifacts first; implementation tasks identify expected Rust/docs touch points.
- **APIs**: May add CLI flags, tool schemas, settings fields, or daemon/session messages as described in the design.
- **Testing**: Targeted unit/integration checks plus `cargo check --tests` for touched crates.
