# Scheduler and Loop Crate Extraction

## Purpose

Extract `clankers-scheduler` and `clankers-loop` into standalone crates.
Both are generic task execution engines with zero workspace dependencies
and standard tokio/chrono/serde deps.

They could ship as two crates or merge into one. This spec covers both
and defers that decision to implementation.

## Requirements

### Scheduler crate

r[scheduler.identity.name]
The extracted crate MUST be named `cron-tick` (or chosen alternative).

r[scheduler.identity.repo]
The crate MUST live in its own GitHub repository.

r[scheduler.source.files]
The following files MUST be moved:

- `src/lib.rs` — module declarations and re-exports
- `src/schedule.rs` — `Schedule`, `ScheduleId`, `ScheduleKind`, `ScheduleStatus`
- `src/engine.rs` — `ScheduleEngine`, `ScheduleEvent`
- `src/cron.rs` — cron pattern parsing and matching

r[scheduler.source.no-clankers-refs]
The source MUST NOT contain the string "clankers".

r[scheduler.api.engine]
The crate MUST export `ScheduleEngine` with:
- `add(schedule) -> ScheduleId`
- `remove(id)`
- `pause(id)` / `resume(id)`
- `tick()` — check all schedules, emit events for those that fired
- `subscribe() -> broadcast::Receiver<ScheduleEvent>`

r[scheduler.api.kinds]
The crate MUST export schedule kinds:
- `Once` — fires at a specific datetime
- `Interval` — fires every N seconds/minutes/hours
- `Cron` — fires on minute/hour/day-of-week patterns

r[scheduler.api.payload]
Schedules MUST carry an arbitrary `serde_json::Value` payload that the
consumer interprets. The engine does not inspect the payload.

r[scheduler.tests.existing]
All existing tests MUST pass in the extracted crate.

r[scheduler.migration.re-export]
After extraction, `crates/clankers-scheduler/` MUST re-export the
extracted crate via git dep. The one caller (`src/tools/schedule.rs`)
MUST compile unchanged.

### Loop crate

r[loop.identity.name]
The extracted crate MUST be named `iter-engine` (or chosen alternative).

r[loop.identity.repo]
The crate MUST live in its own GitHub repository (or shared with scheduler
if merged).

r[loop.source.files]
The following files MUST be moved:

- `src/lib.rs` — module declarations and re-exports
- `src/iteration.rs` — `LoopDef`, `LoopId`, `LoopKind`, `LoopState`, `LoopStatus`
- `src/engine.rs` — `LoopEngine`, `LoopEvent`
- `src/condition.rs` — `BreakCondition`, `parse_break_condition`
- `src/truncation.rs` — `OutputTruncationConfig`, `truncate_tool_output`

r[loop.source.no-clankers-refs]
The source MUST NOT contain the string "clankers". The one doc comment
in `truncation.rs` that references clankers MUST be rewritten.

r[loop.api.engine]
The crate MUST export `LoopEngine` with:
- `start(def) -> LoopId`
- `cancel(id)`
- `check_condition(id, output) -> bool` — test break condition against output
- `status(id) -> LoopStatus`
- `active_count() -> usize`

r[loop.api.kinds]
The crate MUST export loop kinds:
- `Fixed` — run N iterations
- `Until` — run until a condition matches output
- `Poll` — run at intervals until condition or timeout

r[loop.api.truncation]
The crate MUST export truncation utilities:
- `truncate_tool_output(output, config) -> TruncationResult`
- `cleanup_temp_files()`

r[loop.api.bounds]
The crate MUST export the safety constants:
- `MAX_ACTIVE_LOOPS`
- `MAX_ITERATIONS_HARD_LIMIT`

r[loop.tests.existing]
All existing tests MUST pass in the extracted crate.

r[loop.migration.re-export]
After extraction, `crates/clankers-loop/` MUST re-export the extracted
crate via git dep. All 7 call sites MUST compile unchanged.

### Shared requirements

r[shared.migration.workspace-builds]
`cargo check` and `cargo nextest run` MUST pass on the full clankers
workspace after each extraction.
