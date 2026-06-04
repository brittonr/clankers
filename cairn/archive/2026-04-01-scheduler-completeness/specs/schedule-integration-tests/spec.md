## ADDED Requirements

### Requirement: Tool CRUD operations tested
Integration tests SHALL verify all ScheduleTool actions return correct results.

#### Scenario: Create interval schedule
- **WHEN** the tool is called with `action: "create", kind: "interval", interval: "5m", name: "test"`
- **THEN** the result contains the schedule name and ID
- **THEN** a subsequent `action: "list"` includes the schedule

#### Scenario: Create once schedule with relative time
- **WHEN** the tool is called with `action: "create", kind: "once", at: "+1h", name: "reminder"`
- **THEN** the result contains the schedule name and ID

#### Scenario: Create cron schedule
- **WHEN** the tool is called with `action: "create", kind: "cron", cron: "0 9 1-5", name: "standup"`
- **THEN** the result contains the schedule name and ID

#### Scenario: Pause and resume
- **WHEN** a schedule is created and then paused via `action: "pause", id: "<id>"`
- **THEN** `action: "info"` shows status Paused
- **WHEN** resumed via `action: "resume", id: "<id>"`
- **THEN** `action: "info"` shows status Active

#### Scenario: Delete schedule
- **WHEN** a schedule is deleted via `action: "delete", id: "<id>"`
- **THEN** `action: "list"` does not include the schedule
- **THEN** `action: "info"` returns an error for that ID

#### Scenario: Invalid action rejected
- **WHEN** the tool is called with `action: "invalid"`
- **THEN** the result is an error

#### Scenario: Missing required params rejected
- **WHEN** `action: "create", kind: "once"` is called without `at`
- **THEN** the result is an error mentioning the missing parameter

### Requirement: Fire-to-prompt flow tested
Integration tests SHALL verify that a fired schedule injects a prompt into the agent command channel.

#### Scenario: Interval schedule fires and injects prompt
- **WHEN** an interval schedule with `payload: {"prompt": "check status"}` fires
- **THEN** the broadcast receiver receives a `ScheduleEvent` with the correct payload
- **THEN** the event's `schedule_name` and `fire_count` are correct

#### Scenario: Schedule without prompt field is skipped
- **WHEN** a schedule fires with `payload: {"command": "ls"}` (no `prompt` key)
- **THEN** no `AgentCommand::Prompt` is sent (the event is logged and dropped)

#### Scenario: Max-fires causes expiry
- **WHEN** a schedule with `max_fires: 2` fires twice
- **THEN** the schedule status becomes Expired
- **THEN** no further events are emitted on subsequent ticks

### Requirement: Persistence roundtrip tested
Integration tests SHALL verify schedules survive save/load cycles.

#### Scenario: Save and reload
- **WHEN** schedules are added to a persistent engine, then a new engine loads from the same path
- **THEN** the new engine contains the same schedules with matching IDs, names, and fire counts

#### Scenario: Expired schedules not reloaded
- **WHEN** a one-shot schedule fires (becoming Expired), the engine saves, and a new engine loads
- **THEN** the expired schedule is not present in the new engine
