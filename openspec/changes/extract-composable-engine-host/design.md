## Verification Summary

This change is ready for implementation only after the contract-surface cleanup has landed or equivalent dependency rails are already green. Implementation is done when Clankers runtime flows use the reusable host runner and parity tests prove behavior did not move backward into `clankers-agent`.

## Context

`clankers-engine` now owns reducer decisions, but `clankers-agent::turn::run_turn_loop` remains the only reusable async shell that actually executes a full turn. It mixes effect interpretation, provider streaming, stream accumulation, tool execution, hooks, DB/capability plumbing, usage tracking, cancellation, retry sleeping, model switching, and event emission.

Embedders need a smaller layer: they should compose an engine runner with their own model client, tool executor, event sink, storage, and policy adapters.

## Goals / Non-Goals

**Goals**

- Extract reusable async engine-effect interpretation into an engine-host layer.
- Define trait seams for model execution, tool execution, sleep, event emission, cancellation, and usage observation.
- Extract reusable tool execution/catalog behavior out of `clankers-agent`.
- Extract deterministic stream accumulation with positive and negative tests.
- Preserve existing Clankers `Agent` behavior through adapters.

**Non-Goals**

- Do not move provider backends into the engine host.
- Do not move plugin runtime supervision into the engine.
- Do not move system prompt assembly, config discovery, session DB ownership, daemon protocol, or TUI rendering.
- Do not remove the existing Clankers `Agent` public assembly yet.

## Decisions

### 1. Add host traits instead of hard-coding provider/tool crates

**Choice:** Define host traits for model execution, tool execution, sleeping, cancellation, event sink, and usage observation.

**Rationale:** Embedders can wire `clanker-router`, a custom model backend, plugin tools, or test fakes without depending on full Clankers agent runtime.

**Alternative:** Move `run_turn_loop` wholesale into `clankers-engine`. Rejected because it would pull async/runtime/tool/provider concerns into the reducer crate.

### 2. Keep Clankers as default assembly

**Choice:** `clankers-agent` remains the high-level product assembly over the reusable host runner.

**Rationale:** Existing users and daemon/interactive flows should not see an API or behavior break while the internals become composable.

**Implementation:** Replace internal turn-loop policy with host-runner adapter calls, then keep existing `Agent` methods and events as shell translation.

### 3. Tool host is separate from engine host

**Choice:** Extract tool catalog/execution/result accumulation into a dedicated tool-host surface or module rather than embedding it in the generic runner.

**Rationale:** Tool execution has its own dependencies and policy seams: capability checks, hooks, output truncation, plugin adapters, cancellation, and result details. Keeping it separate lets minimal embedders use a tiny executor while Clankers wires the full host.

### 4. Stream accumulation is deterministic core within host layer

**Choice:** Move content-block stream folding into a reusable pure-ish module that has no event-bus or UI dependencies.

**Rationale:** Stream folding is reusable model response logic. Event forwarding is shell behavior.

**Implementation:** The host runner can call the accumulator while adapters optionally mirror stream deltas to an event sink.

## Risks / Trade-offs

**Trait explosion** → Mitigate by grouping seams by runtime responsibility and keeping the first runner generic enough for current Clankers plus tests only.

**Behavior drift in event ordering** → Mitigate with focused runtime parity tests over existing `AgentEvent` sequences.

**Tool plugin deadlocks or cancellation regressions** → Mitigate by reusing existing tool tests and adding host-level cancellation/missing-tool/error negative cases.

**Too-large first step** → Mitigate by landing stream accumulator and host traits first, then tool-host extraction, then `clankers-agent` adapter migration.
