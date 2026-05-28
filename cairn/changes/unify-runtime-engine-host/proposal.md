# Proposal: Unify Runtime with Engine Host

## Problem

`clankers-runtime` is marketed as the host-facing facade, but `SessionHandle::submit_prompt` currently calls a simple `ModelAdapter::complete` and forwards `SessionEvent` values directly. It does not dogfood `clankers-engine-host::run_engine_turn`, real model/tool continuation policy, retry handling, cancellation, usage observation, or tool execution. That leaves the embedded runtime as a parallel toy facade rather than the reusable Clankers session brick.

## Proposed Change

Make the runtime session path execute prompts through the same engine-host runner used by the agent turn path. The runtime should accept host-owned model/tool/retry/event/cancellation/usage adapters, convert prompt/session state into `EnginePromptSubmission`, run `run_engine_turn`, and project engine-host outcomes into host-facing `SessionEvent` values.

## Impact

- **Files**: `crates/clankers-runtime/src/{runtime,session,prompt,events}.rs`, `crates/clankers-engine-host`, `crates/clankers-adapters`, `src/modes/batch.rs`, embedded examples.
- **Testing**: fake-adapter runtime turn tests, runtime-vs-agent parity fixture, cancellation/retry/tool/usage matrix cases, embedded SDK acceptance rail.
