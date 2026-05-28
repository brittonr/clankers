# Proposal: Unified Semantic Event Stream

## Problem

Clankers still has fragmented event models: `EngineEvent`, provider stream events, `AgentEvent`, controller-private `ControllerDomainEvent`, runtime `SessionEvent`, `DaemonEvent`, TUI events, Matrix events, and attach projection state. This makes SDK embedding and shell parity brittle because every boundary can synthesize or drop behavior differently.

## Proposed Change

Define a reusable semantic session event stream contract that represents prompt, assistant/thinking, tool, confirmation, usage, error, and completion semantics. Engine/runtime/agent/controller paths should project into this stream, and TUI/daemon/Matrix/attach adapters should project out of it.

## Impact

- **Files**: possible new event crate/module, `crates/clankers-runtime/src/events.rs`, `crates/clankers-controller/src/domain_event.rs`, `crates/clankers-agent/src/events.rs`, daemon/TUI/Matrix projection adapters.
- **Testing**: event ordering fixtures, projection parity matrix, source-boundary rail against display/protocol leakage.
