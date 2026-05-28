## Phase 1: Runtime execution substrate

- [x] [serial] I1: Replace direct `ModelAdapter::complete` session execution with an engine-host execution path that builds `EnginePromptSubmission` from assembled prompt and replay context. [covers=r[runtime-engine-host.engine-host-delegation.submit-prompt]]
- [x] [serial] I2: Define runtime-owned adapter slots for model, tool, retry, event, cancellation, and usage services, with compatibility shims for existing `ModelAdapter` and echo defaults. [covers=r[runtime-engine-host.engine-host-delegation.host-adapters]]
- [x] [parallel] I3: Project engine-host progress, assistant/thinking deltas, tool lifecycle, usage, errors, and terminal report data into safe `SessionEvent` values. [covers=r[runtime-engine-host.session-events.engine-projection]]
- [x] [serial] I4: Update batch/headless runtime callers and embedded examples to use the real runtime engine-host path without daemon/TUI/provider discovery. [covers=r[runtime-engine-host.adapter-parity.batch-headless]]

## Phase 2: Verification

- [x] [parallel] V1: Add deterministic runtime tests for successful assistant output, tool call/result continuation, retryable model failure, cancellation, usage observation, and missing adapter fail-closed behavior. [covers=r[runtime-engine-host.verification.deterministic-matrix]]
- [x] [serial] V2: Add runtime-vs-agent fake-provider parity evidence for session id, prompt acceptance, model request metadata, event ordering, and terminal completion. [covers=r[runtime-engine-host.adapter-parity.agent-runtime]]
- [x] [serial] V3: Run `scripts/check-embedded-agent-sdk.rs`, `cargo test -p clankers-runtime --lib`, focused agent turn tests, Cairn validate/gates, and `git diff --check` before archive. [covers=r[runtime-engine-host.verification.closeout]]
