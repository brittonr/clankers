# Idle Session Reaping

## Purpose

Free resources for sessions that haven't been used in a while.  The daemon
currently evicts by count (max_sessions) but not by time.

## Requirements

### Configurable idle timeout

The daemon MUST support an idle timeout after which inactive sessions are
reaped.  Default: 30 minutes.

Configuration via `DaemonConfig.idle_timeout: Duration`.

### Reaper tick

The daemon MUST run a background task that checks session `last_active`
timestamps periodically (every 60 seconds) and removes sessions that
exceed the idle timeout.

GIVEN a session's last_active is 31 minutes ago
WHEN the reaper ticks
THEN the session is removed from the SessionStore
AND its prompt_lock is removed

### Reaping preserves session persistence

Session reaping MUST NOT delete persisted session data (JSONL files).
A reaped session can be restored if the same user sends a new message
(get_or_create will create a fresh agent, and history can be reloaded
from disk if session resume is implemented).

### Log on reap

The daemon MUST log at `info` level when a session is reaped, including
the session key and how long it was idle.
