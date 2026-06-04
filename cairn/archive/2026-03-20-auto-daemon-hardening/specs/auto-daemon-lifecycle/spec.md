## ADDED Requirements

### Requirement: Session cleanup on process exit
The auto-daemon client SHALL kill its session when the process exits, whether via normal quit, Ctrl+C (SIGINT), SIGTERM, or panic unwind. The `KillSession` command MUST be sent to the daemon control socket before the process terminates.

#### Scenario: Normal quit
- **WHEN** the user quits the TUI (`:q`, `/quit`, or `Esc` from normal mode)
- **THEN** the auto-daemon session is killed via `KillSession` before process exit

#### Scenario: Ctrl+C during active session
- **WHEN** the user presses Ctrl+C while the auto-daemon TUI is running
- **THEN** the session is killed via `KillSession` and the terminal is restored

#### Scenario: Panic unwind
- **WHEN** the auto-daemon client panics during execution
- **THEN** the session is killed via best-effort synchronous `KillSession` during drop

#### Scenario: SIGKILL (non-recoverable)
- **WHEN** the process receives SIGKILL
- **THEN** no cleanup runs (expected), and the daemon's hourly GC SHALL eventually tombstone the orphan

### Requirement: Connection mode stays Embedded
The auto-daemon client SHALL maintain `ConnectionMode::Embedded` throughout its entire lifecycle, including after reconnection. The "ATTACHED" badge MUST NOT appear for auto-daemon sessions.

#### Scenario: Initial connection
- **WHEN** auto-daemon connects to the daemon session
- **THEN** `ConnectionMode` is `Embedded` and no badge is shown

#### Scenario: Reconnection after transient disconnect
- **WHEN** the auto-daemon client loses and reestablishes the Unix socket connection
- **THEN** `ConnectionMode` returns to `Embedded` (not `Attached`) and no badge is shown

### Requirement: Daemon crash recovery
When the daemon crashes during an auto-daemon session, the client SHALL attempt to restart the daemon, create a new session resuming from the automerge checkpoint, and reconnect transparently.

#### Scenario: Daemon crash mid-session
- **WHEN** the daemon process dies (SIGKILL, OOM, bug) while an auto-daemon session is active
- **THEN** the client detects the disconnect, restarts the daemon via `ensure_daemon_running`, creates a new session with the original session's ID as `resume_id`, and reconnects

#### Scenario: Daemon restart fails
- **WHEN** the daemon cannot be restarted (e.g., port conflict, binary missing)
- **THEN** the client displays an error and allows the user to `/quit`

#### Scenario: Daemon restart succeeds but resume fails
- **WHEN** the daemon restarts but the session cannot be resumed (e.g., automerge file corrupted)
- **THEN** the client creates a fresh session and notifies the user that history was lost

### Requirement: Daemon startup reliability
`ensure_daemon_running` SHALL return an error when the daemon socket does not become responsive within the timeout period, even if the daemon process is alive.

#### Scenario: Daemon process alive but socket unresponsive
- **WHEN** `ensure_daemon_running` spawns a daemon that stays alive but never binds the control socket within 5 seconds
- **THEN** an error is returned with a message pointing the user to the daemon log file

#### Scenario: Daemon process dies during startup
- **WHEN** the spawned daemon process exits before the socket becomes responsive
- **THEN** an error is returned indicating startup failure with the log file path

### Requirement: Concurrent startup coordination
Multiple simultaneous `clankers` invocations SHALL coordinate daemon startup so that exactly one daemon process is started.

#### Scenario: Two simultaneous starts with no daemon running
- **WHEN** two `clankers` invocations both call `ensure_daemon_running` concurrently with no existing daemon
- **THEN** exactly one daemon process is spawned, and both invocations connect to it

#### Scenario: Start while daemon is already starting
- **WHEN** a second `clankers` invocation runs while the first is still waiting for the daemon socket
- **THEN** the second invocation detects the in-progress startup (via lockfile) and waits for the socket instead of spawning a second daemon
