# Design: Make Controller Runtime Boundary Lego-Clean

## Summary

The controller should coordinate session commands and core prompt lifecycle. It should not be the owner of concrete agent internals, desktop session stores, database search, display state, or transport frames. This change turns the existing adapter seam into the production boundary.

## Current coupling points

- `SessionController` stores `Option<Agent>`, `SessionManager`, `HookPipeline`, `SearchIndex`, and queues `DaemonEvent`.
- `runtime_adapter.rs::AgentBackedRuntimeAdapter` is a narrow owner but still directly exposes many `Agent` mutation methods.
- `convert.rs` is the right projection owner, but command paths and persistence still carry protocol/session assumptions.

## Decisions

### 1. Production runtime is injected

Daemon/root assembly should choose an agent-backed or runtime-backed adapter. Controller command policy should call the adapter rather than owning concrete agent mutation.

### 2. Projection remains centralized

Daemon/TUI/protocol events are edge projections from semantic/domain events. Command policy may request output but must not reconstruct transport DTOs inline.

### 3. Persistence is a service

Session persistence/search should be accessed through controller/runtime session services with a compatibility adapter for the current `SessionManager` path.

## Validation plan

- Fake-runtime fixtures for prompt, cancel, thinking, disabled tools, resume identity, and semantic event projection.
- Agent-backed parity tests for the desktop daemon path.
- Source rails over command, runtime adapter, persistence, and convert modules.
- Socketless controller tests for command lifecycle.
