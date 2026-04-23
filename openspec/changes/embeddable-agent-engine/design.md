## Context

Clankers now has a real `clankers-core` crate with a `#![no_std]` reducer, explicit effect correlation, bare-metal compile rails, and FCIS boundary checks. That work proved the repo can enforce a core/shell split, but the currently extracted slice is still narrow and shaped around `SessionController` rather than around an embeddable agent engine.

Today the reusable-looking semantics are split across three places:
- `clankers-core` owns one deterministic prompt-lifecycle slice.
- `clankers-controller` owns controller-shaped translation and effect execution around `CoreState`.
- `clankers-agent` still owns the end-to-end async turn loop (`prompt -> model -> tool calls -> repeat`) plus prompt assembly, hook/db wiring, and provider/tool runtime integration.

That means the current architecture is good at proving FCIS boundaries but not yet good at exposing one small host-facing crate that another project can embed. If another project wants “Clankers as a harness”, it still has to adopt too much controller, protocol, and runtime baggage.

## Goals / Non-Goals

**Goals:**
- Define a new `clankers-engine` crate as the canonical embeddable harness boundary.
- Make that boundary host-first: explicit engine state, inputs, effects, outcomes, and semantic events.
- Move the highest-value reusable orchestration policy next: end-to-end turn execution, including model/tool round trips and continuation/stop decisions.
- Reframe controller and agent crates as adapters over the engine rather than long-term homes for reusable harness semantics.
- Keep the extraction path compatible with continued downward migration of deterministic policy into `clankers-core`.

**Non-Goals:**
- Moving prompt assembly, AGENTS.md loading, skills loading, or OpenSpec context discovery into the engine.
- Replacing the daemon protocol or TUI protocol surface with the engine API.
- Finishing all extraction work in one change; this change defines the architecture and migration contract.
- Forcing the full engine crate to be `no_std` immediately. The hard requirement is that pure deterministic logic remains movable into `clankers-core`.

## Decisions

### 1. Add `clankers-engine` as a new layer between `clankers-core` and app shells

**Choice:** Introduce a new crate, `clankers-engine`, whose job is to expose a compact host-facing harness API.

**Rationale:** `clankers-core` is intentionally low-level and pure. `clankers-controller` and `clankers-agent` are too tied to daemon, TUI, async runtime, provider, and app-shell concerns. A separate engine crate gives embedding a stable target without polluting the pure core or forcing embedders through Clankers protocol types.

**Alternative considered:** Reuse `SessionController` as the embedding surface.

**Why not:** `SessionController` is already shaped around `SessionCommand`, `DaemonEvent`, embedded-mode special cases, and controller-owned execution helpers. That would preserve the current controller-centric architecture instead of giving the repo a true host-facing engine boundary.

### 2. Define the engine boundary as explicit host-driven contracts

**Choice:** The engine API will revolve around explicit data contracts: `EngineState`, `EngineInput`, `EngineEffect`, `EngineOutcome`, and `EngineEvent`.

**Rationale:** This preserves FCIS discipline and makes the engine usable in many hosts. A host should be able to submit user input, execute requested model/tool effects, and feed correlated results back without importing daemon or UI concepts.

**Alternative considered:** Hide the engine behind one async trait object that directly performs model and tool calls.

**Why not:** That would collapse effect planning and effect execution back into one shell-heavy abstraction. It weakens determinism, makes testing harder, and obscures the eventual path for moving pure policy into `clankers-core`.

### 3. Extract turn orchestration before lower-value app refactors

**Choice:** The next major reusable slice will be the turn state machine currently centered in `crates/clankers-agent/src/turn/mod.rs`.

**Rationale:** Embeddability depends on owning the prompt → model → tool → continuation flow. Extracting more narrow controller-only slices first would improve FCIS cleanliness without creating a useful harness. The highest leverage move is to make model request planning, tool request planning, tool-result ingestion, retry, cancellation, and stop decisions engine-owned.

**Alternative considered:** Continue extracting small controller lifecycle slices into `clankers-core` before introducing the engine.

**Why not:** That risks optimizing for purity of local seams rather than for a host-facing reusable product boundary. It would leave the repo with a cleaner controller but still no obvious embedding API.

### 4. Keep app-specific prompt assembly and transport outside the engine

**Choice:** Prompt assembly (`system_prompt.rs`), daemon/client framing, attach-mode behavior, TUI rendering, and project-context discovery stay outside `clankers-engine`.

**Rationale:** Those are valuable Clankers application behaviors, but they are not required for a minimal embedded harness. Keeping them outside the engine avoids locking embedders into Clankers app conventions.

**Alternative considered:** Make `clankers-engine` assemble prompts and emit `DaemonEvent`-like outputs directly.

**Why not:** That would leak Clankers app policy into the embedding surface and make the engine harder to adopt in other projects.

### 5. Add architecture rails that enforce the embedding target

**Choice:** Add explicit verification rails for the new engine boundary: public-surface leakage checks, turn-state-machine tests, and parity tests proving controller/agent shells only adapt engine requests/results.

**Rationale:** The repo already succeeded once by making FCIS boundaries executable through tests and scripts. The embeddable-engine goal needs the same kind of rails or the architecture will drift back toward controller-local logic.

**Alternative considered:** Rely on code review and the existing `no-std-functional-core` rails.

**Why not:** Existing rails protect the pure core slice but do not constrain the shape of the host-facing embedding boundary.

## Risks / Trade-offs

- **[Two-layer complexity]** → Adding `clankers-engine` introduces another architectural layer. Mitigation: give it a narrow purpose and move only embedding-relevant policy into it.
- **[Premature API freeze]** → A public engine API can ossify too early. Mitigation: specify the contracts in OpenSpec first, keep the initial public surface small, and allow internal adapter reshaping during early implementation.
- **[Split-brain orchestration during migration]** → Controller/agent and engine may temporarily duplicate logic. Mitigation: add parity rails and explicitly track which orchestration paths are authoritative at each migration step.
- **[Core/engine boundary confusion]** → Teams may not know whether logic belongs in `clankers-core` or `clankers-engine`. Mitigation: document the rule: pure deterministic logic trends downward into core; host-facing reusable harness semantics land in engine first; app-specific shells remain above engine.
- **[Async/runtime impedance mismatch]** → Current turn logic is deeply async/provider/tool shaped. Mitigation: extract explicit model/tool effect contracts first, then move the runtime I/O behind host adapters.

## Migration Plan

1. Create `clankers-engine` with draft engine-owned state/input/effect/outcome/event types and engine boundary rails.
2. Adapt current controller/agent code to prove the engine types can represent one end-to-end prompt/model/tool/continue flow.
3. Move reusable turn-orchestration policy out of `clankers-agent::turn` into the engine while preserving current shell-visible behavior through parity tests.
4. Rebase `SessionController` and embedded/interactive shells onto engine adapters.
5. Continue migrating deterministic sub-slices downward into `clankers-core` once they are expressed as explicit engine state/input/effect transformations.

## Engine Contract

### Public crate and layering

The new workspace crate will be named `clankers-engine` and will sit between `clankers-core` and all Clankers-specific shells.

```text
clankers-core        -> pure deterministic reducers/effects that can be no_std
clankers-engine      -> host-facing reusable agent-harness policy and contracts
clankers-controller  -> session/daemon adapter over engine
clankers-agent       -> async runtime adapter over engine
src/modes/*          -> app shells, transport, TUI, attach, interactive mode
```

`clankers-engine` is the first landing zone for reusable harness semantics that still need richer host-facing contracts than the current `clankers-core` slice. Pure deterministic sub-slices discovered inside the engine remain candidates for later downward migration into `clankers-core`.

### Host-facing plain-data types

The public engine surface is defined in engine-native plain-data terms rather than daemon, TUI, or interactive-mode protocol types.

- `EngineState`
  - canonical conversation state for the reusable engine-owned slice
  - pending model request slot keyed by an engine-owned correlation ID
  - pending tool call slots keyed by engine-owned correlation IDs
  - turn phase for the current reusable state machine
  - retry/cancellation/token-limit bookkeeping required for deterministic next-step decisions
  - reusable message-evolution state and continuation ordering state
- `EngineInput`
  - `SubmitUserPrompt { prompt, attachments, metadata }`
  - `ModelCompleted { request_id, response }`
  - `ModelFailed { request_id, failure }`
  - `ToolCompleted { call_id, result }`
  - `ToolFailed { call_id, failure }`
  - `CancelTurn { reason }`
  - any explicit host feedback required to advance the reusable turn slice
- `EngineEffect`
  - `RequestModel { request_id, request }`
  - `ExecuteTool { call_id, tool_name, input }`
  - `EmitEvent { event }`
  - `FinishTurn { outcome }`
- `EngineOutcome`
  - next `EngineState`
  - ordered `Vec<EngineEffect>` to execute outside the engine
  - typed acceptance or rejection for the supplied input
- `EngineEvent`
  - semantic engine-native notices such as busy-state changes, tool/model lifecycle markers, retry notices, cancellation notices, and terminal turn status

All correlation IDs are engine-owned plain data. Hosts echo those IDs back when reporting model or tool completion. The engine does not mint transport-native IDs and does not depend on `DaemonEvent`, `SessionCommand`, Tokio tasks, channel handles, TUI widgets, or terminal runtime state.

### Execution contract

The engine is host-driven.

1. The host submits `EngineInput::SubmitUserPrompt`.
2. The engine returns an `EngineOutcome` whose ordered effects may include a `RequestModel` effect and semantic `EmitEvent` effects.
3. The host executes the model request outside the engine and feeds the result back through either `ModelCompleted` or `ModelFailed` with the same `request_id`.
4. If the engine decides tool work is needed, it emits one or more `ExecuteTool` effects with correlated `call_id` values.
5. The host executes those tool calls outside the engine and feeds results back through `ToolCompleted` or `ToolFailed`.
6. The engine alone decides whether the turn stops, retries, requests another model continuation, emits terminal failure, or finishes successfully.

This keeps execution policy deterministic and testable while leaving actual provider I/O, tool I/O, transport, hooks, and UI work in host shells.

## Canonical crate map and ownership split

| Layer | Owns | Must not own |
|---|---|---|
| `clankers-core` | pure deterministic reducers, correlation rules, plain-data effect planning that can be `no_std` | provider I/O, tool I/O, daemon/TUI/runtime types |
| `clankers-engine` | reusable host-facing turn orchestration, message evolution, continuation policy, retry/cancellation/token-limit policy, engine-native semantic events | prompt discovery, AGENTS.md loading, OpenSpec/skills scanning, daemon protocol, TUI widgets, Tokio/runtime handles |
| `clankers-controller` | session/daemon translation, engine-state persistence wiring, engine effect interpretation requests, protocol/event translation, embedded-shell adapter APIs | authoritative reusable turn policy for prompt/model/tool continuation |
| `clankers-agent` | async runtime execution for provider calls, tool dispatch, hook plumbing, streaming assembly around engine-directed work | authoritative reusable turn-state-machine policy |
| `clankers-provider` | provider-specific request/stream/auth execution | reusable engine turn policy or conversation ordering |
| `clankers-message` | reusable message/content structs that remain shell-agnostic | engine control flow, prompt assembly, daemon/TUI policy |
| app shells (`src/modes/*`, TUI, daemon, attach) | transport, terminal rendering, local slash UX, prompt assembly inputs, user-visible shell behavior | reusable engine semantics or direct engine-internal state transitions |

## Next reusable slice to migrate

The next extraction target is the end-to-end turn orchestration now centered in `crates/clankers-agent/src/turn/mod.rs` and its nearby helpers.

The engine-owned reusable slice must include:

- prompt submission acceptance/rejection and turn-start state transitions
- model request planning after prompt submission
- model completion ingestion and stop-reason interpretation
- tool-call planning from model output
- tool-result ingestion and continuation decisions
- retry policy for retryable model/tool failures
- cancellation handling and cancellation terminalization
- token-limit handling and the decision to stop, retry, or request continuation
- terminal stop behavior and typed turn outcome emission

The engine-owned message evolution rules must include:

- canonical user-input message insertion
- canonical assistant/model output insertion
- canonical tool-result message insertion
- continuation ordering between assistant output, tool requests, tool results, and follow-up model requests
- preserving message evolution independent of how a host assembled system prompts

Prompt assembly remains outside the engine. AGENTS.md, SYSTEM.md, APPEND_SYSTEM.md, OpenSpec context, skills, and project-context discovery stay in app-shell or prompt-assembly crates that hand the engine already-prepared prompt inputs.

## Adapter responsibilities after the split

### `clankers-controller`

`clankers-controller` becomes the adapter that:

- translates session commands and embedded shell callbacks into `EngineInput`
- persists and restores session-owned engine state
- interprets engine semantic events into `DaemonEvent` and shell-native outputs
- tracks controller-specific session metadata not owned by the reusable engine
- exposes shell-native helper APIs so embedded runtime files do not construct raw engine internals directly

It stops being the authoritative place for reusable prompt/model/tool continuation policy.

### `clankers-agent`

`clankers-agent` becomes the runtime host that:

- executes provider/model work requested by `EngineEffect::RequestModel`
- executes tool work requested by `EngineEffect::ExecuteTool`
- streams provider output and converts it into engine feedback payloads
- runs hooks and shell runtime concerns around engine-directed work
- reports correlated model/tool success or failure back into the engine

It stops being the authoritative place for reusable turn-state-machine policy currently living in async runtime code.

## Verification rails

The embeddable-engine architecture needs four explicit rails.

1. **Engine public-surface rail**
   - inventory exported `clankers-engine` API items
   - reject `DaemonEvent`, `SessionCommand`, TUI widget/runtime types, and other app-shell types in public signatures
2. **Adapter parity rail**
   - prove `clankers-controller` and `clankers-agent` execute engine-requested model/tool work and only translate engine semantic events/results back to shell forms
   - reject local re-derivation of reusable continuation policy in adapters for the migrated slice
3. **Engine turn-state-machine rail**
   - positive coverage for prompt -> model -> tool -> continuation -> stop
   - negative coverage for wrong correlation IDs, duplicate completion, invalid cancellation timing, retry exhaustion, token-limit stop, and tool/model failure ordering
4. **Architecture oracle review**
   - human-reviewed checkpoint confirming prompt assembly, transport, and UI concerns stay outside the engine boundary while reusable harness semantics route through the engine first

## Open Questions

- Should `clankers-engine` start with a fully serializable state surface, or is that a follow-on once the turn state machine is moved?
- Which current message types can be reused directly from `clankers-message`, and which need engine-native wrappers to avoid leaking app assumptions?
- How much of retry and cancellation policy should be pure engine state versus runtime-managed host policy in the first extraction?
