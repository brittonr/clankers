# daemon-client — Design

## Decisions

### D1: Native actors, not WASM isolation

**Choice:** Actors are tokio tasks with signal channels. No WASM sandbox.

**Rationale:** Clankers agents hold native resources (HTTP clients, file
handles, process handles, Arc'd providers) that can't cross a WASM boundary
without extensive host function bridging. Lunatic's process model is built
around WASM module instances — wrong abstraction level. The ~400 lines of
useful ideas (signals, linking, death propagation) are trivially
reimplemented on tokio.

**Alternatives considered:**
- Lunatic runtime as foundation — rejected. Wasmtime version conflict
  (lunatic pins 41, extism uses 37). WASM sandboxing forces all agent
  state through host functions. 14k lines of runtime for 400 lines of
  value.
- OS processes per agent — rejected for default path. IPC serialization
  overhead on every event, credential distribution complexity, no shared
  connection pools. Keep as opt-in for untrusted plugin execution later.
- Pure tokio tasks with no actor structure — rejected. No crash propagation,
  no supervised restart, no formal parent-child hierarchy. The current
  ad-hoc channel wiring is exactly the problem we're solving.

### D2: JSON over length-prefixed frames

**Choice:** `serde_json` serialization with `[4-byte length][JSON]` framing.

**Rationale:** Consistency with existing iroh RPC layer. Debuggable via
socat/ncat. Backward-compatible on field addition. Serialization is never
the bottleneck — the LLM API call is 100-1000x slower.

**Alternatives considered:**
- rkyv (zero-copy) — rejected. Alignment constraints, unsafe access,
  versioning pain, non-debuggable wire format. Gains are irrelevant for
  small text-heavy messages at human-interactive rates.
- bincode/msgpack — not needed yet. JSON overhead is measured in
  microseconds per frame. Would revisit only if profiling shows
  serialization in the hot path, which it won't.
- protobuf/flatbuffers — schema maintenance overhead, code generation
  step, overkill for an internal protocol between components of the same
  binary.

### D3: Unix domain sockets for local transport

**Choice:** One control socket + one socket per session under
`$XDG_RUNTIME_DIR/clankers/`.

**Rationale:** Zero-copy kernel-side for local connections. No TLS setup.
Filesystem-based discovery (ls the socket dir to see sessions). Natural
cleanup on process death (though we also clean up explicitly). Matches
how tmux, Docker, and other daemon tools work.

**Alternatives considered:**
- TCP localhost — works but adds TLS complexity for auth, port conflicts,
  and firewall issues. No advantage over Unix sockets for same-host.
- Named pipes (FIFOs) — unidirectional, no multiplexing, no concurrent
  clients. Already using one for trigger pipe (and it's painful).
- Shared memory ring buffer — fastest possible, but complex, and the
  bottleneck is rendering not IPC.

### D4: Embedded mode preserved as default

**Choice:** `clankers` (no flags) runs the agent in-process, exactly as
today. `clankers --daemon` spawns a session on the running daemon.
`clankers attach <id>` connects to an existing session.

**Rationale:** Zero disruption to current users. The daemon is opt-in.
Internally, embedded mode still uses SessionController — it's just wired
in-process instead of over a socket. This means both paths exercise the
same code.

**Alternatives considered:**
- Daemon-only (force all users to run daemon first) — rejected. Breaks
  the "just type clankers and go" experience. Many use cases don't need
  persistence or multi-client.
- Auto-start daemon on first run — considered but deferred. Adds
  complexity around daemon lifecycle management. Better to make it
  explicit first, then add auto-start later if users want it.

### D5: Slash command split by mutation target

**Choice:** Slash commands that mutate agent state go through
`SessionCommand`. Slash commands that mutate display state stay local.

**Rationale:** The split follows the ownership boundary. The controller
owns the agent, model, session, hooks. The TUI owns the panels, layout,
theme, clipboard. A command's category is determined by what it mutates,
not what it displays.

Gray area: `/branch`, `/switch`, `/merge` affect session tree structure
(controller concern) but also update the branch UI (TUI concern). These
go through `SessionCommand` for the mutation, and the resulting
`DaemonEvent` triggers the UI update.

### D6: History replay for mid-conversation attach

**Choice:** On attach, the client sends `ReplayHistory` and receives
the conversation as `HistoryBlock` events, then `HistoryEnd`.

**Rationale:** The TUI reconstructs its display state from events. No
need to serialize and transfer `App` state (which includes ratatui
types that aren't serializable). The client processes history events
through the same `handle_tui_event()` path as live events — no special
case needed.

**Alternatives considered:**
- Snapshot + delta (serialize App state, send incremental updates) —
  rejected. App contains ratatui Rects, crossterm state, non-serializable
  panel trait objects. Snapshotting would require a parallel serializable
  state type.
- No history (start fresh on attach) — rejected. Defeats the purpose
  of attach. You want to see what happened while detached.

### D7: Automerge for session tree, not for everything

**Choice:** Session tree, todo list, napkin, and peer registry become
Automerge CRDT documents. Settings, auth tokens, and ephemeral state
stay as-is.

**Rationale:** The session tree is already an append-only DAG of immutable
entries with unique IDs and parent pointers — that's literally what
Automerge stores. Switching eliminates ~300 lines of manual merge/cherry-pick
code in `crates/clankers-session/src/merge.rs` and enables concurrent writes
from multiple agents/clients without conflict. The todo list and napkin are
small shared-mutable documents with the same concurrent-write problem.

Settings, auth tokens, and streaming output don't benefit. Settings
need last-writer-wins (CRDT merge of `model: "sonnet"` + `model: "opus"`
picks one arbitrarily). Auth tokens need immediate authority (a revoked
token must not be merged back by a stale replica). Streaming output is
ephemeral and high-frequency — Automerge overhead is unjustified.

**Alternatives considered:**
- CRDT for everything — rejected. Overkill for settings, wrong semantics
  for auth, unnecessary overhead for ephemeral data.
- Keep JSONL, add manual multi-writer locking — rejected. The session tree
  already has branches, merges, cherry-picks. It's a CRDT problem being
  solved with non-CRDT tools. The merge code is complex and fragile.
- Custom CRDT (not Automerge) — rejected. Automerge is battle-tested,
  has iroh-docs integration via aspen, and handles the text/map/list
  types we need. No reason to reinvent.

### D8: Aspen as optional distributed backend

**Choice:** `SessionBackend` trait with local (Automerge files) and aspen
(KV + blobs) implementations. Agent work submittable as aspen jobs.

**Rationale:** The `SessionController` already abstracts persistence
through `SessionManager`. Adding a trait boundary lets us swap in aspen's
distributed KV (Raft-replicated, linearizable) for session storage and
aspen's blob store (content-addressed, BLAKE3) for tool artifacts. The
aspen job queue turns subagent work into distributable tasks that any
cluster node can execute.

Local mode remains the default. Aspen is opt-in via `--backend aspen`.
The Automerge CRDT layer handles offline-first for session data regardless
of backend — aspen adds durable distributed storage and compute.

**Alternatives considered:**
- Aspen as the only backend — rejected. Most users run clankers on a
  single machine with no cluster. Forcing a distributed system dependency
  for local use is hostile.
- SQLite/redb instead of aspen for distributed case — rejected. Neither
  provides multi-node replication. You'd have to build consensus on top,
  which is what aspen already does.
- S3/cloud storage backend — possible future addition via the same
  `SessionBackend` trait, but not worth specifying now.

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                  clankers daemon                     │
│                                                     │
│  ┌──────────────┐  ┌──────────────┐                 │
│  │ ProcessRegistry │  │ Supervisor   │               │
│  └──────┬───────┘  └──────┬───────┘                 │
│         │                  │                         │
│  ┌──────▼──────────────────▼─────┐                   │
│  │      AgentProcess (actor)      │                   │
│  │  ┌──────────────────────────┐ │                   │
│  │  │   SessionController      │ │  ◄── control.sock │
│  │  │  ┌─────┐ ┌────────────┐ │ │                   │
│  │  │  │Agent│ │SessionMgr  │ │ │                   │
│  │  │  │     │ │LoopEngine  │ │ │                   │
│  │  │  │     │ │HookPipeline│ │ │                   │
│  │  │  └─────┘ └────────────┘ │ │                   │
│  │  └──────────────────────────┘ │                   │
│  │                                │                   │
│  │  ┌──────────┐ ┌──────────┐   │                   │
│  │  │ Child    │ │ Child    │   │  session-xxx.sock  │
│  │  │ Agent    │ │ Agent    │   │       │            │
│  │  └──────────┘ └──────────┘   │       │            │
│  └──────────────────────────────┘       │            │
│                                          │            │
└──────────────────────────────────────────┼────────────┘
                                           │
                              ┌────────────▼───────────┐
                              │     TUI Client          │
                              │  ┌─────┐ ┌──────────┐  │
                              │  │ App │ │ Renderer  │  │
                              │  └─────┘ └──────────┘  │
                              └─────────────────────────┘
```

## Data Flow

### Embedded mode (no daemon)

```
Terminal event
  → EventLoopRunner.handle_terminal_events()
  → SessionCommand
  → SessionController.handle_command()
  → Agent.prompt()
  → AgentEvent (broadcast)
  → SessionController.drain_events()
  → DaemonEvent
  → EventLoopRunner.apply_daemon_event()
  → App.handle_tui_event()
  → terminal.draw()
```

### Client mode (with daemon)

```
Terminal event
  → EventLoopRunner.handle_terminal_events()
  → SessionCommand
  → Unix socket write
  ─── (daemon process boundary) ───
  → SessionController.handle_command()
  → Agent.prompt()
  → AgentEvent (broadcast)
  → SessionController.drain_events()
  → DaemonEvent
  → Unix socket write
  ─── (process boundary) ───
  → ClientAdapter
  → App.handle_tui_event()
  → terminal.draw()
```

Same data, same types, just a socket in the middle.

### Persistence layers

```
SessionController
    │
    ▼
SessionManager (Automerge doc)
    │
    ├── LocalSessionBackend (default)
    │     └── ~/.local/share/clankers/sessions/<id>/session.automerge
    │
    └── AspenSessionBackend (opt-in: --backend aspen)
          ├── KV: sessions/{id}/doc → Automerge bytes
          ├── KV: sessions/{id}/meta → JSON metadata
          ├── Blobs: tool outputs, images (BLAKE3 content-addressed)
          └── iroh-docs: real-time sync to TUI clients
```

### CRDT sync

```
Daemon SessionController
    │ (Automerge changes)
    ▼
Session document (Automerge)  ◄──── iroh-docs sync ────►  TUI local replica
Todo document (Automerge)     ◄──── iroh-docs sync ────►  TUI local replica
    │
    ▼ (if aspen backend)
aspen KV (Raft consensus) → DocsExporter → iroh-docs namespace
```
