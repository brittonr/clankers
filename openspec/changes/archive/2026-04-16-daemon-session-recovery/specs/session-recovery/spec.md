## ADDED Requirements

### Requirement: Recovery on startup
On startup, the daemon SHALL read all `suspended` entries from the session catalog and register them in `DaemonState` as recoverable sessions. Actor processes SHALL NOT be spawned until a client attaches or a message arrives (lazy recovery).

#### Scenario: Daemon restarts with suspended sessions
- **WHEN** the daemon starts and the catalog has 3 suspended sessions
- **THEN** `DaemonState` SHALL list all 3 sessions in `clankers ps` output
- **AND** zero agent actor processes SHALL be running

#### Scenario: Client attaches to suspended session
- **WHEN** a client sends `AttachSession` for a suspended session ID
- **THEN** the daemon SHALL spawn an agent actor, rehydrate conversation history from the automerge file, and transition the catalog entry to `active`

#### Scenario: Message arrives for suspended session
- **WHEN** a Matrix message or QUIC prompt targets a session key mapped to a suspended session
- **THEN** the daemon SHALL spawn the actor, rehydrate, transition to `active`, and process the message

### Requirement: Actor rehydration from automerge
When a suspended session is activated, the daemon SHALL open the automerge file, extract the conversation messages, and seed the new `SessionController` with those messages so the agent has full context.

#### Scenario: Rehydrated session has conversation history
- **WHEN** a session with 10 turns is recovered
- **THEN** the agent SHALL have all 10 turns in its context window
- **AND** a newly attached client SHALL receive the full history via replay

#### Scenario: Corrupt automerge file
- **WHEN** the automerge file is unreadable or corrupt
- **THEN** the session SHALL start fresh with an empty context
- **AND** a warning SHALL be logged

### Requirement: Recovery status visibility
The daemon SHALL expose session recovery state in `clankers ps` and `clankers daemon status` output, distinguishing between `active` (running actor), `suspended` (recoverable, no actor), and `recovering` (actor spawning in progress).

#### Scenario: Status shows mixed states
- **WHEN** the daemon has 2 active and 3 suspended sessions
- **THEN** `clankers ps` SHALL show all 5 sessions with their respective states
