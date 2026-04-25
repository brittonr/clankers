## ADDED Requirements

### Requirement: Engine host MUST expose composable async execution contracts

The system MUST provide a reusable host-facing layer that interprets engine effects through caller-supplied model, tool, sleep, event, and cancellation adapters instead of requiring embedders to depend on `clankers-agent`.
r[embeddable-agent-engine.composable-host-contract]

#### Scenario: host runner drives engine effects through traits
r[embeddable-agent-engine.host-runner-traits]
- **WHEN** an embedding crate wants to run a complete engine turn
- **THEN** it can provide trait implementations for model execution, tool execution, retry sleeping, event emission, cancellation, and usage observation
- **THEN** the reusable runner executes `EngineEffect` values and feeds correlated `EngineInput` feedback back into the reducer
- **THEN** the runner does not require daemon, TUI, built-in tool bundle, session DB, or Clankers prompt assembly dependencies

#### Scenario: Clankers agent becomes default assembly
r[embeddable-agent-engine.agent-default-assembly]
- **WHEN** existing Clankers interactive, daemon, or attach flows run a turn
- **THEN** they use the reusable host runner through Clankers-specific adapters
- **THEN** existing shell-visible behavior for streaming, tool execution, retries, cancellation, usage updates, model switching, hooks, and event emission remains unchanged

### Requirement: Tool execution surface MUST be reusable outside clankers-agent

The system MUST provide a tool-host surface that can execute tool calls requested by the engine without importing the full Clankers agent runtime.
r[embeddable-agent-engine.reusable-tool-host]

#### Scenario: tool catalog and executor are independent host components
r[embeddable-agent-engine.tool-host-catalog]
- **WHEN** an embedding crate builds an agent with a custom tool set
- **THEN** it can supply a tool catalog and executor compatible with engine tool-call effects
- **THEN** the executor supports successful results, tool errors, missing tools, capability denial, cancellation, and output truncation as explicit host outcomes

#### Scenario: plugin-backed tools share the same executor seam
r[embeddable-agent-engine.plugin-tool-adapter]
- **WHEN** WASM or stdio plugin tools are enabled
- **THEN** they are exposed through the same tool-host executor seam as built-in tools
- **THEN** plugin runtime details remain outside `clankers-engine` and outside the generic host-runner policy

### Requirement: Stream accumulation MUST be reusable deterministic logic

The system MUST expose deterministic stream-folding logic that turns provider stream events into model responses without depending on Clankers TUI or event bus plumbing.
r[embeddable-agent-engine.reusable-stream-accumulator]

#### Scenario: stream folding handles normal model output
r[embeddable-agent-engine.stream-folding-positive]
- **WHEN** a model stream emits text, thinking, tool-use JSON deltas, usage deltas, and message stop events
- **THEN** reusable accumulator logic returns canonical assistant content, usage, model name, and stop reason
- **THEN** UI/event-bus forwarding remains adapter-only behavior around that deterministic fold

#### Scenario: stream folding rejects or normalizes malformed inputs deterministically
r[embeddable-agent-engine.stream-folding-negative]
- **WHEN** a model stream emits malformed tool JSON, non-object tool JSON, missing block starts, duplicate indexes, late deltas, or provider error events
- **THEN** the accumulator returns a deterministic normalized result or explicit error according to documented rules
- **THEN** positive and negative tests cover those paths without standing up a provider or TUI

### Requirement: Host extraction rails MUST prevent clankers-agent from regaining runner ownership

The system MUST add validation rails proving reusable async turn-driving policy lives in the host layer and `clankers-agent` remains the Clankers default assembly.
r[embeddable-agent-engine.host-extraction-rails]

#### Scenario: source rails reject duplicated runner policy
r[embeddable-agent-engine.no-duplicated-runner-policy]
- **WHEN** validation inventories non-test `clankers-agent::turn` code after extraction
- **THEN** it fails if that code reintroduces authoritative model/tool/retry/cancellation continuation loops instead of delegating to the reusable host runner
- **THEN** adapter code may still translate Clankers events, build provider requests, emit hooks, update usage, and bridge model-switch state

#### Scenario: runtime parity rails cover host adapters
r[embeddable-agent-engine.host-adapter-parity]
- **WHEN** validation runs after host extraction
- **THEN** focused runtime tests prove the Clankers adapters preserve streaming deltas, tool-call events, tool failures, retry backoff behavior, cancellation behavior, usage updates, hook dispatch, and model switching while using the reusable host runner
