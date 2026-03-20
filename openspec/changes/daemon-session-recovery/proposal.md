## Why

When the daemon stops — crash, upgrade, or `daemon stop` — all session state evaporates. Conversation files exist on disk (automerge), but on restart the daemon has zero sessions and no way to revive them. Clients that reconnect find nothing. This makes the daemon feel disposable instead of durable, and blocks any serious multi-session or remote-access workflow where the daemon is expected to survive restarts.

## What Changes

- Persistent session catalog (redb) that tracks which sessions the daemon owns, their metadata, and their automerge file paths
- Session recovery on daemon startup: scan the catalog, rebuild `DaemonState` entries, lazy-spawn agent actors when a client attaches or a message arrives
- QUIC remote reconnect: fix the broken re-attach path so disconnected remote clients can reconnect to the same session after transient failures or daemon restart
- Session checkpoint on shutdown: flush in-flight state to the automerge file before the actor dies, so recovery has clean data
- `clankers daemon restart` command that drains active sessions, checkpoints, restarts the process, and revives sessions

## Capabilities

### New Capabilities
- `session-catalog`: Persistent redb-backed index of daemon-managed sessions with metadata, file paths, and lifecycle state
- `session-recovery`: Rebuild daemon sessions from the catalog + automerge files on startup, with lazy actor spawning
- `quic-reconnect`: Fix remote QUIC re-attach so disconnected clients can reconnect to the same session
- `graceful-restart`: Drain, checkpoint, restart, and revive sessions without losing conversation state

### Modified Capabilities

## Impact

- `src/modes/daemon/mod.rs` — startup sequence gains recovery phase, shutdown gains checkpoint phase
- `src/modes/daemon/agent_process.rs` — checkpoint-on-shutdown, resume-from-file actor spawning
- `src/modes/daemon/session_store.rs` — new session catalog (redb tables alongside auth tables)
- `src/modes/daemon/quic_bridge.rs` — fix `handle_attach_stream` reconnect path
- `src/modes/attach.rs` — QUIC reconnect retry logic
- `crates/clankers-controller/src/transport.rs` — `DaemonState` gains catalog sync, `SessionHandle` gains recovery metadata
- `crates/clankers-session/src/lib.rs` — `SessionManager::open()` path for rehydrating from existing files
- `crates/clankers-protocol/` — new `ControlCommand::RestartDaemon`, updated `SessionSummary` with recovery status
