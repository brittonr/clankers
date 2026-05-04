## Context

Clankers has a daemon-client architecture where clients communicate with sessions using `SessionCommand` and receive `DaemonEvent` streams. The TUI and attach paths already rely on that substrate for prompts, aborts, thinking-level changes, disabled-tool updates, confirmations, history replay, plugin/tool queries, and session status. Existing MCP integration focuses on consuming external MCP servers as agent-visible tools; this change adds the inverse direction: clankers publishes a local MCP bridge for safe session control.

Self-evolution should run as an external orchestrator over that bridge rather than as privileged in-process mutation. This keeps experiments visible, replayable, interruptible, and subject to the same confirmation/policy gates as human sessions.

## Goals / Non-Goals

**Goals:**

- Make MCP another first-class session client, equivalent in authority to attach/TUI clients where operations overlap.
- Centralize command mapping so user/TUI, attach, and MCP inputs converge on the same `SessionCommand` variants.
- Return mutation receipts backed by daemon events or session state evidence.
- Provide a safe self-evolution outer loop that can evaluate candidate artifacts without live in-place mutation.
- Preserve human interrupt/approval control when self-evolution is running.

**Non-Goals:**

- MCP does not directly manipulate TUI widgets, reducers, raw terminal input, or private controller fields.
- MCP does not get privileged bypass for confirmations, disabled tools, trust modes, or model/tool ceilings.
- Self-evolution does not install evolved skills/prompts/code automatically.
- First pass is local stdio-only; remote/network MCP auth is a separate change.

## Decisions

### 1. MCP is an adapter over session commands

**Choice:** Implement MCP tools/resources as an adapter that attaches to a clankers daemon session and emits ordinary `SessionCommand` values.

**Rationale:** The session protocol is already the user-visible substrate. Reusing it prevents drift, preserves replay/persistence, and keeps daemon policy authoritative.

**Rejected alternative:** Calling `SessionController` or TUI `App` methods directly. That would create a hidden path with different behavior and weaker auditability.

**Implementation:** Add a small command-mapping layer, for example `McpSessionAction -> SessionCommand`, with explicit support and unsupported errors for every MCP operation.

### 2. TUI parity is verified by command/event equivalence

**Choice:** Each MCP mutation must be tested against the corresponding TUI/attach/slash path where one exists.

**Rationale:** The core promise is not just that MCP can perform actions, but that it performs the same actions the user does.

**Implementation:** Add tests that route representative human inputs and MCP calls through their adapters and compare the resulting `SessionCommand`, daemon event, persisted metadata, or confirmation behavior.

### 3. Receipts are event-backed

**Choice:** MCP mutation calls return structured receipts only after they can point to accepted command submission and, when available, a resulting daemon event/state observation.

**Rationale:** External orchestrators and self-evolution loops need audit evidence, not just optimistic "sent" responses.

**Implementation:** Receipt fields include source `mcp`, session id, action, command variant, status, event id/sequence when available, timestamp, and sanitized error details. Receipts do not include raw secrets, full prompt text unless the call itself is explicitly returning user-requested history, environment values, or provider payloads.

### 4. Self-evolution runs outside the authority boundary

**Choice:** Self-evolution is an orchestrated workflow over MCP/session commands plus local isolated artifact directories/worktrees.

**Rationale:** The self-evolver should be able to run experiments and propose candidates, but it should not mutate active skills/prompts/code or bypass the same gates that protect normal sessions.

**Implementation:** Add a disabled-by-default `self_evolution` CLI/tooling surface or MCP prompt/resource bundle that can run baseline-vs-candidate loops with fake/deterministic executors first. Candidate adoption produces a review packet and requires explicit human approval.

### 5. Local stdio first, remote later

**Choice:** First implementation is a local stdio MCP bridge.

**Rationale:** It avoids network auth, multi-tenant access, and remote capability concerns while proving the substrate and self-evolution loop.

**Rejected alternative:** Exposing the daemon/session control plane over network MCP immediately.

## Risks / Trade-offs

**Command drift** → Mitigate with shared mapping helpers and parity tests for every supported operation.

**Overbroad MCP authority** → Mitigate with allowlisted tools, explicit unsupported errors, session capability ceilings, and confirmation gates.

**Prompt/history leakage** → Mitigate with local-only transport, safe metadata defaults, redaction, and explicit resource selection for history/event reads.

**Self-evolution reward hacking** → Mitigate with deterministic checks, held-out evals, isolated candidates, no live mutation, and human promotion gates.

**Event receipt races** → Mitigate with accepted-command receipts first and best-effort event correlation; tests cover both immediate status and later stream evidence.

## Validation Plan

- Unit-test MCP action parsing and command mapping.
- Integration-test stdio MCP bridge with a fake or temp daemon/session socket.
- Add parity tests for prompt, abort, thinking level, disabled tools/capabilities, confirmation approval/denial, and history/status observation.
- Add negative tests proving unsupported/private operations fail and MCP cannot bypass confirmation/capability restrictions.
- Add self-evolution dry-run tests using fake evaluator/executor and isolated output directories.
- Run targeted checks, then `cargo check --tests -p clankers-protocol -p clankers` and OpenSpec validation.
