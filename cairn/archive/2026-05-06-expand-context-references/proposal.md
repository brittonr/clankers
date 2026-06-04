## Why

Local file and directory reference expansion improves prompts, but Hermes-like ergonomics need safe references for diffs, URLs, sessions, and generated artifacts with provenance metadata.

## What Changes

- Extend @ expansion to git diffs, URLs, session ids, and session artifacts.
- Preserve deterministic prompt expansion and safe metadata.
- Add failure modes for unsupported remote or oversized references.

## Out of Scope

- Implicit credentialed network fetches.
- Persisting raw secret-bearing content in metadata.

## Capabilities

### New Capabilities
- `context-references` follow-up behavior for expand context references.

### Modified Capabilities
- `context-references` gains implementation-ready requirements for the next Hermes parity slice.

## Impact

- **Files**: OpenSpec artifacts first; implementation tasks identify expected Rust/docs touch points.
- **APIs**: May add CLI flags, tool schemas, settings fields, or daemon/session messages as described in the design.
- **Testing**: Targeted unit/integration checks plus `cargo check --tests` for touched crates.
