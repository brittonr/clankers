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

## Selected drain slice

`scripts/check-controller-runtime-boundary.rs` is the closeout owner receipt for this change. It classifies `SessionController` concrete fields, keeps `runtime_adapter.rs` as the prompt/control runtime owner, limits `persistence.rs` to the desktop compatibility persistence/search adapter, and keeps daemon/TUI/semantic projection in `convert.rs`.

The selected production-compatible command lifecycle is `SessionController::{submit_prompt_with_runtime_adapter,apply_control_with_runtime_adapter,handle_command_with_runtime_adapter_for_test}`. Existing fake-runtime and agent-backed tests exercise prompt, abort/reset, thinking, disabled tools, session identity, and semantic projection without sockets/providers for the reusable path while preserving the agent-backed desktop path.

## Validation plan

- Fake-runtime fixtures for prompt, cancel, thinking, disabled tools, resume identity, and semantic event projection.
- Agent-backed parity tests for the desktop daemon path.
- Source rails over command, runtime adapter, persistence, and convert modules.
- Socketless controller tests for command lifecycle.
