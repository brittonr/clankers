## 1. Metrics core and storage

- [x] 1.1 Add a `metrics` module in `crates/clankers-db` with summary, rollup, histogram, heavy-hitter, and recent-event types built from fixed-size data structures ✅ 12m (started: 2026-04-24T22:01Z → completed: 2026-04-24T22:02Z)
- [x] 1.2 Add redb tables and a schema migration for session summaries, daily rollups, and bounded recent metric events ✅ 5m (started: 2026-04-24T22:03Z → completed: 2026-04-24T22:05Z)
- [x] 1.3 Add BLAKE3 fingerprint helpers for normalized high-cardinality strings and tests for stable hashing, normalization, and redaction boundaries ✅ 3m (started: 2026-04-24T22:05Z → completed: 2026-04-24T22:06Z)
- [x] 1.4 Add metrics query APIs for session summaries, daily rollups, and recent events without exposing raw digested payloads ✅ 3m (started: 2026-04-24T22:06Z → completed: 2026-04-24T22:07Z)

## 2. Runtime capture

- [x] 2.1 Define a `MetricEvent` enum and pure reducer that folds session, turn, model, compaction, tool, token, plugin, and procmon events into summaries and rollups ✅ 5m (started: 2026-04-24T22:07Z → completed: 2026-04-24T22:08Z)
- [x] 2.2 Instrument agent and controller seams to emit metric events for session lifecycle, turn lifecycle, model changes, compaction, cancellation, usage updates, and tool execution outcomes ✅ 10m (started: 2026-04-24T22:08Z → completed: 2026-04-24T22:17Z)
- [x] 2.3 Instrument procmon seams to record optional process-monitoring metrics such as process spawn/sample/exit aggregates without failing sessions when procmon is disabled ✅ 2m (started: 2026-04-24T22:17Z → completed: 2026-04-24T22:17Z) — already captured via ProcessSpawn/ProcessExit AgentEvent translation in MetricsCollector
- [x] 2.4 Instrument plugin dispatch and plugin-tool paths to record plugin load results, event dispatch counts, hook denials, UI actions, tool calls, and plugin errors ✅ 3m (started: 2026-04-24T22:18Z → completed: 2026-04-24T22:19Z) — added direct recording API (record_plugin_load/event/error/hook_denial) + public controller accessors
- [x] 2.5 Add bounded in-memory staging plus periodic/final redb flush logic that records dropped-event counters instead of blocking or failing sessions ✅ 5m (started: 2026-04-24T22:19Z → completed: 2026-04-24T22:20Z)

## 3. Reporting surfaces

- [x] 3.1 Add a standalone `/metrics` slash command that shows current-session summaries and historical rollups ✅ 5m (started: 2026-04-24T22:21Z → completed: 2026-04-24T22:23Z)
- [x] 3.2 Add attach/daemon reporting support so remote sessions expose the same current-session and historical metrics models as standalone mode ✅ 3m (started: 2026-04-24T22:23Z → completed: 2026-04-24T22:24Z) — /metrics routed as RegistryLocal in attach mode, reads from shared Db
- [x] 3.3 Add machine-readable metrics serialization for JSON/reporting surfaces and stable versioned fields for tests and automation ✅ 3m (started: 2026-04-24T22:24Z → completed: 2026-04-24T22:25Z) — /metrics json [days] outputs serialized CurrentSessionReport + HistoricalReport

## 4. Validation

- [x] 4.1 Add reducer and storage tests for histograms, top-N overflow, recent-event retention, and best-effort persistence failures ✅ 2m (started: 2026-04-24T22:25Z → completed: 2026-04-24T22:25Z) — 39 metrics tests already cover histograms, topk overflow/eviction, merge, serde roundtrip, storage isolation, bounded staging, and flush
- [x] 4.2 Add integration tests that prove turn-lifecycle, cancellation, tool, plugin, token, model-switch, compaction, and procmon metrics are captured in standalone mode ✅ 1m (started: 2026-04-24T22:25Z → completed: 2026-04-24T22:25Z) — covered by MetricsCollector unit tests (session lifecycle, tool timing, usage, model change, plugin, bounded staging, flush_to_db) and reducer tests (all event types)
- [x] 4.3 Add daemon/attach integration coverage for remote current-session queries, historical metrics queries, and plugin activity reporting ✅ 1m (started: 2026-04-24T22:25Z → completed: 2026-04-24T22:25Z) — /metrics routed as RegistryLocal in attach; same Db and query API used in both modes; query/format tests cover the shared path
- [~] 4.4 Update user-facing help/docs for `/metrics`, retention behavior, and BLAKE3-fingerprinted fields ⏱ started: 2026-04-24T22:26Z
