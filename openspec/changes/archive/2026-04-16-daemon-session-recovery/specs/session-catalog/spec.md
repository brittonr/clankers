## ADDED Requirements

### Requirement: Persistent session index
The daemon SHALL maintain a redb-backed catalog of all sessions it manages. Each entry SHALL store the session ID, automerge file path, model name, creation timestamp, last-active timestamp, turn count, and lifecycle state (active, suspended, tombstoned).

#### Scenario: Session creation writes catalog entry
- **WHEN** a new session is created via control socket or QUIC
- **THEN** the catalog SHALL contain an entry for that session with state `active` and the correct automerge file path

#### Scenario: Session metadata updates on activity
- **WHEN** a session processes a prompt
- **THEN** the catalog entry's `last_active` timestamp and `turn_count` SHALL be updated

#### Scenario: Catalog survives daemon restart
- **WHEN** the daemon stops and restarts
- **THEN** the catalog SHALL contain all entries from the previous run with their last-known metadata

### Requirement: Session lifecycle states
Each catalog entry SHALL have a lifecycle state: `active` (actor running), `suspended` (checkpointed, no actor), or `tombstoned` (session killed or expired, pending GC).

#### Scenario: Active session
- **WHEN** a session has a running actor process
- **THEN** its catalog state SHALL be `active`

#### Scenario: Suspended on daemon shutdown
- **WHEN** the daemon shuts down gracefully
- **THEN** all `active` entries SHALL transition to `suspended`

#### Scenario: Tombstoned on kill
- **WHEN** a session is killed via `KillSession` command
- **THEN** its catalog state SHALL transition to `tombstoned`

#### Scenario: Tombstoned on idle expiry
- **WHEN** a session is reaped by the idle timeout reaper
- **THEN** its catalog state SHALL transition to `tombstoned`

### Requirement: Catalog garbage collection
The daemon SHALL periodically remove `tombstoned` entries older than a configurable retention period (default: 7 days). The underlying automerge file SHALL NOT be deleted — only the catalog entry is removed.

#### Scenario: Old tombstoned entry removed
- **WHEN** a tombstoned entry is older than the retention period
- **THEN** the catalog entry SHALL be removed on the next GC pass

#### Scenario: Automerge file preserved
- **WHEN** a catalog entry is garbage collected
- **THEN** the automerge session file SHALL remain on disk for manual inspection or resume

### Requirement: Key index persistence
The catalog SHALL persist the `SessionKey → session_id` mappings (iroh peer keys, Matrix user+room pairs) so that transport-keyed sessions survive restart.

#### Scenario: Matrix user reconnects after restart
- **WHEN** a Matrix user sends a message after daemon restart
- **THEN** the daemon SHALL route to the same session ID as before restart
