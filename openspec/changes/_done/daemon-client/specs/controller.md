# SessionController

## Purpose

Transport-agnostic orchestrator that owns one agent session. Accepts
`SessionCommand` inputs, emits `DaemonEvent` outputs. Does not know
about terminals, sockets, or rendering. This is the piece extracted
from `EventLoopRunner` that contains all the non-TUI logic.

## Requirements

### Ownership

The SessionController MUST own:
- The `Agent` instance (conversation loop, tool dispatch)
- The agent event receiver (`broadcast::Receiver<AgentEvent>`)
- The `SessionManager` (JSONL persistence)
- The `LoopEngine` (loop/retry iteration tracking)
- The `HookPipeline` (lifecycle, tool, git hooks)
- The `AuditTracker` (tool call timing, leak detection)
- Bash confirmation routing (holds oneshot senders)
- Todo request routing (holds oneshot senders)

The SessionController MUST NOT own or reference:
- Any terminal or rendering type (ratatui, crossterm)
- `App` or any TUI component struct
- Keymaps, themes, or panel state
- Mouse/clipboard/selection state

### Event translation

The SessionController MUST translate `AgentEvent` into `DaemonEvent`
before emitting. The existing `event_translator.rs` logic moves into
the controller (or the controller calls it).

GIVEN the agent emits `AgentEvent::ToolExecutionEnd { call_id, result, is_error }`
WHEN the controller processes the event
THEN it emits `DaemonEvent::ToolDone { call_id, text, images, is_error }`

### Command dispatch

The SessionController MUST accept `SessionCommand` variants and map
them to agent operations.

GIVEN a `SessionCommand::Prompt { text, images }`
WHEN the controller receives it
THEN it calls `agent.prompt()` or `agent.prompt_with_images()`
AND emits `DaemonEvent::AgentStart` when the agent begins

GIVEN a `SessionCommand::Abort`
WHEN the controller receives it
THEN it calls `agent.abort()`

GIVEN a `SessionCommand::SlashCommand { command, args }`
WHEN the command is an agent-side slash command (model, session, auth)
THEN the controller dispatches it to the slash handler
AND emits resulting `DaemonEvent::SystemMessage` events

### Confirmation routing

The SessionController MUST route bash confirmation requests to clients
and hold the oneshot sender until a response arrives.

GIVEN the bash tool emits a confirmation request
WHEN the controller receives it on `bash_confirm_rx`
THEN it assigns a `request_id` and emits `DaemonEvent::ConfirmRequest`
AND stores the oneshot sender keyed by `request_id`

GIVEN a `SessionCommand::ConfirmBash { request_id, approved }`
WHEN the controller receives it
THEN it looks up the stored oneshot sender by `request_id`
AND sends the approval decision
AND removes the stored sender

GIVEN a `SessionCommand::ConfirmBash` with an unknown `request_id`
WHEN the controller receives it
THEN it logs a warning and does nothing

### Loop integration

The SessionController MUST manage the loop engine, feeding tool output
and checking break conditions after each agent turn.

GIVEN an active loop with `max_iterations: 5`
WHEN the agent completes iteration 5
THEN the controller stops the loop
AND emits `DaemonEvent::SystemMessage` with the loop result

### Session persistence

The SessionController MUST persist messages to the session JSONL file
on every `AgentEvent::TurnEnd` and `AgentEvent::UserInput`.

GIVEN a session with persistence enabled
WHEN the agent finishes a turn
THEN the controller writes the turn's messages to the session file

### History replay

The SessionController MUST support replaying conversation history for
clients that attach mid-conversation.

GIVEN a `SessionCommand::ReplayHistory`
WHEN the controller receives it
THEN it emits `DaemonEvent::HistoryBlock` for each conversation block
AND emits `DaemonEvent::HistoryEnd` when complete

### Slash command split

Slash commands MUST be split into two categories:

**Agent-side** (dispatched by SessionController):
- `/model`, `/thinking` — model configuration
- `/session`, `/resume`, `/compact` — session management
- `/autotest` — auto-test configuration
- `/loop` — loop control
- `/hooks` — hook management
- `/auth`, `/token` — authentication
- `/clear`, `/reset` — conversation reset
- Any slash command that mutates agent state

**Client-side** (handled locally by the TUI):
- `/zoom`, `/layout`, `/panel` — BSP tiling operations
- `/theme` — visual theming
- `/copy`, `/yank` — clipboard
- `/branch`, `/switch`, `/merge`, `/cherry-pick` — branch UI navigation
- `/help`, `/keys` — help display
- `/quit` — client exit (not agent shutdown)
- Any slash command that only mutates display state

GIVEN a TUI client receives `/zoom` input
WHEN it checks the command category
THEN it handles it locally without sending a `SessionCommand`

GIVEN a TUI client receives `/model sonnet` input
WHEN it checks the command category
THEN it sends `SessionCommand::SlashCommand { command: "model", args: "sonnet" }`

### Capability enforcement

The SessionController MUST hold an `Option<Vec<Capability>>` and enforce
it on every tool call, independent of transport.

GIVEN a SessionController with `capabilities: Some([ToolUse("read,grep")])`
WHEN the agent attempts to call the `bash` tool
THEN the controller blocks the call and returns a tool error

GIVEN a SessionController with `capabilities: None`
WHEN the agent attempts to call any tool
THEN the controller allows it (full access)

GIVEN a SessionController with `capabilities: Some([FileAccess { prefix: "/home/user/project/", read_only: true }])`
WHEN the agent attempts to write to `/home/user/.ssh/id_rsa`
THEN the controller blocks the call

Tool filtering MUST happen at session creation (build the tool set once)
AND at execution time (defense in depth for tools that access paths
dynamically). This matches the existing `filter_tools_by_capabilities()`
in the daemon's `SessionStore`.

### Capability delegation for child agents

When the SessionController spawns a child agent (via subagent or delegate
tool), it MUST pass a capability set that is a subset of its own.

GIVEN a controller with `capabilities: Some([ToolUse("read,grep,bash")])`
WHEN it spawns a child with `ToolUse("read,grep")`
THEN the child is created with `capabilities: Some([ToolUse("read,grep")])`

GIVEN a controller with `capabilities: Some([ToolUse("read,grep")])`
WHEN it spawns a child with `ToolUse("*")`
THEN the spawn clamps the child's capabilities to `ToolUse("read,grep")`

GIVEN a controller with `capabilities: None` (full access)
WHEN it spawns a child with any capability set
THEN the child gets exactly what was requested (parent has no restrictions)

### Concurrency

The SessionController MUST serialize prompt execution. Only one prompt
runs at a time per controller.

GIVEN a prompt is in progress
WHEN a second `SessionCommand::Prompt` arrives
THEN the controller rejects it with `DaemonEvent::SystemMessage { is_error: true }`

### Shutdown

The SessionController MUST support graceful shutdown.

GIVEN a shutdown signal
WHEN the controller receives it
THEN it aborts any in-progress prompt
AND flushes session persistence
AND fires `SessionEnd` hook
AND drops all resources
