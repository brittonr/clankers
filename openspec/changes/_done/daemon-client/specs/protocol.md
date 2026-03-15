# Protocol

## Purpose

Define the wire protocol between daemon and clients (TUI, CLI, other agents).
Two enums — `SessionCommand` (client → daemon) and `DaemonEvent` (daemon →
client) — carried over length-prefixed JSON frames on a Unix domain socket
or QUIC stream.

## Requirements

### SessionCommand enum

The system MUST define a `SessionCommand` enum that covers all client-to-daemon
operations. Each variant maps to an existing `AgentCommand` or new daemon
operation.

```rust
enum SessionCommand {
    /// Send a prompt to the agent
    Prompt { text: String, images: Vec<ImageData> },
    /// Cancel the current operation
    Abort,
    /// Reset cancellation state (allow new prompts after abort)
    ResetCancel,
    /// Switch the active model
    SetModel { model: String },
    /// Clear conversation history
    ClearHistory,
    /// Truncate to N messages
    TruncateMessages { count: usize },
    /// Set thinking level
    SetThinkingLevel { level: String },
    /// Cycle thinking level
    CycleThinkingLevel,
    /// Seed initial messages (session restore)
    SeedMessages { messages: Vec<SerializedMessage> },
    /// Replace the system prompt
    SetSystemPrompt { prompt: String },
    /// Get the current system prompt
    GetSystemPrompt,
    /// Switch account credentials
    SwitchAccount { account: String },
    /// Update disabled tools
    SetDisabledTools { tools: Vec<String> },
    /// Respond to a bash confirmation request
    ConfirmBash { request_id: String, approved: bool },
    /// Respond to a todo action request
    TodoResponse { request_id: String, response: serde_json::Value },
    /// Execute a slash command (agent-side only)
    SlashCommand { command: String, args: String },
    /// Request session history replay (on attach)
    ReplayHistory,
    /// Query the session's active capabilities
    GetCapabilities,
    /// Graceful disconnect
    Disconnect,
}
```

GIVEN a TUI client connected to a daemon session
WHEN the user types a prompt and presses enter
THEN the client sends `SessionCommand::Prompt` with the text

GIVEN a TUI client connected to a daemon session
WHEN the bash tool requests confirmation
THEN the daemon sends `DaemonEvent::ConfirmRequest`
AND the client responds with `SessionCommand::ConfirmBash`

### DaemonEvent enum

The system MUST define a `DaemonEvent` enum that covers all daemon-to-client
notifications. This is a superset of `TuiEvent` with session metadata.

```rust
enum DaemonEvent {
    // ── Agent lifecycle ─────────────────────────
    AgentStart,
    AgentEnd,

    // ── Streaming ───────────────────────────────
    ContentBlockStart { is_thinking: bool },
    TextDelta { text: String },
    ThinkingDelta { text: String },
    ContentBlockStop,

    // ── Tool events ─────────────────────────────
    ToolCall { tool_name: String, call_id: String, input: serde_json::Value },
    ToolStart { call_id: String, tool_name: String },
    ToolOutput { call_id: String, text: String, images: Vec<ImageData> },
    ToolProgressUpdate { call_id: String, progress: serde_json::Value },
    ToolChunk { call_id: String, content: String, content_type: String },
    ToolDone { call_id: String, text: String, images: Vec<ImageData>, is_error: bool },

    // ── Session events ──────────────────────────
    UserInput { text: String, agent_msg_count: usize },
    SessionCompaction { compacted_count: usize, tokens_saved: usize },
    UsageUpdate { input_tokens: u64, output_tokens: u64, cache_read: u64, model: String },
    ModelChanged { from: String, to: String, reason: String },

    // ── Confirmation requests ───────────────────
    ConfirmRequest { request_id: String, command: String, working_dir: String },
    TodoRequest { request_id: String, action: serde_json::Value },

    // ── Session metadata ────────────────────────
    SessionInfo { session_id: String, model: String, system_prompt_hash: String },
    SystemPromptResponse { prompt: String },

    // ── Subagent events ─────────────────────────
    SubagentStarted { id: String, name: String, task: String, pid: Option<u32> },
    SubagentOutput { id: String, line: String },
    SubagentDone { id: String },
    SubagentError { id: String, message: String },

    // ── Capability events ───────────────────────
    /// Response to GetCapabilities — None means full access
    Capabilities { capabilities: Option<Vec<String>> },
    /// Tool call was blocked by capability enforcement
    ToolBlocked { call_id: String, tool_name: String, reason: String },

    // ── System messages ─────────────────────────
    SystemMessage { text: String, is_error: bool },
    PromptDone { error: Option<String> },

    // ── History replay ──────────────────────────
    HistoryBlock { block: serde_json::Value },
    HistoryEnd,
}
```

GIVEN a client sends `SessionCommand::ReplayHistory`
WHEN the daemon has existing conversation blocks
THEN the daemon sends `DaemonEvent::HistoryBlock` for each block
AND finishes with `DaemonEvent::HistoryEnd`

### ImageData type

The system MUST define image payloads as base64-encoded data with a media type,
matching the existing `DisplayImage` structure.

```rust
struct ImageData {
    data: String,       // base64-encoded
    media_type: String, // e.g., "image/png"
}
```

### Framing

The system MUST use length-prefixed JSON frames, reusing the existing
`write_frame`/`read_frame` functions from the iroh RPC layer.

Each frame is: `[4-byte big-endian length][JSON payload]`.

GIVEN a `SessionCommand::Prompt { text: "hello" }`
WHEN serialized to a frame
THEN the frame is `[length][{"Prompt":{"text":"hello","images":[]}}]`

### Serialization

The system MUST use `serde_json` for serialization.

The system MUST NOT use rkyv, bincode, msgpack, or other binary formats.
JSON provides debuggability (wire inspection via socat/ncat), backward
compatibility on field addition, and consistency with the existing RPC layer.

### Transport independence

The protocol types MUST NOT depend on any transport crate (tokio-net, iroh,
hyper). The `clankers-protocol` crate contains only types, serialization,
and frame helpers. Transport bindings live in the daemon and client code.

GIVEN `clankers-protocol` in Cargo.toml
WHEN checking its dependencies
THEN only `serde`, `serde_json`, and standard library types are present

### Versioning

The system SHOULD include a protocol version in the initial handshake.

```rust
struct Handshake {
    protocol_version: u32,  // starts at 1
    client_name: String,    // e.g., "clankers-tui/0.1.0"
}
```

GIVEN a client connects to a daemon
WHEN the client sends a `Handshake` with `protocol_version: 1`
AND the daemon supports version 1
THEN the daemon responds with `SessionInfo`

GIVEN a client sends a `Handshake` with `protocol_version: 99`
AND the daemon only supports version 1
THEN the daemon responds with an error and closes the connection
