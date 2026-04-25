## Why

`clankers-core` and `clankers-engine` now both exist, but their ownership boundary is still easy to blur. The engine state currently carries a `CoreState` pass-through slot even though turn execution mostly copies it through rather than actively composing core reducer transitions. That creates ambiguity: future work could accidentally make the turn engine absorb prompt lifecycle, thinking, loop, or disabled-tools policy that belongs in the no-std core, or make core concerns leak into the engine without a clear adapter seam.

This change defines and enforces the composition boundary between pure core reducers, the engine turn reducer, and shell adapters.

## What Changes

- Document which reducer owns prompt lifecycle, loop/auto-test follow-ups, thinking, disabled tools, model/tool turn orchestration, retry, cancellation, and terminalization.
- Remove dormant pass-through `CoreState` from `EngineState`; any future active core/engine state composition requires a separate explicit no-std-core contract and verification rail.
- Add adapter-level composition helpers that sequence `clankers-core` and `clankers-engine` inputs/effects without either reducer importing shell runtime concerns.
- Narrow the `clankers-agent`/`clankers-core` boundary so `clankers-agent` runtime and public APIs stay shell-native for the migrated lifecycle slice; controller-owned adapters translate core inputs/effects and request shell work from the agent.
- Extend FCIS/source rails so future work cannot silently move core-owned policy into the engine or engine-owned turn policy back into controller/agent shells.

## Non-Goals

- Do not merge `clankers-core` and `clankers-engine`.
- Do not move provider I/O, tool I/O, daemon protocol handling, TUI rendering, or async runtime coordination into either reducer.
- Do not migrate engine-owned model/tool turn policy into `clankers-core` in this change; future downward movement requires a separate explicit no-std-core contract and verification rail.
- Do not change user-visible prompt lifecycle, loop, auto-test, thinking, disabled-tool, retry, cancellation, or terminalization behavior except for removing dormant pass-through state.

## Capabilities

### Modified Capabilities

- `embeddable-agent-engine`: clarifies reducer ownership and composition so embedders can combine core lifecycle policy and engine turn policy predictably.
- `no-std-functional-core`: preserves the no-std core as the owner for deterministic prompt lifecycle and shell-independent control policy.

## Impact

- **Crates**: `clankers-core`, `clankers-engine`, `clankers-controller`, `clankers-agent`, boundary tests/scripts.
- **API boundary**: `clankers-agent` must not newly expose or consume `clankers-core` runtime/public API types for this lifecycle slice; controller adapters remain the core-type boundary.
- **APIs**: engine state shape may remove unused `core_state`; adapter helpers may expose clearer composition functions.
- **Testing**: deterministic reducer tests plus boundary rails prove no dormant pass-through state and no ownership leakage.

## Verification

Design/tasks must wire the acceptance bundle to these entrypoints before implementation is marked complete:

- `cargo check -Zbuild-std=core,alloc --target thumbv7em-none-eabi -p clankers-core --no-default-features`
- `cargo test -p clankers-core pre_engine_cancellation`
- `cargo test -p clankers-engine --lib`
- `cargo test -p clankers-engine --lib engine_state_fields_are_active`
- `cargo test -p clankers-controller core_engine_composition`
- `cargo test -p clankers-controller pre_engine_cancellation`
- `cargo test -p clankers-controller accepted_engine_prompt`
- `cargo test -p clankers-agent engine_feedback`
- `cargo test -p clankers-controller --test fcis_shell_boundaries`
- `./scripts/check-llm-contract-boundary.sh`


Boundary rail mapping:

- `engine-excludes-core-dependency` is enforced by `./scripts/check-llm-contract-boundary.sh`.
- `cross-reducer-source-rail` and `agent-core-type-rail` are enforced by `cargo test -p clankers-controller --test fcis_shell_boundaries`.
- Core cancellation semantics are enforced by `cargo test -p clankers-core pre_engine_cancellation`.
- `composition-tests` is enforced by `cargo test -p clankers-controller core_engine_composition`, `cargo test -p clankers-controller accepted_engine_prompt`, `cargo test -p clankers-agent engine_feedback`, and `cargo test -p clankers-core pre_engine_cancellation`.
- Existing `cargo-tree-rail` and `source-surface-rail` remain enforced by `./scripts/check-llm-contract-boundary.sh` and `cargo test -p clankers-controller --test fcis_shell_boundaries` respectively.
- `no.std.functional.core.pre.engine.cancellation.shell-parity` is enforced by `cargo test -p clankers-controller pre_engine_cancellation` and `cargo test -p clankers-controller --test fcis_shell_boundaries` (including no pre-engine `EngineInput::CancelTurn` construction outside agent turn adapters).
- `embeddable-agent-engine.no-dormant-core-state` / `engine-state-active-data` is enforced by `cargo test -p clankers-controller --test fcis_shell_boundaries`, `cargo test -p clankers-engine --lib engine_state_fields_are_active`, and `./scripts/check-llm-contract-boundary.sh` by rejecting `core_state`, `CoreState`, `CoreEffectId`, and `clankers_core` in non-test `clankers-engine` source/dependencies and by requiring an explicit reducer test for every remaining `EngineState` field.
- `adapter-held-prompt-correlation` is enforced by `cargo test -p clankers-controller core_engine_composition`, `cargo test -p clankers-controller accepted_engine_prompt`, and `cargo test -p clankers-agent engine_feedback`; these tests must prove retained `CoreEffectId`, accepted prompt kind, stale/mismatched rejection, and no `CoreEffectId` storage in `EngineState`.
