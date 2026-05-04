## Why

The browser automation baseline defines a policy seam, but Hermes parity requires a concrete stateful backend that can operate real local Chromium/CDP sessions for web debugging and app interaction.

## What Changes

- Implement a local CDP runtime for navigation, snapshot/current URL, click, type, screenshot, close, and gated evaluate.
- Publish the browser tool through shared tool construction when config enables it.
- Add deterministic fake-runtime tests plus optional live-browser smoke coverage.

## Out of Scope

- Remote browser providers.
- Bypassing origin/evaluate/screenshot policy.

## Capabilities

### New Capabilities
- `browser-automation` follow-up behavior for complete browser automation cdp backend.

### Modified Capabilities
- `browser-automation` gains implementation-ready requirements for the next Hermes parity slice.

## Impact

- **Files**: OpenSpec artifacts first; implementation tasks identify expected Rust/docs touch points.
- **APIs**: May add CLI flags, tool schemas, settings fields, or daemon/session messages as described in the design.
- **Testing**: Targeted unit/integration checks plus `cargo check --tests` for touched crates.
