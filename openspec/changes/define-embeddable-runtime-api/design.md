## Context

`SessionController`, `clankers-agent`, `clankers-engine`, `clankers-provider`, and `clankers-session` are already separated enough to support an embedding facade. The missing piece is an intentional product API with host-owned inputs, events, and control handles.

## Decisions

### Runtime facade as the embedding boundary

**Choice:** Add a small runtime facade crate rather than declaring `SessionController` or the daemon protocol to be the embedding API.

**Rationale:** `SessionController` remains a useful internal shell, but it still carries clankers session semantics and daemon event names. A facade can stabilize host-facing concepts while internally delegating to controller/agent/engine seams.

**Rejected:** Embedding by invoking the CLI or connecting to the daemon socket. That keeps transport, global paths, and lifecycle policy coupled to the terminal app.

### Typed host events, not TUI or daemon events

**Choice:** Publish a host-facing `RuntimeEvent` / `SessionEvent` stream that can be translated to `DaemonEvent` and TUI events by adapters.

**Rationale:** Host applications need semantic events (assistant delta, tool start, confirmation request, cost update, completion) without carrying terminal render or protocol framing types.

### Adapters over runtime

**Choice:** Existing CLI, TUI, daemon, ACP, MCP, and Matrix shells should converge on runtime/session handles over time.

**Rationale:** This prevents a second, incompatible embeddable path and makes embedding parity testable against existing behavior.

## Risks / Trade-offs

- **Facade too thin:** If it only renames `SessionController`, embedders still inherit daemon-shaped semantics. Mitigate with public API boundary tests.
- **Facade too broad:** If it absorbs tools, provider auth, prompt assembly, and storage all at once, it becomes another app shell. Mitigate with explicit dependency injection and follow-up OpenSpecs for stores/tools/prompts.
- **Adapter churn:** Migrating all shells at once is risky. Start with a narrow prompt/control/event slice and parity rails.
