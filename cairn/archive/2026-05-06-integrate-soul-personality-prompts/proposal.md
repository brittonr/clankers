## Why

The SOUL/personality validation surface is intentionally non-mutating. Hermes parity requires safe prompt integration for local SOUL.md and curated personality presets with clear precedence relative to AGENTS.md and CLAUDE.md.

## What Changes

- Add prompt assembly integration for local SOUL.md and validated presets.
- Define precedence, opt-in/disable controls, and metadata.
- Reject remote or command-executed persona sources.

## Out of Scope

- Fetching remote personality prompts.
- Persisting raw persona contents in debug metadata.

## Capabilities

### New Capabilities
- `soul-personality-system` follow-up behavior for integrate soul personality prompts.

### Modified Capabilities
- `soul-personality-system` gains implementation-ready requirements for the next Hermes parity slice.

## Impact

- **Files**: OpenSpec artifacts first; implementation tasks identify expected Rust/docs touch points.
- **APIs**: May add CLI flags, tool schemas, settings fields, or daemon/session messages as described in the design.
- **Testing**: Targeted unit/integration checks plus `cargo check --tests` for touched crates.
