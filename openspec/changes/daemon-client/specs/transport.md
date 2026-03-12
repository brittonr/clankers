# Transport

## Purpose

Define how daemon and clients connect and exchange protocol frames.
Primary transport is Unix domain sockets for local connections. QUIC
via iroh for remote connections (already exists, needs adaptation).

## Requirements

### Socket layout

The daemon MUST create a control socket and per-session sockets.

```
$XDG_RUNTIME_DIR/clankers/
├── control.sock          # session listing, creation, attach
├── session-abc123.sock   # session-specific event stream
├── session-def456.sock
└── daemon.pid            # PID file for single-instance enforcement
```

If `$XDG_RUNTIME_DIR` is unset, fall back to `/tmp/clankers-$UID/`.

GIVEN no daemon is running
WHEN `clankers daemon` starts
THEN it creates the socket directory and `control.sock`
AND writes its PID to `daemon.pid`

GIVEN a daemon is already running (PID file exists and process is alive)
WHEN `clankers daemon` starts
THEN it prints an error and exits

### Control socket protocol

The control socket MUST accept `ControlCommand` / `ControlResponse` frames.

```rust
enum ControlCommand {
    /// List active sessions
    ListSessions,
    /// Create a new session (returns the session socket path)
    CreateSession { model: Option<String>, system_prompt: Option<String> },
    /// Attach to an existing session (returns the session socket path)
    AttachSession { session_id: String },
    /// Query the process tree
    ProcessTree,
    /// Kill a specific session
    KillSession { session_id: String },
    /// Shutdown the daemon
    Shutdown,
    /// Daemon status (uptime, session count, resource usage)
    Status,
}

enum ControlResponse {
    Sessions(Vec<SessionSummary>),
    Created { session_id: String, socket_path: String },
    Attached { socket_path: String },
    Tree(Vec<ProcessInfo>),
    Killed,
    ShuttingDown,
    Status(DaemonStatus),
    Error { message: String },
}

struct SessionSummary {
    session_id: String,
    model: String,
    turn_count: usize,
    last_active: String,  // ISO 8601
    client_count: usize,
    socket_path: String,
}
```

GIVEN a running daemon with 2 sessions
WHEN a client sends `ControlCommand::ListSessions` to the control socket
THEN the daemon responds with `ControlResponse::Sessions` containing 2 entries

### Session socket protocol

Each session socket MUST carry the `SessionCommand`/`DaemonEvent` protocol
defined in the protocol spec.

Connection lifecycle:
1. Client connects to the session socket
2. Client sends `Handshake { protocol_version, client_name }`
3. Daemon responds with `DaemonEvent::SessionInfo`
4. Client optionally sends `SessionCommand::ReplayHistory`
5. Bidirectional command/event exchange begins
6. Client sends `SessionCommand::Disconnect` or drops the connection

GIVEN a TUI client connects to `session-abc123.sock`
WHEN it sends a valid `Handshake`
THEN the daemon responds with `SessionInfo`
AND begins streaming `DaemonEvent`s for that session

### Multiple clients per session

The daemon MUST support multiple simultaneous clients on one session socket.

Each connected client MUST receive all `DaemonEvent`s (broadcast).
Only one client's prompt can be active at a time (serialized).

GIVEN two TUI clients connected to the same session
WHEN client A sends `SessionCommand::Prompt`
THEN both client A and client B receive the resulting `DaemonEvent` stream

GIVEN two TUI clients connected to the same session
WHEN client A has an active prompt
AND client B sends `SessionCommand::Prompt`
THEN client B receives `DaemonEvent::SystemMessage { is_error: true }`

### Confirmation routing with multiple clients

When a bash confirmation is needed, the daemon MUST send the
`ConfirmRequest` to all connected clients. The first response wins.

GIVEN two TUI clients connected to the same session
WHEN the bash tool requests confirmation
THEN both clients receive `DaemonEvent::ConfirmRequest`
AND whichever client responds first determines the outcome
AND the other client receives a `DaemonEvent::SystemMessage` noting the decision

### Client disconnect handling

The daemon MUST handle client disconnects without affecting the agent.

GIVEN a TUI client connected to a session
WHEN the client's TCP connection drops (crash, network failure, Ctrl+C)
THEN the daemon removes the client from the broadcast list
AND the agent continues running uninterrupted

### Auto-create session on TUI start

The `clankers` command (no subcommand) SHOULD check for a running daemon
and attach to a new or existing session instead of running in-process.

```
clankers                  # in-process (no daemon) — existing behavior
clankers --daemon         # connect to daemon, create session
clankers attach <id>      # connect to daemon, attach to existing session
clankers attach           # connect to daemon, attach to most recent session
```

GIVEN a daemon is running
WHEN the user runs `clankers --daemon`
THEN the TUI connects to the control socket
AND sends `ControlCommand::CreateSession`
AND connects to the returned session socket
AND begins the TUI event loop as a client

GIVEN no daemon is running
WHEN the user runs `clankers --daemon`
THEN the TUI prints "no daemon running, use `clankers daemon` to start one"

### Authentication vs authorization

Authentication and authorization are separate concerns, enforced at
different layers.

**Authentication** (who are you?) is transport-specific:
- Unix socket: implicit. The connecting process has filesystem access to
  the socket, which means it's the same user. No token exchange needed
  for identity.
- QUIC (iroh): ed25519 public key from the TLS handshake. Verified
  against allowlist or UCAN token, same as existing `chat/1` auth.

**Authorization** (what can you do?) is transport-independent:
- Every `SessionController` holds an `Option<Vec<Capability>>`.
- Every tool call checks against the session's capability set.
- Child agents inherit a subset of their parent's capabilities.
- This enforcement happens regardless of how the session was created.

GIVEN a local TUI creates a session via Unix socket with no token
WHEN the session is created
THEN the SessionController gets `capabilities: None` (full access)
AND all tools are available

GIVEN a local TUI creates a session via Unix socket with a token
WHEN the token contains `ToolUse { tool_pattern: "read,grep" }`
THEN the SessionController gets `capabilities: Some([ToolUse("read,grep")])`
AND only read and grep tools are available

GIVEN a parent agent spawns a child agent in the actor tree
WHEN the parent has `ToolUse { tool_pattern: "read,grep,bash" }`
AND the parent delegates `ToolUse { tool_pattern: "read,grep" }`
THEN the child SessionController enforces `read,grep` only

The key point: UCAN is not just for remote peers. A local user may want
to run an agent with reduced capabilities (no bash, no file writes, no
network). The token is the mechanism for that regardless of transport.

### Capability in CreateSession

`ControlCommand::CreateSession` MUST accept an optional capability token.

```rust
CreateSession {
    model: Option<String>,
    system_prompt: Option<String>,
    token: Option<String>,  // base64-encoded UCAN token
}
```

GIVEN `clankers --daemon --read-only`
WHEN the TUI sends `CreateSession` with a read-only token
THEN the daemon verifies the token
AND creates a session with `capabilities: Some([ToolUse("read,grep,find,ls")])`

GIVEN `clankers --daemon` with no capability flags
WHEN the TUI sends `CreateSession` with `token: None`
THEN the daemon creates a session with `capabilities: None` (full access)

### QUIC transport (remote)

The existing iroh QUIC endpoint MUST support the same session protocol
for remote TUI clients.

A new ALPN `clankers/session/1` MUST be defined for session-level
connections (distinct from `clankers/rpc/1` and `clankers/chat/1`).

The daemon's iroh endpoint handler MUST accept `clankers/session/1`
connections and route them through the same `SessionCommand`/`DaemonEvent`
protocol as Unix sockets. The `ClientAdapter` is generic over transport
(`AsyncRead + AsyncWrite`), so the daemon wraps the QUIC bidi stream
the same way it wraps a Unix socket connection.

GIVEN a remote TUI client with the daemon's node ID
WHEN it connects via iroh QUIC with ALPN `clankers/session/1`
THEN the same `Handshake` → `SessionInfo` → command/event flow applies

GIVEN a remote client connects via QUIC
WHEN the handshake does not include a valid UCAN token
THEN the daemon rejects the connection (auth required for remote)

GIVEN a remote client connects via QUIC with a valid UCAN token
WHEN the token has `ToolUse { tool_pattern: "read,grep" }`
THEN the session is created with those capability restrictions

### IrohBiStream adapter

The system MUST provide an `IrohBiStream` wrapper that combines iroh's
`SendStream` and `RecvStream` into a single `AsyncRead + AsyncWrite`
type for use with `ClientAdapter`.

```rust
struct IrohBiStream {
    send: iroh::endpoint::SendStream,
    recv: iroh::endpoint::RecvStream,
}

impl AsyncRead for IrohBiStream { /* delegate to recv */ }
impl AsyncWrite for IrohBiStream { /* delegate to send */ }
```

GIVEN an iroh QUIC connection with an open bidi stream
WHEN wrapped in `IrohBiStream`
THEN it satisfies `AsyncRead + AsyncWrite + Unpin + Send`
AND can be used with `ClientAdapter` identically to a Unix socket

### Socket cleanup

The daemon MUST clean up socket files on graceful shutdown.

GIVEN a running daemon
WHEN it receives SIGTERM or `ControlCommand::Shutdown`
THEN it sends `Signal::Shutdown` to all agent processes
AND waits up to 10 seconds for graceful termination
AND removes all socket files and the PID file

GIVEN stale socket files from a crashed daemon
WHEN a new daemon starts
THEN it checks the PID file against running processes
AND removes stale files before creating new ones
