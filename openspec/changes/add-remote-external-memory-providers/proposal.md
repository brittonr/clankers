## Why

Local external-memory search is useful, but Hermes parity needs optional remote memory/personalization providers without weakening curated local memory boundaries.

## What Changes

- Add HTTP/provider adapter configuration behind disabled-by-default policy.
- Support search/status with credential-env based authentication.
- Keep prompt injection opt-in and metadata safe.

## Out of Scope

- Implicit remote memory writes.
- Sending local curated memory to remote providers unless explicitly requested by a future spec.

## Capabilities

### New Capabilities
- `external-memory-providers` follow-up behavior for add remote external memory providers.

### Modified Capabilities
- `external-memory-providers` gains implementation-ready requirements for the next Hermes parity slice.

## Impact

- **Files**: OpenSpec artifacts first; implementation tasks identify expected Rust/docs touch points.
- **APIs**: May add CLI flags, tool schemas, settings fields, or daemon/session messages as described in the design.
- **Testing**: Targeted unit/integration checks plus `cargo check --tests` for touched crates.
