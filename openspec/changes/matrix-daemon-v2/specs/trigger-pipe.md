# Trigger Pipes

## Purpose

Let external processes wake the bot immediately by writing to a named pipe.
Cron jobs, mail watchers, webhook handlers, CI notifications — anything that
can `echo "..." > trigger.pipe` gets a direct line to the agent.

## Requirements

### FIFO creation

The daemon MUST create a named pipe (FIFO) at `<session-dir>/trigger.pipe`
when a session directory is initialized.  If a non-FIFO file exists at
that path, it MUST be replaced.

### Reader goroutine

The daemon MUST spawn a background reader for each session's trigger pipe.
The reader blocks on the FIFO and processes each line written to it as a
separate trigger.

GIVEN an external process writes "New email from alice@example.com" to trigger.pipe
WHEN the reader receives the line
THEN the agent is prompted with:
  "An external process sent a trigger. Read the content below and act on it.
   --- External trigger ---
   New email from alice@example.com
   --- end trigger ---"

### Trigger responses

The agent response is sent to the Matrix room associated with the session,
unless the response contains `HEARTBEAT_OK` (same suppression as heartbeat).

### Trigger typing

The daemon SHOULD send typing indicators during trigger processing.

### Non-idle triggers

Trigger prompts MUST NOT reset the session's `last_active` timestamp
(same as heartbeat — prevents triggers from keeping idle sessions alive).

### Trigger prompt customization

The trigger prompt prefix SHOULD be configurable via
`DaemonConfig.trigger_prompt: Option<String>`.

### Empty lines ignored

Blank lines written to the pipe MUST be silently ignored.

### Pipe persistence

The FIFO MUST persist across daemon restarts (it's a filesystem object).
The daemon MUST re-open existing FIFOs on startup for sessions that have
them.

### Reader cleanup

When a session is reaped (idle timeout), the reader goroutine for its
trigger pipe MUST be stopped and the FIFO removed.
