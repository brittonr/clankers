## 1. System Prompt

- [x] 1.1 Replace `HEARTBEAT_SECTION` with `SCHEDULE_SECTION` in `system_prompt.rs` — describe the schedule tool API (actions, kinds, payload format, examples)
- [x] 1.2 Update system prompt tests: daemon_mode=true produces schedule guidance, no "HEARTBEAT" references anywhere

## 2. Persistence in clanker-scheduler

- [x] 2.1 Add `with_persistence(path: PathBuf)` builder method to `ScheduleEngine` — stores an `Option<PathBuf>`
- [x] 2.2 Add `fn save(&self)` — serialize active schedules to the persistence path (skip if None)
- [x] 2.3 Add `fn load(path: &Path) -> Vec<Schedule>` — read and deserialize, discard Expired, warn on corrupt/missing
- [x] 2.4 Call `save()` after mutating operations: `add`, `remove`, `pause`, `resume`, `record_fire` (in tick loop after fire batch)
- [x] 2.5 Add unit tests for save/load roundtrip, expired filtering, missing file, corrupt file

## 3. Wire Persistence in Clankers

- [x] 3.1 In `src/modes/interactive.rs`: create engine with `with_persistence(schedules_path)`, load existing schedules on startup
- [x] 3.2 In `src/commands/daemon.rs`: create engine with `with_persistence(schedules_path)`, load existing schedules on startup
- [x] 3.3 Resolve schedules path via `ClankersPaths` — `~/.clankers/agent/schedules.json`

## 4. Integration Tests

- [x] 4.1 Tool CRUD test: create all three schedule kinds, list, pause, resume, delete, info, error cases
- [x] 4.2 Fire-to-event test: create engine with fast tick, add schedule, assert broadcast receiver gets ScheduleEvent
- [x] 4.3 No-prompt-field test: fire schedule without `prompt` key, verify no prompt injected
- [x] 4.4 Max-fires expiry test: schedule with max_fires=2, tick twice, verify expired
- [x] 4.5 Persistence roundtrip test: add schedules, save, load into new engine, verify match
