## Context

The controller replay rail validates in-memory shell behavior. Session resume additionally depends on session persistence and restore code, which can drift independently from turn execution.

## Goals

- Exercise a real persisted-session/resume path without credentials or network.
- Reuse scripted provider/tool behavior to keep the test cheap and deterministic.
- Pin semantic request shape after resume: session id, roles, user prompt, assistant tool use, tool result, and follow-up prompt.
- Keep the deterministic harness fast and discoverable.

## Non-Goals

- No live daemon socket, QUIC attach, or provider auth.
- No full UI visual replay snapshots.
- No broad persistence format migration unless the test exposes a narrowly fixable defect.

## Decisions

### 1. Root integration test over crate-only unit test

**Choice:** Add the rail under root `tests/` so it can compose `SessionController`, `Agent`, provider, tools, and session persistence helpers.

**Rationale:** The risk is cross-crate shell integration, not a pure persistence helper.

### 2. Isolated temp state and scripted provider

**Choice:** Use temp session/config state and in-memory fake provider/tool implementations.

**Rationale:** Replay must be credential-free, network-free, and stable across runs.

### 3. Normalized semantic receipt

**Choice:** Normalize provider requests, restored events/history, and tool calls while preserving semantic fields.

**Rationale:** BLAKE3 detects drift while avoiding volatile timestamp/path sensitivity.

## Risks / Trade-offs

- **Resume helper friction:** If no small public helper exists, use the narrowest real persistence API available rather than testing private implementation details.
- **Over-broad assertions:** Field-level assertions should pin semantic history without freezing incidental timestamps.
- **Harness cost:** Keep the rail to one focused test so `deterministic` remains cheap.
