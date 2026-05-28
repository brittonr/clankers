# Semantic Event Stream Specification

## Purpose

Defines the `semantic-event-stream` capability.

## Requirements

### Requirement: Semantic event contract covers session behavior [r[semantic-event-stream.contract]]

Clankers MUST provide a reusable semantic event contract for prompt, model, tool, confirmation, usage, error, and completion behavior without binding to TUI, daemon, Matrix, provider-native, or root-shell DTOs.

#### Scenario: Event variants cover core behavior [r[semantic-event-stream.contract.semantic-coverage]]
- GIVEN a turn emits prompt acceptance, assistant text, thinking, tool call, tool result, confirmation, usage, error, completion, or shutdown information
- WHEN the semantic event stream represents it
- THEN a typed event variant MUST carry the semantic data and safe metadata
- AND the event type MUST NOT expose TUI widgets, daemon frames, Matrix types, provider-native payloads, credentials, or hidden prompt context

### Requirement: Runtime and agent project into semantic events [r[semantic-event-stream.inbound-projection]]

Runtime and agent/controller paths MUST converge on the semantic event stream before transport/display projection.

#### Scenario: Agent and runtime emit shared semantics [r[semantic-event-stream.inbound-projection.agent-runtime]]
- GIVEN runtime or agent/controller execution observes engine/provider/tool activity
- WHEN events are produced
- THEN they MUST be emitted as, or converted into, the shared semantic event contract
- AND duplicate event policy MUST NOT be independently reimplemented in every shell path

### Requirement: Transport/display adapters project out of semantic events [r[semantic-event-stream.edge-projection]]

Daemon, TUI, Matrix, attach, JSON, and batch outputs MUST project from semantic events at their edges.

#### Scenario: Edge projection preserves behavior [r[semantic-event-stream.edge-projection.transport-display]]
- GIVEN a semantic event fixture is projected to daemon, TUI, Matrix, attach, JSON, or batch surfaces
- WHEN the projection runs
- THEN existing user-visible or protocol-visible behavior MUST be preserved for covered events
- AND projection code MUST NOT synthesize domain policy that belongs in runtime/agent/engine layers

### Requirement: Controller domain event seam converges [r[semantic-event-stream.migration]]

The controller-private domain event seam MUST either be replaced by the shared semantic event contract or kept as a named compatibility adapter with a convergence path.

#### Scenario: Controller domain event has one owner [r[semantic-event-stream.migration.controller-domain-event]]
- GIVEN controller event translation is inspected
- WHEN semantic event migration is complete
- THEN there MUST be one owner for domain-to-transport projection
- AND any remaining controller-private event type MUST document why it exists and how it converges with the shared event contract

### Requirement: Semantic event verification is deterministic [r[semantic-event-stream.verification]]

Verification MUST prove event ordering, projection parity, and metadata safety.

#### Scenario: Ordering fixtures cover causal flow [r[semantic-event-stream.verification.ordering-fixtures]]
- GIVEN deterministic runtime and agent/controller fixtures run
- WHEN prompt accepted, assistant/thinking deltas, tool start/result, usage, error, and completion events are emitted
- THEN causal order and required metadata MUST match the fixture expectations

#### Scenario: Edge parity and redaction are tested [r[semantic-event-stream.verification.edge-parity]]
- GIVEN semantic events are projected to edge surfaces
- WHEN projection parity tests run
- THEN daemon/TUI/attach/JSON outputs MUST match expected shapes for covered events
- AND metadata redaction MUST prevent credential, header, environment, hidden prompt, and provider payload leakage

#### Scenario: Closeout validates event stream [r[semantic-event-stream.verification.closeout]]
- GIVEN implementation is complete
- WHEN focused validation runs
- THEN event/controller/runtime tests, FCIS boundary rail, Cairn validation/gates, and diff checks MUST pass
