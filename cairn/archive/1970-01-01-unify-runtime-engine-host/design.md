# Design: Unify Runtime with Engine Host

## Summary

The runtime facade should become a thin host-facing shell around `clankers-engine` and `clankers-engine-host`, not a second prompt lifecycle implementation. `RuntimeBuilder` remains the construction API, but its session execution services should be engine-host adapters.

## Decisions

### Decision: runtime sessions submit `EnginePromptSubmission`

`SessionHandle::submit_prompt` should assemble host prompt input, load host-owned replay context, build neutral `EngineMessage` history, and call `run_engine_turn`. Direct `ModelAdapter::complete -> Vec<SessionEvent>` remains only as a compatibility adapter implemented on top of `ModelHost`, not as the primary session execution path.

### Decision: host-facing events are projections

`SessionEvent` should be emitted by a projection from engine events, model deltas, tool starts/results, usage observations, and terminal report state. The projection must keep metadata safe and must not expose daemon/TUI/provider-native DTOs.

### Decision: defaults stay safe and deterministic

The default runtime should continue to use in-memory/noop services and fake deterministic adapters. It must not discover credentials, dotdirs, providers, plugins, or network services unless explicit desktop adapters are injected.

## Verification Plan

- Add a runtime fixture that observes prompt acceptance, model request, assistant output, usage, and completion through `run_engine_turn`.
- Add negative fixtures for missing/denied model and tool adapters.
- Add parity evidence that batch/runtime and agent fake-provider paths preserve prompt id/session id, model request metadata, event order, and terminal completion.
