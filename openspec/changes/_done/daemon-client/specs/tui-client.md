# TUI Client Mode

## Purpose

Define how the TUI operates as a display client connected to a daemon
session over a socket, rather than running the agent in-process.

## Requirements

### Two TUI modes

The TUI MUST support two operational modes:

1. **Embedded** (current behavior) — agent runs in-process, no daemon.
   `EventLoopRunner` owns the agent directly. This remains the default
   for `clankers` with no flags.

2. **Client** — TUI connects to a daemon session socket. No local agent.
   Receives `DaemonEvent` stream, sends `SessionCommand`.

The TUI rendering, keybindings, panels, BSP tiling, and App state machine
MUST work identically in both modes.

GIVEN the TUI running in client mode
WHEN the user sees a streaming response
THEN it looks identical to embedded mode

### ClientAdapter

The system MUST provide a `ClientAdapter` that maps `DaemonEvent` into
`App::handle_tui_event()` calls and maps user actions to `SessionCommand`.

The `ClientAdapter` MUST be generic over the transport stream. It takes
any `AsyncRead + AsyncWrite + Unpin + Send` — not a concrete socket type.

```rust
struct ClientAdapter<S: AsyncRead + AsyncWrite + Unpin + Send> {
    reader: FrameReader<S>,
    writer: FrameWriter<S>,
    event_tx: broadcast::Sender<TuiEvent>,
}
```

This allows the same adapter code to be instantiated with:
- `tokio::net::UnixStream` for local daemon connections
- `(iroh::endpoint::SendStream, iroh::endpoint::RecvStream)` for remote
  connections via iroh QUIC

```rust
// Local
let stream = UnixStream::connect(socket_path).await?;
let adapter = ClientAdapter::new(stream);

// Remote
let conn = endpoint.connect(node_id, ALPN_SESSION).await?;
let (send, recv) = conn.open_bi().await?;
let adapter = ClientAdapter::new(IrohBiStream::new(send, recv));
```

The `ClientAdapter` replaces the `event_rx: broadcast::Receiver<AgentEvent>`
that `EventLoopRunner` currently drains. Instead of:

```
AgentEvent → event_translator → TuiEvent → App::handle_tui_event()
```

The flow becomes:

```
DaemonEvent (from socket/QUIC) → daemon_to_tui_event() → App::handle_tui_event()
```

GIVEN a `DaemonEvent::TextDelta { text }` arrives on any transport
WHEN the ClientAdapter processes it
THEN it calls `app.handle_tui_event(&TuiEvent::TextDelta(text))`

GIVEN a `ClientAdapter` instantiated with a Unix socket
WHEN events are processed
THEN behavior is identical to one instantiated with an iroh QUIC stream

### Client-side slash commands

The TUI MUST handle client-side slash commands without sending them to
the daemon.

GIVEN the user types `/zoom` in client mode
WHEN the TUI checks the command category
THEN it handles the zoom toggle locally
AND does NOT send a `SessionCommand`

GIVEN the user types `/model gpt-4` in client mode
WHEN the TUI checks the command category
THEN it sends `SessionCommand::SlashCommand { command: "model", args: "gpt-4" }`

### Confirmation UX

The TUI MUST render bash confirmation prompts from `DaemonEvent::ConfirmRequest`
identically to the current inline confirmation UI.

GIVEN a `DaemonEvent::ConfirmRequest` arrives
WHEN the TUI renders it
THEN the user sees the same y/n prompt as embedded mode
AND pressing `y` sends `SessionCommand::ConfirmBash { approved: true }`

### History replay on attach

When connecting to a session with existing conversation, the TUI MUST
request and render the history.

GIVEN a TUI attaches to a session with 10 conversation blocks
WHEN the TUI sends `SessionCommand::ReplayHistory`
THEN it receives 10 `DaemonEvent::HistoryBlock` events
AND renders them as conversation blocks in the chat view
AND the `DaemonEvent::HistoryEnd` event triggers a scroll-to-bottom

### Reconnection

The TUI SHOULD attempt reconnection on socket disconnect.

GIVEN a TUI in client mode
WHEN the socket connection drops unexpectedly
THEN the TUI shows a "disconnected — reconnecting..." status message
AND retries connection with exponential backoff (1s, 2s, 4s, max 30s)
AND on reconnection, sends `ReplayHistory` to restore state

GIVEN reconnection fails after 5 minutes
WHEN the timeout expires
THEN the TUI shows "connection lost" and offers to quit or keep retrying

### Status bar

The TUI MUST indicate daemon connection status in the status bar.

Embedded mode: `model_name | tokens | session_id`
Client mode: `model_name | tokens | session_id | 🔌 daemon`

GIVEN the TUI is in client mode
WHEN the status bar renders
THEN it includes a daemon indicator

### Subagent panel

Subagent events from the daemon MUST route to the existing subagent
panel and BSP panes.

GIVEN a `DaemonEvent::SubagentStarted` arrives
WHEN the TUI processes it
THEN the SubagentPanel and SubagentPaneManager update identically
to how they update from the in-process `SubagentEvent` channel

### Remote attach

The TUI MUST support attaching to a daemon on another machine via iroh.

```
clankers attach --remote <node-id> [session-id]
```

The `node-id` is the daemon's ed25519 public key — the same identifier
peers already exchange for iroh communication. It can be a full node ID
or a name from `~/.clankers/agent/peers.json`.

Connection flow:
1. TUI creates (or reuses) a local iroh endpoint
2. TUI connects to `node-id` with ALPN `clankers/session/1`
3. If `session-id` is given, TUI sends it in the `Handshake`
4. If no `session-id`, daemon picks the most recent session (or creates one)
5. UCAN auth token is sent in the handshake (required for remote)
6. Normal `SessionCommand`/`DaemonEvent` exchange begins

GIVEN a daemon running on machine A with node ID `abc123...`
WHEN a user on machine B runs `clankers attach --remote abc123`
THEN the TUI connects via iroh QUIC
AND sends a `Handshake` with a UCAN capability token
AND the daemon verifies the token
AND the TUI receives `SessionInfo` and begins rendering

GIVEN a user runs `clankers attach --remote abc123` with no stored token
WHEN the connection attempt begins
THEN the TUI prints "no auth token for this peer — run `clankers token create` on the remote host and import with `clankers token import`"

GIVEN the remote daemon is behind NAT
WHEN the TUI connects via iroh
THEN iroh's MagicSocket handles hole punching or relay fallback
AND the user does not need to configure port forwarding

### Status bar (remote)

Remote connections MUST show the remote node in the status bar.

Embedded mode: `model_name | tokens | session_id`
Local client: `model_name | tokens | session_id | 🔌 daemon`
Remote client: `model_name | tokens | session_id | 🌐 abc123..ef`

GIVEN the TUI is connected to a remote daemon
WHEN the status bar renders
THEN it shows a globe indicator and the short node ID

### Detach without quit

The TUI MUST support detaching from a session without stopping the agent.

GIVEN the user presses the detach key (or runs `/detach`)
WHEN the TUI is in client mode
THEN it sends `SessionCommand::Disconnect`
AND exits the TUI cleanly
AND the daemon agent continues running

GIVEN the user presses `/quit` in client mode
WHEN the TUI processes it
THEN it only exits the TUI client
AND does NOT shut down the agent
AND to kill the agent, the user must use `clankers kill <session-id>` or `/kill`
