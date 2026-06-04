## ADDED Requirements

### Requirement: Checkpoint on shutdown
When the daemon receives a shutdown signal (SIGINT, `daemon stop`, or `RestartDaemon`), it SHALL flush all active sessions: complete any in-flight tool executions, persist pending messages to the automerge file, and transition catalog entries from `active` to `suspended`.

#### Scenario: Clean shutdown with active sessions
- **WHEN** the daemon receives SIGINT with 3 active sessions
- **THEN** all 3 sessions SHALL have their latest state persisted to automerge files
- **AND** all 3 catalog entries SHALL be `suspended`

#### Scenario: In-flight tool execution
- **WHEN** a session is executing a bash tool during shutdown
- **THEN** the daemon SHALL wait up to the drain timeout (default: 10s) for the tool to complete
- **AND** persist whatever state exists at the timeout boundary

### Requirement: Restart command
The daemon SHALL support a `RestartDaemon` control command that performs checkpoint → stop → re-exec. The new process SHALL recover all suspended sessions.

#### Scenario: Restart via CLI
- **WHEN** a user runs `clankers daemon restart`
- **THEN** the daemon SHALL checkpoint all sessions, exit, and the CLI SHALL start a new daemon process
- **AND** `clankers ps` on the new daemon SHALL list all previously active sessions as `suspended`

#### Scenario: Restart preserves remote connections
- **WHEN** a remote QUIC client is connected during restart
- **THEN** the client's QUIC stream SHALL close
- **AND** the client's reconnect logic SHALL re-attach to the recovered session on the new daemon

### Requirement: Drain timeout
The shutdown drain phase SHALL have a configurable timeout (default: 10 seconds). Sessions that do not complete within the timeout SHALL be force-checkpointed with their current state.

#### Scenario: Session completes within timeout
- **WHEN** a session finishes its current turn within the drain timeout
- **THEN** the full turn SHALL be persisted

#### Scenario: Session exceeds drain timeout
- **WHEN** a session is still processing after the drain timeout
- **THEN** the session SHALL be force-stopped
- **AND** partial state (messages persisted so far) SHALL be saved
- **AND** the catalog entry SHALL be `suspended`
