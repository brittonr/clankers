## Why

The validation-first voice surface documents unsupported capture. Hermes parity requires a real local/live speech-to-text loop that remains explicit about audio retention, provider policy, and transcript handling.

## What Changes

- Add local microphone/file capture pipeline behind explicit enablement.
- Add STT provider adapter with local-first/default-off cloud policy.
- Connect transcribed prompts to normal sessions and optional TTS replies.

## Out of Scope

- Always-on microphone capture.
- Persisting raw audio or transcripts without explicit retention policy.

## Capabilities

### New Capabilities
- `voice-mode` follow-up behavior for add live voice stt mode.

### Modified Capabilities
- `voice-mode` gains implementation-ready requirements for the next Hermes parity slice.

## Impact

- **Files**: OpenSpec artifacts first; implementation tasks identify expected Rust/docs touch points.
- **APIs**: May add CLI flags, tool schemas, settings fields, or daemon/session messages as described in the design.
- **Testing**: Targeted unit/integration checks plus `cargo check --tests` for touched crates.
