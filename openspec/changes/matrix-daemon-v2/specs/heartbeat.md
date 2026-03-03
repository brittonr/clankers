# Heartbeat Scheduler

## Purpose

Proactive agent wake-up.  Users can say "remind me to deploy at 3pm" and
the agent writes to HEARTBEAT.md.  A background scheduler periodically
reads the file and prompts the agent if there's work to do.  This turns
the daemon from a reactive responder into something that can manage tasks
over time.

## Requirements

### Heartbeat configuration

The daemon MUST support a heartbeat interval, disabled by default.

Configuration via `DaemonConfig.heartbeat_interval: Option<Duration>`.
Environment variable: `CLANKERS_HEARTBEAT_INTERVAL` (Go-style duration
string: `30m`, `1h`, `5m`).

GIVEN `heartbeat_interval` is `None` or `0`
WHEN the daemon starts
THEN no heartbeat scheduler is started

### HEARTBEAT.md location

The heartbeat file MUST live at `<session-dir>/HEARTBEAT.md`.  Each
session has its own heartbeat file.

### Scheduler tick

The scheduler MUST check all active sessions (plus any session directories
with a HEARTBEAT.md on disk) every 60 seconds.  For each session where
the heartbeat interval has elapsed since the last beat:

1. Read `HEARTBEAT.md`
2. If the file is missing or effectively empty (only headers, blank lines,
   empty list items), skip — no API call
3. Otherwise, prompt the agent with the heartbeat contents

### Heartbeat prompt

The daemon MUST wrap the HEARTBEAT.md contents in a system prompt:

```
Read HEARTBEAT.md below. Follow any tasks listed there strictly.
Do not infer or repeat old tasks from prior conversations.
If nothing needs attention, reply with exactly: HEARTBEAT_OK

--- HEARTBEAT.md contents ---
<contents>
--- end HEARTBEAT.md ---
```

The prompt text SHOULD be configurable via
`DaemonConfig.heartbeat_prompt: Option<String>`.

### HEARTBEAT_OK suppression

If the agent response contains `HEARTBEAT_OK`, the daemon MUST suppress
the response (do not send it to the Matrix room).

GIVEN the agent responds with "HEARTBEAT_OK"
WHEN the daemon processes the heartbeat response
THEN nothing is sent to the Matrix room

### Non-idle heartbeats

Heartbeat prompts MUST NOT reset the session's `last_active` timestamp.
This ensures that sessions with only heartbeat activity are still reaped
by the idle timeout.

### Heartbeat typing

The daemon SHOULD send typing indicators during heartbeat processing,
same as for regular prompts.

### System prompt for HEARTBEAT.md

The agent's default system prompt SHOULD include instructions about
HEARTBEAT.md:

```
You have a file called HEARTBEAT.md in your session directory. A background
scheduler reads this file periodically and prompts you with its contents.
Use it for reminders and recurring tasks. When asked to remember or schedule
something, write it to HEARTBEAT.md. When you act on a task, mark it done
or remove it.
```
