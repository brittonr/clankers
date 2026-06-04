## Context

The archived `embeddable-agent-engine` change established the long-term boundary: `clankers-engine` should own reusable host-facing turn orchestration while controller, agent runtime, and UI/transport shells adapt that boundary. The repo now has a draft `crates/clankers-engine` crate plus partial adoption in `clankers-agent::turn`, but the current implementation is still transitional:

- `EngineState`, `EngineInput`, and `EngineEffect` exist, but only the initial model-request planner and model-completion decoder are authoritative.
- `EngineInput::ToolCompleted`, `ToolFailed`, and `CancelTurn` are declared without matching engine-owned reducers.
- `clankers-agent::turn` still reconstructs turn flow locally after model completion, owns tool-result ingestion, and recreates per-step request-state glue that embedders cannot reuse directly.
- The current runtime path treats engine state as a transient helper for one request rather than as the authoritative state machine for the migrated turn slice.

This follow-on change makes the first executable engine slice real: prompt submission, model completion, tool feedback, continuation, cancellation, and terminal outcomes for the initial prompt → model → tool → continuation loop.

## Goals / Non-Goals

**Goals:**
- Promote `clankers-engine` from helper-only request planning to the authoritative state machine for the first executable turn slice.
- Keep engine-owned state, correlation IDs, inputs, effects, and terminal outcomes alive across model and tool boundaries inside the migrated slice.
- Move tool-result ingestion, tool-failure handling, cancellation handling, and continuation decisions out of `clankers-agent::turn` local policy and into engine-owned reducers/helpers.
- Keep controller and agent runtime code as imperative shells that execute engine effects and translate shell results back into engine inputs.
- Add deterministic positive/negative rails and adapter-parity checks that make the migrated engine slice reviewable and hard to regress.

**Non-Goals:**
- Migrating prompt assembly, AGENTS.md loading, OpenSpec/skills discovery, or system-prompt building into `clankers-engine`.
- Replacing provider streaming assembly, actual tool execution, hooks, or transport/UI event emission with engine code.
- Migrating every remaining turn concern in one shot; broad retry/tuning policy outside the first executable slice can follow later if needed.
- Reworking session persistence or daemon framing around a fully serialized engine state in this change.

## Decisions

### 1. Model the migrated slice as explicit engine reducers over `EngineState`

**Choice:** extend `clankers-engine` with pure/plain-data reducer helpers for the declared host feedback inputs (`SubmitUserPrompt`, `ModelCompleted`, `ModelFailed`, `ToolCompleted`, `ToolFailed`, `CancelTurn`) and make `EngineState` carry the pending model/tool slots needed for those transitions.

**Rationale:** the current draft already proved request planning and model-completion decoding fit the engine boundary. The next step is to complete the executable slice so every state transition in the first prompt/model/tool round trip flows through engine-owned state and correlation data rather than agent-local branching.

**Alternative considered:** keep using engine helpers only for request building and model-completion parsing while `clankers-agent::turn` continues to own tool feedback and cancellation logic.

**Why not:** that leaves the host-facing engine contract half-real, keeps the reusable continuation policy in async runtime code, and makes the public engine types misleading because several declared inputs would still have no authoritative behavior.

### 2. Keep provider I/O and tool I/O in `clankers-agent`, but drive them only from engine effects

**Choice:** `clankers-agent::turn` remains the imperative shell that streams provider output, executes tools, runs hooks, and emits `AgentEvent`s, but it may only do so in response to `EngineEffect`s and must feed resulting success/failure payloads back through engine inputs.

**Rationale:** this preserves the functional-core / imperative-shell split. The engine stays deterministic and host-facing; the agent runtime remains the boring I/O layer.

**Alternative considered:** move provider/tool execution traits or async callbacks into `clankers-engine`.

**Why not:** that would reintroduce runtime-heavy abstractions into the engine boundary, make the crate harder to embed in non-Clankers hosts, and weaken deterministic testing.

### 3. Make engine state authoritative across the whole migrated round trip

**Choice:** replace the current per-request `request_state`/`request_id` handoff with one engine-owned turn state that survives prompt submission, model completion, tool execution feedback, continuation scheduling, and terminal finish for the migrated slice.

**Rationale:** a real engine boundary must own more than one request builder call. Carrying one authoritative `EngineState` through the round trip makes the API usable to other hosts and prevents the runtime from reconstructing implicit state out of local tuples.

**Alternative considered:** keep the transient state handoff and add only more helper functions.

**Why not:** that still leaves the runtime as the true owner of phase/correlation/continuation policy and blocks future controller-host reuse.

### 4. Scope the first slice to one executable prompt/model/tool/continuation loop, not the entire engine roadmap

**Choice:** the initial migration covers prompt submission acceptance, model completion handling, tool-call planning, tool-result or tool-failure feedback, cancellation, continuation back into at most one follow-up model request at a time, and terminal outcomes for that loop.

**Rationale:** this is the highest-ROI slice that turns the engine from a spec artifact into a real reusable harness boundary without trying to absorb every remaining policy at once.

**Alternative considered:** jump straight to full retry/token-budget/session persistence ownership or introduce a richer pending-work queue in the first slice.

**Why not:** that broadens the change before the core prompt/model/tool round trip is stable and reviewable.

### 5. Keep engine-owned messages canonical and narrow for the first slice

**Choice:** `EngineState.messages` will track only the canonical conversation ordering needed for the migrated slice: submitted user prompts, assistant/model outputs, tool-use requests, and tool results. Display-only metadata, render caches, timestamps used only by shells, and transport/UI presentation state remain outside the engine.

**Rationale:** the engine needs enough message state to own continuation decisions, but pulling render-specific or session-persistence concerns into the first slice would over-scope the change.

**Alternative considered:** either leave all message mutation shell-local for now or move the full shell-visible message model into the engine immediately.

**Why not:** the first option keeps continuation policy split-brain, while the second absorbs too much app-specific state too early.

### 6. Limit controller changes to adapter-only wrappers and parity seams

**Choice:** this change may touch controller-owned helper seams only where they translate shell-native values to or from the migrated engine slice, but it does not introduce a new controller public API or move broader session orchestration into the controller.

**Rationale:** the highest-ROI work is in `clankers-engine` and `clankers-agent::turn`. Controller scope should stay narrow unless a touched adapter seam must change to keep the engine boundary authoritative.

**Alternative considered:** redesign controller session APIs as part of the same migration.

**Why not:** that couples the first executable engine slice to a larger controller refactor and increases risk without unlocking the core engine state-machine migration.

### 7. Pin the boundary with engine-focused and adapter-focused rails

**Choice:** add deterministic engine tests for positive and negative paths plus parity/boundary rails that assert `clankers-agent::turn` interprets engine effects instead of re-deriving the migrated policy locally.

**Rationale:** the repo’s FCIS work only held because the boundaries became executable rails. The engine migration needs the same treatment.

**Alternative considered:** rely on unit tests inside `clankers-agent::turn` alone.

**Why not:** local runtime tests can still pass while the engine surface drifts or the runtime quietly regains authority over continuation logic.

## Risks / Trade-offs

- **[State-shape churn]** → expanding `EngineState` now may require follow-on reshaping as more slices migrate. **Mitigation:** keep the first-slice fields explicit and narrowly tied to prompt/model/tool/cancel flow.
- **[Temporary dual paths]** → some runtime glue may coexist with new engine reducers during migration. **Mitigation:** land the reducers first, then immediately switch adapters and add parity rails that fail if local branching remains authoritative.
- **[Message evolution ambiguity]** → moving tool-result ingestion into the engine can blur what belongs in engine messages versus shell-only display state. **Mitigation:** keep engine message evolution limited to canonical conversation ordering and leave display/rendering concerns outside.
- **[Over-scoping]** → retry/session-persistence desires can balloon the change. **Mitigation:** explicitly stop at one executable prompt/model/tool/continuation loop with at most one pending follow-up model request and defer broader policy once this slice is stable.

## Migration Plan

1. Extend `clankers-engine` state/input/effect/outcome types so every declared first-slice host feedback path has a reducer-backed transition.
2. Move the prompt/model/tool/cancel continuation decisions from `clankers-agent::turn` local branching into engine reducers while keeping provider and tool execution in the runtime shell.
3. Update runtime adapters to carry one authoritative `EngineState` and correlated IDs across the migrated round trip and to interpret engine effects without re-deriving migrated policy.
4. Add deterministic positive/negative engine tests plus adapter-parity and FCIS boundary rails.
5. Keep broader retry/session persistence follow-ons out of scope until the first executable engine slice is stable.

## Open Questions

- Should the first-slice engine expose a small helper for hosts that want to inspect the next pending model/tool correlation IDs without pattern-matching the full state structure?
