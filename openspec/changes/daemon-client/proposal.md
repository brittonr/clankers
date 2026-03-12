# daemon-client

## Intent

The TUI and the agent are fused into one process. If the TUI crashes, the
agent dies mid-tool-execution. If you close your terminal, the conversation
is gone. You can't watch an agent work from two screens, you can't detach
and reattach, and subagents are spawned as dumb child processes that
communicate via stdout line scraping.

Split clankers into a daemon that runs agents and a TUI that renders state.
The daemon owns agent lifecycles, tool execution, session persistence, and
capability enforcement. The TUI becomes a display client that subscribes to
an event stream and sends commands over a socket. Other clients (CLI,
Matrix, iroh peers) use the same protocol.

Make each agent a supervised process in a tree. Parents spawn children,
delegate capabilities, and get notified on crash. This replaces the ad-hoc
subprocess spawning in SubagentTool/DelegateTool with a structured actor
model using native tokio tasks (not WASM isolation — wrong level of
abstraction for this problem).

## Scope

### In Scope

- `clankers-protocol` crate — wire types: `SessionCommand`, `DaemonEvent`
- `clankers-controller` crate — transport-agnostic `SessionController`
- `clankers-actor` crate — process registry, signals, linking, supervision
- Unix domain socket transport between daemon and TUI
- `clankers attach` command — connect TUI to a running daemon session
- Agent process tree — daemon supervisor spawns agent actors, agents spawn
  child agents for subagent/delegate work
- Bash confirmation and todo requests routed over the protocol
- Session history replay on client attach (mid-conversation join)
- `clankers ps` — show the process tree

- Session tree as Automerge CRDT document — replaces JSONL append log
  and manual merge/cherry-pick with native concurrent-write semantics
- iroh-docs sync for live session replication between daemon and clients
- Todo list and napkin as Automerge CRDT documents
- Aspen cluster as optional backend (KV for sessions, blobs for artifacts,
  job queue for distributed agent execution)

### Out of Scope

- WASM-based process isolation (use native tokio tasks, not lunatic)
- Distributed multi-node supervision (single host only; iroh for remote)
- Changes to the TUI rendering, BSP tiling, or panel system
- Changes to the LLM provider layer
- Web UI client (protocol supports it, but building one is separate work)
- Hot code reloading

## Approach

Five phases, each independently shippable:

1. **Protocol crate** — define `SessionCommand` and `DaemonEvent` as serde
   enums with JSON serialization. Reuse the existing `write_frame`/`read_frame`
   length-prefixed framing from the iroh RPC layer.

2. **SessionController extraction** — pull agent-command dispatch, event
   draining, session persistence, loop engine, and hook pipeline out of
   `EventLoopRunner` into a transport-agnostic controller. The TUI event
   loop becomes a thin adapter that maps terminal events to `SessionCommand`
   and `DaemonEvent` to `App::handle_tui_event()`.

3. **Unix socket transport** — daemon listens on
   `$XDG_RUNTIME_DIR/clankers/<session-id>.sock`. TUI connects, subscribes
   to the event stream, sends commands. Multiple clients can attach to the
   same session (broadcast).

4. **Actor layer** — `ProcessRegistry`, `ProcessHandle`, `Signal` enum
   (Message, Kill, Link, LinkDied, Shutdown), supervisor strategies. Root
   daemon becomes a supervisor. SubagentTool/DelegateTool spawn child
   actors instead of raw subprocesses.

5. **CRDT session layer** — replace JSONL session persistence with
   Automerge documents. Session tree branching and merging become native
   concurrent operations. Sync between daemon and clients via iroh-docs.
   Todo list and napkin become CRDT documents.

6. **Aspen backend** (optional) — `SessionBackend` trait with local
   (Automerge files) and aspen (KV + blobs) impls. Agent work submittable
   as aspen jobs for distributed execution.

7. **Polish** — reconnection, `clankers ps`, resource budgets per agent,
   graceful shutdown cascading.
