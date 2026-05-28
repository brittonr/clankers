## Phase 1: Event contract and projections

- [x] [serial] I1: Define a reusable semantic event contract for prompt acceptance, assistant/thinking deltas, tool lifecycle/results, confirmations, usage, errors, completion, and shutdown. [covers=r[semantic-event-stream.contract.semantic-coverage]]
- [x] [serial] I2: Implement runtime and agent/controller projection into the semantic stream without exposing daemon/TUI/provider-native DTOs. [covers=r[semantic-event-stream.inbound-projection.agent-runtime]]
- [x] [parallel] I3: Implement daemon/TUI/Matrix/attach/JSON edge projection from semantic events, preserving existing user-visible behavior. [covers=r[semantic-event-stream.edge-projection.transport-display]]
- [x] [serial] I4: Replace controller-private domain event ownership with the shared semantic event contract or a compatibility adapter with a named convergence path. [covers=r[semantic-event-stream.migration.controller-domain-event]]

## Phase 2: Verification

- [x] [parallel] V1: Add ordering fixtures for prompt accepted, assistant/thinking deltas, tool start/result, usage, error, and completion across runtime and agent/controller paths. [covers=r[semantic-event-stream.verification.ordering-fixtures]]
- [x] [parallel] V2: Add projection parity tests for daemon/TUI/attach/JSON outputs and redaction tests for event metadata. [covers=r[semantic-event-stream.verification.edge-parity]]
- [x] [serial] V3: Run event/controller/runtime focused tests, FCIS boundary rail, Cairn validate/gates, and `git diff --check`. [covers=r[semantic-event-stream.verification.closeout]]
