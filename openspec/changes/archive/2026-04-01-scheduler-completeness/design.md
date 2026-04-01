## Context

The `clanker-scheduler` crate provides a `ScheduleEngine` with in-memory schedule storage, a background tick loop, and broadcast event dispatch. The `ScheduleTool` wraps it as an agent tool. Both standalone (interactive) and daemon modes wire up the engine and consume fired events by injecting prompts.

Current gaps: the agent has no system prompt guidance for the tool, there are no integration tests, and schedules are lost on process restart.

## Goals / Non-Goals

**Goals:**
- Agent knows how to use the `schedule` tool (system prompt section).
- Stale HEARTBEAT.md reference removed.
- Schedules survive daemon restart (JSON persistence to `~/.clankers/agent/schedules.json`).
- Integration tests cover tool CRUD and the fire→prompt injection path.

**Non-Goals:**
- Schedule UI panel in the TUI — list via tool is sufficient for now.
- Cross-session schedule sharing — each schedule belongs to one session.
- Schedule editing (update-in-place) — delete + recreate is fine.
- Migrating to a database — JSON file is adequate for the expected scale (dozens of schedules, not thousands).

## Decisions

### 1. Persistence format: JSON file

Store schedules as a JSON array in `~/.clankers/agent/schedules.json`. The `ScheduleEngine` already has `Schedule` types that derive `Serialize`/`Deserialize`.

Alternative considered: SQLite via the existing `clankers-db` crate. Rejected — adds coupling for a simple flat list. A JSON file can be edited by hand and inspected trivially.

### 2. Persistence location: in clanker-scheduler crate

Add `save`/`load` functions to the `clanker-scheduler` crate itself rather than scattering persistence logic across interactive.rs and daemon/mod.rs. The engine owns the schedules; it should own their serialization.

The caller passes a `Path` — the engine doesn't decide where to store. This keeps the crate independent of clankers path conventions.

### 3. Save on mutation, load on startup

Every mutating operation (add, remove, pause, resume, record_fire) saves the full schedule set. This is simple and the data is tiny. On startup, the caller loads from disk and feeds schedules into the engine.

Alternative considered: periodic flush. Rejected — adds a window where a crash loses recent changes, and the write volume is negligible.

### 4. System prompt: replace HEARTBEAT section

The `HEARTBEAT_SECTION` const becomes a `SCHEDULE_SECTION` that describes the tool's API and usage patterns. The `PromptFeatures::daemon_mode` flag already gates this section, which is correct — schedules are only useful in long-running sessions.

### 5. Integration tests: in-process with fast tick

Tests create a `ScheduleEngine` with a 10ms tick interval, add schedules, and assert that the broadcast receiver gets the expected events. For tool-level tests, construct a `ScheduleTool` directly and call `execute()`. No need for full daemon startup.

## Risks / Trade-offs

- [Concurrent writes] Multiple sessions could write `schedules.json` simultaneously in daemon mode. → Mitigation: the engine is shared (single `Arc<ScheduleEngine>` per daemon), so mutations are serialized by the `parking_lot::Mutex`. File writes happen under the same lock.

- [Stale schedules after crash] If the process crashes between `record_fire` and save, a schedule might re-fire on restart. → Acceptable for the current use case. Idempotent prompts are the norm.

- [External crate change] Adding save/load to `clanker-scheduler` means a commit + version bump there. → Fine — it's a first-party crate under the same author.
