## ADDED Requirements

### Requirement: Schedules persist to disk
The ScheduleEngine SHALL save all schedules to a JSON file after every mutating operation (add, remove, pause, resume).

#### Scenario: Save after add
- **WHEN** a schedule is added via `engine.add(schedule)`
- **THEN** the schedules JSON file contains the new schedule

#### Scenario: Save after remove
- **WHEN** a schedule is removed via `engine.remove(id)`
- **THEN** the schedules JSON file no longer contains that schedule

#### Scenario: Save after pause
- **WHEN** a schedule is paused via `engine.pause(id)`
- **THEN** the schedules JSON file shows the schedule with status `Paused`

#### Scenario: Save after resume
- **WHEN** a schedule is resumed via `engine.resume(id)`
- **THEN** the schedules JSON file shows the schedule with status `Active`

### Requirement: Schedules load on startup
The engine SHALL load schedules from disk when initialized with a persistence path. Expired one-shot schedules SHALL be discarded during load.

#### Scenario: Reload after restart
- **WHEN** a daemon starts with existing `schedules.json` containing an active interval schedule
- **THEN** the engine's schedule list contains that schedule with its original ID, name, and fire count

#### Scenario: Expired schedules discarded on load
- **WHEN** `schedules.json` contains a schedule with status `Expired`
- **THEN** the engine does not load that schedule

#### Scenario: Missing file treated as empty
- **WHEN** no `schedules.json` exists at the configured path
- **THEN** the engine starts with zero schedules and no error

#### Scenario: Corrupt file logged and treated as empty
- **WHEN** `schedules.json` contains invalid JSON
- **THEN** the engine logs a warning and starts with zero schedules

### Requirement: Persistence path configured by caller
The ScheduleEngine SHALL accept an optional file path for persistence. When no path is set, the engine operates in memory-only mode (current behavior).

#### Scenario: No persistence path
- **WHEN** `ScheduleEngine::new()` is called without a persistence path
- **THEN** mutating operations do not write to disk

#### Scenario: With persistence path
- **WHEN** `ScheduleEngine::new().with_persistence(path)` is called
- **THEN** mutating operations write to the specified path

### Requirement: System prompt describes schedule tool
The system prompt SHALL include a section describing the schedule tool's API when running in daemon mode. The stale HEARTBEAT.md section SHALL be removed.

#### Scenario: Daemon mode prompt includes schedule section
- **WHEN** `PromptFeatures { daemon_mode: true }` is used
- **THEN** the system prompt contains schedule tool usage guidance
- **THEN** the system prompt does not contain "HEARTBEAT.md"

#### Scenario: Non-daemon mode excludes schedule section
- **WHEN** `PromptFeatures { daemon_mode: false }` is used
- **THEN** the system prompt does not contain schedule tool guidance
