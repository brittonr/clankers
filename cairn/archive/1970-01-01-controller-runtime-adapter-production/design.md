# Design: Move Controller Production Commands Through Runtime Adapter

## Summary

The controller already has a runtime adapter abstraction, but it is not yet the production path. This change makes the adapter the single command execution seam, with fake and agent-backed implementations differing only at the shell edge.

## Decisions

### 1. Adapter owns concrete agent operations

Prompt submission, cancellation reset, abort, thinking level application, disabled-tool application, and message seeding should flow through `ControllerRuntimeAdapter` or narrowly scoped runtime/session interfaces. `command.rs` should decide *what* control is requested, not mutate concrete `Agent` fields directly.

### 2. Keep reducer and projection in controller

Authorization checks, `CoreInput` translation, busy/pending-prompt lifecycle, continuation policy, and `SemanticEvent`/`DaemonEvent` projection stay in controller modules. The adapter returns semantic events and completion status; the controller remains the session lifecycle owner.

### 3. Production adapter can start as an internal bridge

If moving the agent-backed adapter out of `clankers-controller` is too large for the first slice, it may start in a named adapter module with a convergence receipt. The end state is for root/daemon assembly to inject the concrete adapter, letting reusable command policy avoid `Agent` imports.

### 4. Tests must exercise the shared command path

Fake-service fixtures are only useful if the same `handle_command` branches use the adapter in production. New tests should assert recorded fake controls for prompt, abort/reset, thinking, disabled tools, and session identity while existing daemon parity tests prove behavior is unchanged.

## Validation plan

- Controller fake-runtime fixtures for prompt/control lifecycle using `SessionController` command entry points.
- Agent-backed adapter fixtures that observe semantic events and completion mapping without sockets.
- Daemon/attach parity checks for prompt, abort, thinking, disabled tools, and replay-sensitive session id metadata.
- FCIS and lego architecture rails updated to name the adapter owner and fail on direct command-path `Agent` mutation drift.
