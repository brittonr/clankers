# Daemon & Actors

## Daemon mode

The daemon runs agent sessions as actor processes. Clients attach via Unix sockets (local) or iroh QUIC (remote).

```bash
clankers daemon start -d        # start background daemon
clankers daemon status          # show daemon info
clankers daemon create          # create a session
clankers attach [session-id]    # attach TUI to session
clankers attach --auto-daemon   # auto-start daemon + attach
clankers attach --remote <id>   # attach to remote daemon via iroh
clankers ps                     # list sessions
clankers daemon kill <id>       # kill a session
clankers daemon stop            # stop daemon
```

## ACP editor sessions

`clankers acp serve` runs a foreground stdio adapter for ACP-compatible editors. It is separate from the daemon socket/QUIC transport: editors launch the command, speak JSON request lines over stdin/stdout, and receive explicit unsupported-method errors for first-pass gaps.

```bash
clankers acp serve
clankers acp serve --session <id>
clankers acp serve --new --model <model>
```

The adapter logs normalized request metadata (`source`, `transport`, `method`, `status`) without request params so replay/debugging does not leak prompt content or editor secrets.

## Architecture

```
┌──────────┐     Unix socket      ┌──────────────┐
│  TUI     │◄───────────────────► │  Daemon      │
│  Client  │                      │              │
└──────────┘                      │  ┌────────┐  │
                                  │  │ Agent  │  │
┌──────────┐     iroh QUIC        │  │ Process│  │
│  Remote  │◄───────────────────► │  └────────┘  │
│  Client  │   (clankers/daemon/1)│              │
└──────────┘                      │  ┌────────┐  │
                                  │  │ Agent  │  │
                                  │  │ Process│  │
                                  │  └────────┘  │
                                  └──────────────┘
```

### Key components

- **`socket_bridge.rs`** — Unix socket control plane + SessionFactory. Handles client connections, session creation, and event broadcasting.
- **`quic_bridge.rs`** — iroh QUIC remote access using ALPN `clankers/daemon/1`.
- **`agent_process.rs`** — Wraps a `SessionController` as a named actor in the ProcessRegistry.
- **`SessionController`** — Transport-agnostic agent driver. Owns the Agent and SessionManager, handles commands, feeds events.

For the end-to-end prompt/event/provider/session path, see [Request Lifecycle](./request-lifecycle.md).

### Wire protocol

4-byte big-endian length prefix + JSON over Unix sockets or QUIC streams.

Flow: `Handshake → SessionInfo → ReplayHistory → streaming events`

See `crates/clankers-protocol/` for frame types, `DaemonEvent`, and `SessionCommand`.

## Actor system

The actor system ([`clanker-actor`](https://github.com/brittonr/clanker-actor)) provides Erlang-style process management:

- **ProcessRegistry** — Named actor registration with spawn, link, and shutdown
- **Signals** — Shutdown, Kill, Link, Monitor
- **`die_when_link_dies`** — Cascading shutdown when linked actors exit
- **Supervisors** — Process trees with restart policies

SubagentTool and DelegateTool spawn in-process `AgentProcess` actors in daemon mode (subprocess fallback in standalone).

### Spawning an actor

```rust
let registry = ProcessRegistry::new();
let handle = registry.spawn("my-agent", async move {
    // actor body
}).await;
```

### Linking

```rust
registry.link("parent", "child").await;
// if "parent" dies, "child" receives a shutdown signal
```
