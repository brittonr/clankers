## 1. Engine slice expansion

- [ ] 1.1 Extend `crates/clankers-engine` state/input/effect/outcome types so the first executable slice covers tool feedback, cancellation, continuation, and terminal outcomes in addition to initial model-request planning.
- [ ] 1.2 Implement engine-owned reducers for correlated tool success, tool failure, cancellation, and next-step continuation decisions while keeping invalid correlation and wrong-phase rejection explicit.
- [ ] 1.3 Keep canonical first-slice message/phase/correlation updates inside engine-owned state, limited to canonical conversation ordering rather than shell display state.

## 2. Adapter migration

- [ ] 2.1 Update `crates/clankers-agent/src/turn/` to carry one authoritative `EngineState` across the migrated prompt → model → tool → continuation loop and to execute provider/tool work only from `EngineEffect` values.
- [ ] 2.2 Feed provider/tool success and failure back into the engine through matching `EngineInput` values instead of re-deriving continuation or terminal decisions in runtime code.
- [ ] 2.3 Adjust any touched controller-facing seams or helpers so they translate shell-native data to or from engine-native values without becoming the authoritative owner of the migrated slice or adding a broader controller API redesign.

## 3. Verification rails

- [ ] 3.1 Add deterministic positive and negative engine tests for prompt submission, tool planning, tool-result continuation, tool failure, cancellation, terminal finish, mismatched correlation IDs, and wrong-phase rejection.
- [ ] 3.2 Extend parity/FCIS boundary rails so the migrated slice fails if `clankers-agent::turn` or nearby adapters reintroduce shell-owned prompt/model/tool continuation logic outside `clankers-engine`.
- [ ] 3.3 Run the focused engine and adapter validation commands for the migrated slice and capture the results in the change evidence or implementation notes.
