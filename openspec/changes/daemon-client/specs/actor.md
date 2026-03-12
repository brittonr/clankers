# Actor Layer

## Purpose

Lightweight native actor primitives for managing agent process trees.
Not a WASM runtime — these are tokio tasks with Erlang-style signals,
linking, and supervision. Replaces the ad-hoc subprocess spawning in
SubagentTool and DelegateTool with a structured model.

## Requirements

### Process identity

The system MUST assign each actor a unique monotonic `ProcessId` (u64).
IDs are never reused within a runtime instance.

```rust
type ProcessId = u64;
```

### Signal enum

The system MUST define a `Signal` enum for inter-process communication.

```rust
enum Signal {
    /// Opaque application message (typed via downcast)
    Message(Box<dyn Any + Send>),
    /// Immediate termination — no cleanup
    Kill,
    /// Graceful shutdown with a timeout before Kill
    Shutdown { timeout: Duration },
    /// Establish a bidirectional link
    Link { tag: Option<i64>, process_id: ProcessId },
    /// Remove a link
    UnLink { process_id: ProcessId },
    /// Notification that a linked process died
    LinkDied { process_id: ProcessId, tag: Option<i64>, reason: DeathReason },
    /// Start monitoring (unidirectional — monitor gets notified, target doesn't)
    Monitor { watcher: ProcessId },
    /// Stop monitoring
    StopMonitoring { watcher: ProcessId },
    /// Notification that a monitored process exited
    ProcessDied { process_id: ProcessId, reason: DeathReason },
}
```

### DeathReason

```rust
enum DeathReason {
    /// Process finished its work normally
    Normal,
    /// Process failed with an error
    Failed(String),
    /// Process was killed by a signal
    Killed,
    /// Process was shut down gracefully
    Shutdown,
}
```

### ProcessHandle

The system MUST provide a handle to interact with a running actor.

```rust
struct ProcessHandle {
    id: ProcessId,
    signal_tx: UnboundedSender<Signal>,
    join: JoinHandle<DeathReason>,
    name: Option<String>,
    parent: Option<ProcessId>,
    capabilities: Vec<Capability>,
}
```

GIVEN a `ProcessHandle` for a running actor
WHEN `handle.send(Signal::Kill)` is called
THEN the actor's signal channel receives `Kill`

GIVEN a `ProcessHandle` for a terminated actor
WHEN `handle.send(Signal::Message(...))` is called
THEN the send returns without error (fire-and-forget, matches lunatic semantics)

### ProcessRegistry

The system MUST maintain a global process registry for lookup by ID and name.

```rust
struct ProcessRegistry {
    processes: DashMap<ProcessId, ProcessHandle>,
    names: DashMap<String, ProcessId>,
    next_id: AtomicU64,
}
```

The registry MUST support:
- `spawn(name, future) -> ProcessHandle` — register and run a tokio task
- `get(id) -> Option<ProcessHandle>` — lookup by ID
- `get_by_name(name) -> Option<ProcessHandle>` — lookup by name
- `send(id, signal)` — send a signal to a process by ID
- `remove(id)` — deregister a terminated process
- `children(id) -> Vec<ProcessId>` — list child processes

GIVEN a process spawned with name "agent:session-abc"
WHEN `registry.get_by_name("agent:session-abc")` is called
THEN it returns the process handle

### Linking semantics

When two processes are linked, the death of one MUST notify the other
via `Signal::LinkDied`.

GIVEN process A linked to process B
WHEN process B exits with `DeathReason::Failed("timeout")`
THEN process A receives `Signal::LinkDied { process_id: B, reason: Failed("timeout") }`

GIVEN process A linked to process B with `die_when_link_dies: true` (default)
WHEN process A receives `Signal::LinkDied` with `reason: Failed`
THEN process A also terminates with `DeathReason::Killed`

GIVEN process A linked to process B with `die_when_link_dies: false`
WHEN process A receives `Signal::LinkDied` with `reason: Failed`
THEN process A receives the signal as a message and continues running

### Parent-child relationships

The system MUST track parent-child relationships in the registry.

GIVEN process P spawns child process C
WHEN C is registered
THEN `C.parent == Some(P.id)` in the registry

GIVEN process P is killed
WHEN P has children [C1, C2, C3]
THEN all children receive `Signal::Shutdown { timeout }` first
AND after the timeout, any surviving children receive `Signal::Kill`

This is hierarchical shutdown — not in lunatic, but needed for agent trees.

### Supervisor

The system MUST provide a `Supervisor` that restarts failed children
according to a strategy.

```rust
enum SupervisorStrategy {
    /// Restart only the failed child
    OneForOne,
    /// Restart all children if any one fails
    OneForAll,
    /// Restart the failed child and all children started after it
    RestForOne,
}

struct SupervisorConfig {
    strategy: SupervisorStrategy,
    max_restarts: u32,
    restart_window: Duration,
}
```

GIVEN a supervisor with `OneForOne` strategy
WHEN child "worker-1" fails with `DeathReason::Failed`
THEN the supervisor restarts "worker-1" only
AND other children are unaffected

GIVEN a supervisor with `max_restarts: 3, restart_window: 60s`
WHEN a child fails 4 times within 60 seconds
THEN the supervisor itself shuts down with `DeathReason::Failed`
AND its parent is notified via `Signal::LinkDied`

### Agent process

The system MUST wrap `SessionController` in an actor that participates
in the process tree.

```rust
struct AgentProcess {
    controller: SessionController,
    // receives SessionCommand via typed messages
    // emits DaemonEvent to parent and/or connected clients
}
```

GIVEN the root daemon supervisor
WHEN a new iroh connection arrives with session key "iroh:abc123"
THEN the supervisor spawns an `AgentProcess` actor
AND links it to the supervisor
AND registers it as "agent:iroh:abc123"

GIVEN an `AgentProcess` running for session "agent:iroh:abc123"
WHEN the subagent tool is invoked
THEN the agent spawns a child `AgentProcess`
AND links the child to itself
AND the child's UCAN capabilities are a subset of the parent's

### Event forwarding

Child agent events MUST be forwarded to the parent with a prefix.

GIVEN parent agent P with child agent C named "worker:research"
WHEN C emits `DaemonEvent::TextDelta { text: "found 3 files" }`
THEN P receives `SubagentOutput { id: "worker:research", line: "found 3 files" }`

This maps to the existing `SubagentEvent` model but over the actor signal
channel instead of a dedicated `panel_tx` mpsc channel.

### Process tree query

The system MUST support querying the full process tree for display.

```rust
struct ProcessInfo {
    id: ProcessId,
    name: Option<String>,
    parent: Option<ProcessId>,
    children: Vec<ProcessId>,
    state: ProcessState,  // Running, Shutting Down, Dead
    uptime: Duration,
}
```

GIVEN `clankers ps` is invoked
WHEN the daemon has a process tree with 3 agents
THEN it returns a list of `ProcessInfo` structs
AND they form a tree rooted at the supervisor

### Capability delegation in the tree

The system MUST enforce that child processes have capabilities that
are a subset of (or equal to) their parent's capabilities.

GIVEN parent agent P with `ToolUse { tool_pattern: "read,grep,bash" }`
WHEN P spawns child agent C with `ToolUse { tool_pattern: "*" }`
THEN the spawn MUST fail or clamp C's capabilities to P's set

GIVEN parent agent P with `ToolUse { tool_pattern: "read,grep,bash" }`
WHEN P spawns child agent C with `ToolUse { tool_pattern: "read,grep" }`
THEN the spawn succeeds (C is a subset of P)

### No WASM dependency

The actor crate MUST NOT depend on wasmtime, extism, or any WASM runtime.
Actors are native tokio tasks.

GIVEN `clankers-actor` in Cargo.toml
WHEN checking its dependencies
THEN only `tokio`, `dashmap`, `serde`, and standard library types are present
