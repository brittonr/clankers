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

## Open Questions

- Should `clankers-engine` start with a fully serializable state surface, or is that a follow-on once the turn state machine is moved?
- Which current message types can be reused directly from `clankers-message`, and which need engine-native wrappers to avoid leaking app assumptions?
- How much of retry and cancellation policy should be pure engine state versus runtime-managed host policy in the first extraction?
