## Why

`clankers-engine` now exists, but it is still a draft helper surface rather than the authoritative host-facing turn state machine promised by the archived `embeddable-agent-engine` work. The current engine types only cover initial model-request planning and model-completion decoding, while `clankers-agent::turn` still owns tool-result ingestion, cancellation, retry/stop decisions, and transient request-state glue that an embedder cannot reuse directly.

## What Changes

- Make the initial `clankers-engine` turn slice real and authoritative instead of helper-only.
- Extend the engine-owned state/input/effect contract to cover tool-result feedback, tool-failure feedback, cancellation, continuation, and terminal outcomes for the first executable migration slice.
- Move the corresponding prompt → model → tool → continuation control flow out of `clankers-agent::turn` local glue and into engine-owned reducers/helpers.
- Rework controller/agent adapters to carry engine-native state and correlation IDs across the migrated slice rather than reconstructing ad hoc request-state tuples locally.
- Add deterministic positive/negative rails plus adapter-parity checks for the migrated engine slice.

## Capabilities

### New Capabilities
- None.

### Modified Capabilities
- `embeddable-agent-engine`: define the first executable engine-owned turn slice and require controller/agent shells to adapt that slice through engine-native state, inputs, effects, and correlated feedback.

## Impact

- Affected code: `crates/clankers-engine`, `crates/clankers-agent/src/turn/`, `crates/clankers-controller`, and FCIS/parity rails in tests.
- APIs: expands the concrete `clankers-engine` host-facing contract from draft request-planning helpers to an executable first-slice state machine.
- Architecture: removes more reusable turn policy from async agent runtime code and makes the engine boundary real for the initial prompt/model/tool round trip.
- Testing: requires new deterministic engine tests and adapter-parity rails for tool feedback, cancellation, and terminal turn outcomes.
