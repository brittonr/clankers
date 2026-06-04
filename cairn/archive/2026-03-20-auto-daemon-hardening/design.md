## Context

Auto-daemon mode is the default (`use_daemon=true`). When the user runs `clankers` interactively, it transparently starts a background daemon, creates a session, and attaches the TUI. The daemon handles the agent loop while the client handles rendering.

The current implementation has five gaps:
1. Session cleanup only runs in the happy path (after `run_attach_with_reconnect` returns). Signals and panics skip it.
2. Reconnection sets `ConnectionMode::Attached`, leaking the daemon abstraction.
3. Daemon crash is a dead end — the client retries the same dead socket 20 times then gives up.
4. `ensure_daemon_running` returns `Ok(())` when the process is alive but the socket never responds.
5. Concurrent `clankers` invocations can both spawn daemon processes.

## Goals / Non-Goals

**Goals:**
- Auto-daemon sessions never leak on normal exits, signals, or panics
- Daemon crashes are recovered transparently with session resume
- UX stays indistinguishable from in-process mode
- Daemon startup is reliable and coordinated

**Non-Goals:**
- Remote daemon crash recovery (iroh QUIC) — different failure domain
- Graceful degradation to in-process mode on daemon failure — too much complexity
- Multi-client session sharing — auto-daemon sessions are single-owner

## Decisions

### 1. Cleanup guard via Drop + atexit signal handler

Use a `SessionGuard` struct that holds the session ID and sends `KillSession` in its `Drop` impl. The guard lives on the stack in `run_auto_daemon_attach`, so it fires on normal return, early `?` return, and panic unwind.

For Ctrl+C/SIGTERM: install a `tokio::signal` handler that sets a flag, and check it in the event loop. The existing `app.should_quit` path handles graceful exit; the guard's Drop fires during stack unwind.

The Drop impl uses a synchronous blocking send (short-lived Unix socket connect + write) rather than async, since Drop can't be async. The control socket protocol is simple enough that a blocking send with a 500ms timeout is fine.

**Alternative considered**: `ctrlc` crate — rejected because we already use crossterm's raw mode which intercepts Ctrl+C as a key event. Crossterm's `Event::Key(KeyCode::Char('c'), KeyModifiers::CONTROL)` already reaches the event loop. The issue is when the event loop isn't running (e.g., during startup or after a panic).

**Alternative considered**: `atexit(3)` via `libc::atexit` — rejected because it requires a static function pointer with no context, and doesn't run on panic.

### 2. Pass ConnectionMode to reconnection logic

Add a `connection_mode` parameter (or capture the original mode) in `run_attach_with_reconnect`. On successful reconnect, restore the original mode instead of hardcoding `Attached`. For auto-daemon this is `Embedded`; for explicit attach it stays `Attached`.

### 3. Daemon crash recovery: restart + resume loop

Extend `try_reconnect` (or add a parallel `try_recover_daemon` path for auto-daemon) that:

1. Detects that reconnection to the existing socket failed (daemon dead, not just socket glitch)
2. Calls `ensure_daemon_running()` to restart
3. Sends `CreateSession` with `resume_id = original_session_id`
4. Connects to the new session socket
5. Replays history from the daemon's automerge recovery

This only applies to auto-daemon mode. Explicit attach keeps the existing behavior (retry socket, give up).

The caller passes a flag or enum (`RecoveryMode::AutoDaemon { session_id, model, ... }` vs `RecoveryMode::ExplicitAttach`) so `try_reconnect` knows whether to attempt daemon restart.

### 4. `ensure_daemon_running` returns Err on timeout

Change the "process alive but socket unresponsive" path from `Ok(())` with a warning to `Err(...)` with the log file path in the error message. Callers already handle errors from this function.

### 5. Lockfile for daemon startup

Use `flock(2)` (via `fs2::FileExt` or raw `libc::flock`) on a well-known file (`$XDG_RUNTIME_DIR/clankers/daemon.lock`) during the startup sequence:

1. Acquire exclusive lock (non-blocking attempt first)
2. If lock acquired: check PID, spawn if needed, release lock after socket is responsive
3. If lock not acquired: skip spawn, fall through to the polling loop that waits for the socket

`flock` is automatically released on process death, so a crashed starter doesn't hold the lock.

**Alternative considered**: PID file with advisory locking — rejected because PID files are already used for daemon detection and mixing concerns makes both less reliable. A separate lockfile is clearer.

## Risks / Trade-offs

- **[Blocking Drop]** The `SessionGuard::drop` does a synchronous socket connect + send. If the daemon is hung, this blocks for up to 500ms during process exit. Acceptable — the process is dying anyway. → Mitigation: 500ms timeout on the blocking connect.

- **[Resume race]** If daemon crashes and two auto-daemon clients both try to restart and resume, the second `CreateSession(resume_id)` may fail because the first already claimed the session. → Mitigation: The second client creates a fresh session and notifies the user.

- **[Lockfile stale]** If `flock` lock is held by a hung process (not dead, just stuck), other starters block. → Mitigation: Use non-blocking try-lock, fall through to socket polling if lock isn't acquired within 1s.
