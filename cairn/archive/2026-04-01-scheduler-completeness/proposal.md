## Why

The scheduler engine and tool exist but are half-wired. The agent doesn't know the tool exists (no system prompt section), there are no integration tests for the fire→prompt flow, schedules vanish on restart (no persistence), and the HEARTBEAT system prompt section describes a file-based approach that doesn't match reality.

## What Changes

- Add a system prompt section describing the `schedule` tool so the agent knows how to use it.
- Replace the stale HEARTBEAT.md system prompt section with accurate schedule tool guidance.
- Add integration tests covering: tool CRUD, fire→prompt injection (standalone), fire→session routing (daemon).
- Add schedule persistence — save to disk on mutation, reload on engine startup.

## Capabilities

### New Capabilities
- `schedule-persistence`: Save and reload schedules across daemon restarts.
- `schedule-integration-tests`: End-to-end tests for the schedule tool and fire→prompt flow.

### Modified Capabilities

## Impact

- `crates/clankers-agent/src/system_prompt.rs` — replace HEARTBEAT section, add schedule tool section
- `src/tools/schedule.rs` — persistence hooks on create/delete/pause/resume
- `clanker-scheduler` (external crate) — add save/load API
- `src/modes/interactive.rs` — load persisted schedules on startup
- `src/modes/daemon/mod.rs` — load persisted schedules on startup
- `tests/` — new integration test files
