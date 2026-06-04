## MODIFIED Requirements

### Requirement: The engine API MUST expose explicit host-driven execution contracts

The engine MUST define explicit host-facing contracts for model execution requests, tool execution requests, host feedback, and semantic engine events after an adapter has accepted any core-owned prompt lifecycle transition. Host adapters MUST be pure delegation wrappers; the engine-host boundary MUST NOT require adapters to own provider-specific streaming, request construction, or shared mutable conversation state.
r[embeddable-agent-engine.host-driven-contracts]

#### Scenario: host submits an accepted prompt through engine input
r[embeddable-agent-engine.accepted-prompt-engine-input]

- **WHEN** `clankers-core` accepts a prompt lifecycle or follow-up transition and controller adapter code normalizes it into engine prompt data
- **THEN** the adapter submits that accepted prompt to `clankers-engine` through engine-native input
- **THEN** `clankers-engine` owns pending model work, model/tool turn state, and continuation from that point forward
- **THEN** `clankers-engine` does not decide queued prompt replay, loop follow-up dispatch, auto-test follow-up dispatch, thinking-level updates, or disabled-tool filter state

#### Scenario: model execution is requested through explicit engine effects
r[embeddable-agent-engine.model-execution-effects-retained]

- **WHEN** the engine decides that model completion work is needed after accepted prompt submission
- **THEN** it emits an explicit model-request effect containing the correlated request identity and the model request payload
- **THEN** the host returns the model completion or model failure through a correlated engine input rather than the engine performing provider I/O directly

#### Scenario: tool execution is requested through explicit engine effects
r[embeddable-agent-engine.tool-execution-effects-retained]

- **WHEN** the engine decides that tool execution work is needed after model feedback requests tools
- **THEN** it emits an explicit tool-execution effect containing the correlated call identity, tool name, and structured tool input payload
- **THEN** the host returns the tool result or tool failure through a correlated engine input rather than the engine invoking tools directly

#### Scenario: semantic engine events stay separate from lifecycle events
r[embeddable-agent-engine.turn-events-lifecycle-events-separated]

- **WHEN** the engine surfaces model/tool turn progress, retry notices, cancellation, or terminal turn outcomes
- **THEN** it emits engine-native semantic events for those turn concerns
- **THEN** loop-state changes, queued-prompt replay, and session prompt-lifecycle busy changes remain core-owned lifecycle outputs translated by adapters outside the engine
- **THEN** engine turn busy and terminal `BusyChanged` events for accepted model/tool/retry work remain engine-owned turn outputs

#### Scenario: host adapters are pure delegation wrappers
r[embeddable-agent-engine.host-adapters-pure-delegation]

- **WHEN** a Clankers shell implements `ModelHost`, `ToolExecutor`, or other engine-host traits
- **THEN** the adapter struct delegates to purpose-built modules for provider I/O, tool dispatch, and transcript recording
- **THEN** the adapter struct does not contain inline streaming loops, request construction, capability gate checks, or shared mutable turn state
