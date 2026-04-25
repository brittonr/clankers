## Verification Summary

This change is complete when reducer ownership is visible in code and tests: no dormant `CoreState` pass-through in `EngineState`, explicit adapter composition helpers, and source rails that fail if lifecycle policy moves into the engine or turn policy moves back into shells.

## Context

The no-std core reducer and the embeddable engine reducer solve different problems. `clankers-core` owns deterministic shell-independent lifecycle and control policy. `clankers-engine` owns model/tool turn progression. The current engine type still contains `core_state: Option<CoreState>`, but the turn reducer copies it rather than reducing it. That is a coupling smell and a future footgun.

## Goals / Non-Goals

**Goals**

- Define reducer ownership in specs and tests.
- Remove unused `CoreState` pass-through from engine state or replace it with an explicit tested composition contract.
- Add adapter helpers that sequence core and engine effects without provider/TUI/daemon dependencies.
- Extend FCIS rails for cross-reducer ownership.

**Non-Goals**

- Do not merge `clankers-core` and `clankers-engine`.
- Do not move async provider/tool execution into either reducer.
- Do not change existing prompt lifecycle, loop, auto-test, thinking, disabled-tools, retry, or tool-continuation behavior except to remove unused pass-through state.

## Decisions

### 1. Reducers stay separate

**Choice:** Keep `clankers-core` and `clankers-engine` as separate reducers with different ownership domains.

**Rationale:** The core remains no-std and shell-independent. The engine can use richer message/turn contracts while still avoiding provider/runtime dependencies.

**Alternative:** Move turn policy down into `clankers-core`. Rejected for now because model/tool content contracts and turn phases are a larger surface than the current no-std lifecycle reducer should absorb.

### 2. No dormant composition fields

**Choice:** Remove `core_state` from `EngineState` unless a concrete reducer input/effect uses it.

**Rationale:** State fields that are copied but not used make ownership ambiguous and encourage hidden coupling.

**Implementation:** Delete the field and update constructors/tests, or replace it only with a documented active composition structure if implementation proves a real need.

### 3. Composition belongs in adapters

**Choice:** Controller/agent host adapters sequence core reducer effects and engine reducer effects through explicit helper functions.

**Rationale:** Composition is shell orchestration. Each reducer stays deterministic and testable on its own, while adapters can combine their outcomes for product behavior.

### 4. Rails check ownership, not just imports

**Choice:** Extend FCIS/source rails to inventory reducer-owned concepts and shell-owned interpretation points.

**Rationale:** A simple import ban is not enough. The risk is duplicated policy branches in the wrong layer.

## Risks / Trade-offs

**False-positive rails** → Mitigate by checking specific constructor/path inventories and allowing named adapter conversion seams.

**Removing pass-through state breaks tests that clone full engine state** → Mitigate with targeted engine test updates and no behavior changes.

**Composition helper over-abstraction** → Mitigate by keeping helpers small: pure input/effect translation only, no I/O.
