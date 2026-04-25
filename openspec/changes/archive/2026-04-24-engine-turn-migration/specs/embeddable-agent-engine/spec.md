## ADDED Requirements

### Requirement: The first executable engine slice MUST keep authoritative turn state across model and tool boundaries
The system MUST make `clankers-engine` the authoritative state machine for the first executable prompt → model → tool → continuation slice rather than using it only for one-off helper calls.

#### Scenario: prompt submission creates engine-owned pending model work
- **WHEN** a host submits a user prompt into the migrated engine slice
- **THEN** the engine records engine-owned turn state and a correlated pending model request in `EngineState`
- **THEN** the returned effects include the model request the host must execute rather than requiring the runtime shell to reconstruct request state locally

#### Scenario: model completion schedules engine-owned tool work
- **WHEN** the host returns model output that requests one or more tools
- **THEN** the engine updates its authoritative phase and pending tool-call state from that feedback
- **THEN** the returned effects enumerate the tool calls the host must execute without agent-local continuation branching deciding that tool plan independently

#### Scenario: tool feedback drives continuation or terminal finish through engine inputs
- **WHEN** the host reports tool success or tool failure for the migrated slice
- **THEN** the engine updates canonical turn state from that correlated tool feedback and decides whether another model request is needed or the turn should finish
- **THEN** shells do not re-derive that continuation or finish decision outside the engine boundary

### Requirement: The migrated engine slice MUST own cancellation and correlation validation
The system MUST route cancellation and feedback validation for the migrated slice through engine-owned state, correlation IDs, and explicit rejections.

#### Scenario: mismatched feedback is rejected without state mutation
- **WHEN** the host reports model or tool feedback whose correlation ID does not match pending engine-owned work for the current state
- **THEN** the engine returns an explicit rejection
- **THEN** the previously valid state remains unchanged

#### Scenario: wrong-phase feedback is rejected without state mutation
- **WHEN** the host reports model completion, tool feedback, or cancellation in a phase where that input is not valid
- **THEN** the engine returns an explicit rejection describing the phase mismatch
- **THEN** the previously valid state remains unchanged

#### Scenario: cancellation clears pending work through engine-owned terminalization
- **WHEN** the host cancels a turn while model or tool work is pending in the migrated slice
- **THEN** the engine clears the pending engine-owned work for that slice and emits the terminal cancellation outcome/events for the host to surface
- **THEN** shells do not synthesize cancellation completion or terminal state outside the engine boundary

### Requirement: Agent and controller shells MUST adapt the migrated slice through engine-native types
The system MUST carry the migrated turn slice through `clankers-engine` state, inputs, effects, and correlated feedback instead of shell-local request-state tuples or duplicated continuation logic.

#### Scenario: agent runtime executes only engine-requested model and tool work
- **WHEN** `clankers-agent` runs the migrated turn slice
- **THEN** it executes provider and tool I/O only in response to `EngineEffect` values produced by the engine
- **THEN** it feeds the resulting success or failure payloads back through the matching `EngineInput` values rather than deciding next-step policy locally

#### Scenario: controller-facing seams stay adapter-only for the migrated slice
- **WHEN** controller-owned seams participate in the migrated slice
- **THEN** they translate shell-native state and events to or from engine-native values
- **THEN** they do not remain the authoritative owner of the migrated prompt/model/tool continuation policy

### Requirement: Verification rails MUST cover the executable engine slice
The system MUST verify the first executable engine slice with deterministic engine tests and adapter-parity rails.

#### Scenario: engine tests cover positive and negative first-slice paths
- **WHEN** validation runs for this migrated slice
- **THEN** engine-focused tests cover prompt submission, model completion with tool planning, tool-result continuation, tool failure, cancellation, and terminal finish outcomes
- **THEN** the same test suite covers negative paths for mismatched correlation IDs and wrong-phase feedback rejection

#### Scenario: adapter rails reject reintroduced shell-owned continuation logic
- **WHEN** validation runs for this migrated slice
- **THEN** parity or FCIS-style rails prove `clankers-agent::turn` and nearby adapters interpret engine effects and correlated inputs for the migrated slice
- **THEN** failure blocks acceptance if runtime shells reintroduce authoritative prompt/model/tool continuation policy outside `clankers-engine`
