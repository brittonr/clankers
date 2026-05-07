## ADDED Requirements

### Requirement: Embeddable runtime facade [r[embeddable-runtime-api.facade]]

The system MUST expose a host-facing runtime facade that allows another Rust application to construct a Clankers session, submit prompts, receive typed events, and shut down cleanly without using CLI subprocesses, daemon sockets, ACP/MCP bridges, or TUI state as the primary integration boundary.

#### Scenario: host creates a session [r[embeddable-runtime-api.facade.create-session]]

- GIVEN a host application has constructed the runtime with explicit settings, provider, tool, and storage choices
- WHEN the host requests a new session
- THEN the runtime returns a session handle with stable prompt, control, event-stream, and shutdown operations
- THEN the host does not need to instantiate TUI `App` state, daemon socket bridges, or CLI command structs

#### Scenario: host resumes or identifies a session [r[embeddable-runtime-api.facade.session-identity]]

- GIVEN the host supplies a session id or asks the runtime to allocate one
- WHEN prompts are executed through the session handle
- THEN runtime/provider metadata preserves session identity for request shaping, persistence, and replay
- THEN session identity is exposed through host-facing metadata rather than daemon-only protocol frames

### Requirement: Host-facing event stream [r[embeddable-runtime-api.events]]

The runtime MUST provide a typed host-facing session event stream whose public event variants represent semantic agent/session/tool/cost/confirmation outcomes and do not expose TUI widget types, terminal event-loop types, raw daemon frames, or ACP/MCP JSON-RPC envelopes.

#### Scenario: prompt streams semantic events [r[embeddable-runtime-api.events.prompt-stream]]

- GIVEN a host submits a prompt through a session handle
- WHEN the turn runs and produces assistant text, thinking, tool calls, tool results, confirmations, errors, usage, or completion
- THEN the host receives typed semantic events in causal order
- THEN UI/transport adapters can translate those events into daemon, TUI, ACP, MCP, Matrix, or app-specific outputs

#### Scenario: event metadata is safe for host routing [r[embeddable-runtime-api.events.safe-metadata]]

- GIVEN a runtime event includes metadata for replay or debugging
- WHEN the event is emitted to a host application
- THEN metadata includes safe ids, counts, labels, hashes, statuses, and error classes
- THEN metadata MUST NOT include raw credentials, headers, environment values, or hidden prompt/context contents unless that data is already explicit event content

### Requirement: Runtime adapter parity rails [r[embeddable-runtime-api.adapter-parity]]

The system MUST add deterministic rails proving that at least one existing Clankers shell path can use or match the embeddable runtime prompt/control/event semantics without duplicating a separate lifecycle implementation.

#### Scenario: daemon or headless prompt parity [r[embeddable-runtime-api.adapter-parity.prompt]]

- GIVEN a deterministic fake-provider prompt fixture
- WHEN the fixture is run through the embeddable runtime path and an existing headless or daemon path
- THEN both paths preserve prompt acceptance, session identity, provider request metadata, assistant/tool event ordering, and terminal completion semantics

#### Scenario: public API rejects transport leakage [r[embeddable-runtime-api.adapter-parity.no-leakage]]

- GIVEN boundary validation inspects the public embeddable runtime API
- WHEN validation runs
- THEN the public API does not expose `DaemonEvent`, `SessionCommand`, TUI widget types, ACP request types, MCP request types, or CLI argument structs as required host-facing types
