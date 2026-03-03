# matrix-daemon-v2 — Design

## Decisions

### Use `!` prefix for bot commands, not `/`

**Choice:** `!restart` instead of `/restart`
**Rationale:** Matrix clients intercept `/` as their own slash commands.
OpenCrow uses `!` for the same reason.  Unknown `!` commands pass through
to the agent (safe fallback).
**Alternatives considered:** Custom prefix (`clankers:restart`), mention-based
(`@bot restart`).  Both are more typing for no benefit.

### Sendfile tag protocol for file sending

**Choice:** `<sendfile>/path</sendfile>` tags in agent text output
**Rationale:** Same protocol as OpenCrow — proven to work.  The agent
already has file paths from tool execution.  No changes to the tool system
needed.  The daemon strips tags and uploads.
**Alternatives considered:** Dedicated tool (`matrix_upload`), structured
tool result metadata.  Both require agent/tool changes; the tag approach
is zero-change on the agent side.

### Heartbeat via HEARTBEAT.md file, not database

**Choice:** File-based heartbeat at `<session-dir>/HEARTBEAT.md`
**Rationale:** The agent can read/write it with existing file tools.
Human-readable.  Inspectable outside the daemon.  Same approach as OpenCrow.
**Alternatives considered:** redb table, structured JSON.  Both require
dedicated tools for the agent to manage tasks — unnecessary complexity.

### Named pipes for triggers, not HTTP webhooks

**Choice:** FIFO at `<session-dir>/trigger.pipe`
**Rationale:** Zero deps.  Works from any process that can write to a file.
No HTTP server, no auth, no ports.  Same approach as OpenCrow.
**Alternatives considered:** HTTP endpoint on the daemon, Unix socket with
JSON protocol.  Both add complexity; the FIFO is simpler and sufficient.

## Architecture

No new crates or major structural changes.  All features are additions to:

- `src/modes/daemon.rs` — heartbeat, trigger pipe, idle reaper, command dispatch
- `crates/clankers-matrix/src/client.rs` — typing notices, file upload/download, HTML messages
- `crates/clankers-matrix/src/config.rs` — `allowed_users` field

```
┌─────────────────────────────────────────────────────┐
│                    Daemon                            │
│                                                     │
│  ┌──────────┐  ┌──────────┐  ┌──────────────────┐  │
│  │ Heartbeat│  │ Trigger  │  │  Idle Reaper     │  │
│  │ Scheduler│  │ Pipe Mgr │  │  (60s tick)      │  │
│  └────┬─────┘  └────┬─────┘  └────────┬─────────┘  │
│       │              │                 │            │
│       ▼              ▼                 ▼            │
│  ┌─────────────────────────────────────────────┐    │
│  │           SessionStore                       │    │
│  │   sessions: HashMap<SessionKey, LiveSession> │    │
│  │   + last_active timestamps                   │    │
│  └──────────────────┬──────────────────────────┘    │
│                     │                               │
│  ┌──────────────────▼──────────────────────────┐    │
│  │         Matrix Bridge Loop                   │    │
│  │  1. Allowlist check                          │    │
│  │  2. Bot command dispatch                     │    │
│  │  3. Typing indicator start                   │    │
│  │  4. Agent prompt (+ file attachments)        │    │
│  │  5. Empty response re-prompt                 │    │
│  │  6. Sendfile tag extraction + upload         │    │
│  │  7. HTML formatting + chunking               │    │
│  │  8. Typing indicator stop                    │    │
│  └──────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────┘
```

## Data Flow

### Normal message flow

```
User sends message in Matrix room
  → Bridge event loop receives BridgeEvent::TextMessage
  → Allowlist check (reject silently if denied)
  → Bot command check (dispatch if `!` command)
  → Start typing indicator (spawn refresh task)
  → Download any attachments to session dir
  → Agent prompt (with file paths if attachments)
  → Collect response text
  → Empty check (re-prompt once if empty)
  → Extract <sendfile> tags → upload files to Matrix
  → Convert markdown to HTML
  → Chunk if needed
  → Send formatted message(s) to room
  → Stop typing indicator
```

### Heartbeat flow

```
Heartbeat timer fires (every N minutes per session)
  → Read <session-dir>/HEARTBEAT.md
  → Skip if empty/missing
  → Start typing indicator
  → Prompt agent with heartbeat contents
  → If response contains HEARTBEAT_OK → suppress
  → Otherwise send response to Matrix room
  → Stop typing indicator
  → Do NOT update last_active
```

### Trigger flow

```
External process writes line to <session-dir>/trigger.pipe
  → Reader task receives line
  → Start typing indicator
  → Prompt agent with trigger contents
  → If response contains HEARTBEAT_OK → suppress
  → Otherwise send response to Matrix room
  → Stop typing indicator
  → Do NOT update last_active
```
