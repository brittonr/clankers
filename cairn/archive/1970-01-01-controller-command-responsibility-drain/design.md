# Design: Controller Command Responsibility Drain

## Context

Controller decoupling already centralized several projections, but `command.rs` is still the largest controller module. The next slice should reduce responsibility count, not just move helper functions.

## Decisions

### 1. Responsibility map first

Inventory command handling into translation, authorization, core input, runtime dispatch, persistence, continuation, and projection clusters before editing.

### 2. Extract one cluster with tests

Each extracted module should have a narrow API and focused tests that do not need daemon sockets.

### 3. Keep projection in existing owners

New command modules must call `convert`, `transport_convert`, or other existing projection owners instead of constructing protocol DTOs ad hoc.

## Risks / Trade-offs

- Splitting command code can duplicate state mutation; use deterministic replay tests.
- Protocol parity is fragile; keep attach/daemon behavior tests focused.
- Source rails may need AST updates when responsibilities move.
