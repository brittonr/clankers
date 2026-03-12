# EventLoopRunner Extraction

## Purpose

Define what moves out of `EventLoopRunner` into `SessionController`,
what stays, and how the two communicate. This is the critical refactor
that enables everything else.

## Requirements

### Current EventLoopRunner inventory

The `EventLoopRunner` struct currently owns these fields. Each MUST be
classified as controller-side or client-side.

**Moves to SessionController:**
- `event_rx: broadcast::Receiver<AgentEvent>` — agent event stream
- `session_manager: Option<SessionManager>` — JSONL persistence
- `loop_engine: LoopEngine` — loop/retry tracking
- `active_loop_id: Option<LoopId>` — current loop
- `loop_turn_output: String` — accumulated tool output for break conditions
- `hook_pipeline: Option<Arc<HookPipeline>>` — lifecycle hooks
- `audit: AuditTracker` — tool call timing and leak detection
- `tool_call_names: HashMap<String, String>` — call_id → tool_name mapping
- `auto_test_in_progress: bool` — prevents recursive auto-test triggers
- `bash_confirm_rx` — confirmation channel (controller holds oneshot senders)
- `todo_rx` — todo request channel (controller holds oneshot senders)
- `db: Option<Db>` — database handle
- `settings` — agent settings reference

**Stays in TUI EventLoopRunner (or equivalent):**
- `terminal: &mut Terminal<CrosstermBackend>` — rendering
- `app: &mut App` — TUI state
- `keymap: Keymap` — key dispatch configuration
- `panel_rx` — subagent panel events (display-only in client mode)
- `panel_tx` — subagent panel sender (for TUI-initiated events)
- `slash_registry` — slash command registry (both sides keep a copy)
- `plugin_manager` — needed for TUI-side plugin UI contributions

**Split between both:**
- `cmd_tx / done_rx` — in embedded mode, these channels connect directly.
  In client mode, `cmd_tx` sends over the socket and `done_rx` receives
  from the socket.

### Interface

The SessionController MUST expose this interface:

```rust
impl SessionController {
    /// Create from an Agent and supporting infrastructure
    fn new(agent: Agent, config: ControllerConfig) -> Self;

    /// Process a command. Returns immediately (async work happens internally).
    async fn handle_command(&mut self, cmd: SessionCommand) -> Result<()>;

    /// Drain pending events. Called in a loop by the transport layer.
    fn drain_events(&mut self) -> Vec<DaemonEvent>;

    /// Check if the agent is currently processing a prompt
    fn is_busy(&self) -> bool;

    /// Graceful shutdown
    async fn shutdown(&mut self);
}
```

### No direct App mutation

The SessionController MUST NOT call any method on `App`. All communication
from controller to TUI goes through `DaemonEvent`.

GIVEN the controller needs to show a system message
WHEN it would previously call `app.push_system("message", false)`
THEN it emits `DaemonEvent::SystemMessage { text: "message", is_error: false }`

### Embedded mode backward compatibility

In embedded mode (no daemon), the `EventLoopRunner` MUST use
`SessionController` internally, with in-process channels instead of
sockets.

```
User input → EventLoopRunner → SessionCommand → SessionController
SessionController → DaemonEvent → EventLoopRunner → App::handle_tui_event()
```

GIVEN the user runs `clankers` (no --daemon flag)
WHEN the TUI starts
THEN it creates a SessionController in-process
AND uses the same TUI code path as client mode
AND behavior is identical to the current implementation

This means the extraction is non-breaking. The current behavior is preserved
by wiring the controller directly instead of over a socket.

### Agent task migration

The background agent task (`agent_task.rs`) MUST move into the
SessionController. The controller owns the agent and runs prompts
internally, rather than spawning a separate tokio task that receives
`AgentCommand` on a channel.

GIVEN the controller receives `SessionCommand::Prompt`
WHEN it processes the command
THEN it calls `agent.prompt()` directly within its own task
AND emits events as the agent streams

### Incremental extraction

The extraction MUST be done incrementally, not as a big-bang rewrite.

Step 1: Create `SessionController` struct with the moved fields.
Step 2: Move `drain_agent_events()` logic to controller.
Step 3: Move session persistence logic to controller.
Step 4: Move loop engine logic to controller.
Step 5: Move audit tracker to controller.
Step 6: Move bash/todo confirmation routing to controller.
Step 7: Move agent task dispatch to controller.
Step 8: EventLoopRunner creates controller internally (embedded mode works).
Step 9: Add socket transport (client mode works).

Each step MUST leave the codebase compiling and tests passing.

### Testing

The SessionController MUST be testable without a terminal.

GIVEN a SessionController created with a mock provider
WHEN a `SessionCommand::Prompt { text: "hello" }` is sent
AND events are drained
THEN the events include `DaemonEvent::AgentStart` and `DaemonEvent::AgentEnd`

This is the primary advantage of extraction — the agent orchestration
logic becomes unit-testable without PTY harnesses or terminal setup.
