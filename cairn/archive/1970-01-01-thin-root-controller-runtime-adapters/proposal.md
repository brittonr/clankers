# Proposal: Thin Root and Controller Runtime Adapters

## Problem

The root crate still depends on almost every internal subsystem, and `clankers-controller` still directly depends on agent, config, db, provider, protocol, session, hooks, and TUI DTOs. Some of that is legitimate shell wiring, but the boundary is not yet thin enough to make Clankers feel like composed Lego blocks.

## Proposed Change

Make root and controller act as adapter shells around runtime/session services. Root should parse CLI and choose adapters; controller should translate commands/events and own transport/session orchestration while delegating runtime execution, persistence, provider/tool construction, and display/protocol projection to explicit bricks.

## Impact

- **Files**: root `src/**`, `crates/clankers-controller`, `src/modes/daemon`, `src/modes/attach`, `src/runtime_services.rs`, architecture rail scripts.
- **Testing**: dependency budget receipts, controller adapter fixtures, daemon/attach parity tests, FCIS boundary updates.
