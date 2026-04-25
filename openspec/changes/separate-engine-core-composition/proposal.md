## Why

`clankers-core` and `clankers-engine` now both exist, but their ownership boundary is still easy to blur. The engine state currently carries a `CoreState` pass-through slot even though turn execution mostly copies it through rather than actively composing core reducer transitions. That creates ambiguity: future work could accidentally make the turn engine absorb prompt lifecycle, thinking, loop, or disabled-tools policy that belongs in the no-std core, or make core concerns leak into the engine without a clear adapter seam.

This change defines and enforces the composition boundary between pure core reducers, the engine turn reducer, and shell adapters.

## What Changes

- Document which reducer owns prompt lifecycle, loop/auto-test follow-ups, thinking, disabled tools, model/tool turn orchestration, retry, cancellation, and terminalization.
- Remove dormant pass-through `CoreState` from `EngineState` unless implementation introduces an explicit tested composition path that actively uses it.
- Add adapter-level composition helpers that sequence `clankers-core` and `clankers-engine` inputs/effects without either reducer importing shell runtime concerns.
- Extend FCIS/source rails so future work cannot silently move core-owned policy into the engine or engine-owned turn policy back into controller/agent shells.

## Capabilities

### Modified Capabilities

- `embeddable-agent-engine`: clarifies reducer ownership and composition so embedders can combine core lifecycle policy and engine turn policy predictably.
- `no-std-functional-core`: preserves the no-std core as the owner for deterministic prompt lifecycle and shell-independent control policy.

## Impact

- **Crates**: `clankers-core`, `clankers-engine`, `clankers-controller`, `clankers-agent`, boundary tests/scripts.
- **APIs**: engine state shape may remove unused `core_state`; adapter helpers may expose clearer composition functions.
- **Testing**: deterministic reducer tests plus boundary rails prove no dormant pass-through state and no ownership leakage.
