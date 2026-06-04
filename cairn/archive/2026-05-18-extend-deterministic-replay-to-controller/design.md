## Context

`clankers-engine` now has BLAKE3-pinned deterministic replay fixtures. The next high-value seam is the shell that turns agent/controller state into provider calls and tool execution. This layer owns session id propagation, provider message conversion, tool correlation, and user-visible events.

## Goals / Non-Goals

**Goals:**
- Execute one complete scripted user → model tool-call → tool result → final-answer turn through the controller/agent seam.
- Assert stable provider request shape, session metadata, tool result correlation, normalized events/transcript, and BLAKE3 hashes.
- Keep the rail credential-free and isolated from ambient config/auth/session state.
- Include the new rail in `scripts/test-harness.sh deterministic`.

**Non-Goals:**
- Live provider/API coverage.
- Daemon socket, QUIC, Matrix, or TUI attach coverage.
- Full parity with every tool type; one representative deterministic tool and one rejection/negative case are enough for this slice.

## Decisions

### 1. Prefer existing test seams before adding new production surface

**Choice:** Inspect current controller/agent/provider tests first and use existing fake provider or adapter seams where possible.
**Rationale:** This avoids production API churn and keeps the first slice small.
**Alternative:** Add a full new replay framework now; rejected as too broad for the first controller boundary rail.

### 2. Normalize semantic artifacts only

**Choice:** Hash normalized provider requests, emitted events, transcript/messages, and tool results while preserving session ids, roles, tool names, inputs, outputs, and errors.
**Rationale:** The rail should catch shell drift, not hide it behind broad normalization.

### 3. Harness profile remains cheap

**Choice:** Add the controller replay test to the existing deterministic profile rather than a new top-level profile.
**Rationale:** Operators should have one cheap deterministic command for credential-free replay coverage.

## Risks / Trade-offs

- **Existing seams are too coupled to live providers** → add narrow test-only fake provider hooks or an integration test helper instead of touching runtime behavior.
- **Async/event ordering differs** → normalize only documented volatile ordering if semantically unordered; otherwise assert stable order.
- **Harness runtime grows** → keep fixtures tiny and avoid daemon/network startup.
