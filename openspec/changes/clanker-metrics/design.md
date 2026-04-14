## Context

Clankers already has useful fragments of observability:

- `clankers-db::audit` records tool invocations with parameters, result previews, and duration.
- `clankers-db::usage` records daily token totals by model.
- `clankers-db::skill_usage` records skill loads and outcomes.
- `AgentEvent` already exposes session, turn, tool, model, compaction, usage, and procmon signals.
- Plugin activity is visible at runtime, but mostly through logs and ad hoc event forwarding.

What is missing is a bounded metrics model that spans the whole session. Today there is no single answer for:

- Which tools cost the most latency in a session?
- Which plugins add value and which mostly emit noise, errors, or hook denials?
- How many tokens and dollars each model consumed before and after a model switch?
- Whether compaction, branching, or cancellation correlate with failure?
- Which repeated bash commands, file paths, or plugin payload shapes show up often without storing their raw text again?

This feature cuts across `clankers-agent`, `clankers-controller`, `clankers-db`, daemon mode, attach mode, and plugin dispatch. It also changes the persistence model, so it needs a design doc.

## Goals / Non-Goals

**Goals:**
- Capture one bounded metrics stream for standalone and daemon sessions.
- Record tool, plugin, token, model, session, compaction, and procmon metrics with enough detail to debug cost, latency, and failure patterns.
- Persist metrics in redb with explicit retention limits and fixed-size aggregates.
- Use BLAKE3 to fingerprint high-cardinality or sensitive strings before they enter metrics storage.
- Keep the design Tiger Style: deterministic reducer core, imperative capture shell, explicit limits, fixed histograms, and no unbounded cardinality.
- Make current-session and historical metrics queryable from the TUI and attach mode.

**Non-Goals:**
- Prometheus, OTLP, or other external metrics exporters.
- Replacing the existing audit log, usage table, or skill-usage table.
- Storing raw prompt text, raw bash commands, raw file paths, or raw plugin payloads in metrics tables.
- Unbounded per-event time series or arbitrary label maps.
- Per-token tracing or live sampling beyond the events Clankers already emits.

## Decisions

### 1. Build a pure metrics reducer and keep capture/persistence in the shell

**Choice:** Runtime code translates `AgentEvent`, controller events, plugin dispatch results, and slash-command/report requests into a compact `MetricEvent` enum. A pure reducer folds `MetricEvent` values into fixed-size `SessionMetricsSummary` and `DailyMetricsRollup` structs. Persistence code writes those structs to redb.

**Rationale:** This matches Tiger Style. The reducer stays deterministic and testable because it does no I/O, reads no clocks, and allocates no unbounded maps. The shell injects timestamps, session IDs, model IDs, and plugin/tool metadata, then handles batching and redb writes.

**Alternative:** Write directly to redb from every event hook. Rejected because it couples runtime code to storage, makes tests noisy, and increases the chance of blocking hot paths.

### 2. Keep low-cardinality labels raw, fingerprint high-cardinality labels with BLAKE3

**Choice:** Store safe low-cardinality labels as plain strings: tool name, plugin name, model ID, transport, outcome, event kind, and hook kind. Normalize then BLAKE3-hash high-cardinality or sensitive strings before storage: cwd/worktree paths, prompt fingerprints, bash commands, tool input JSON, read/write/edit paths, plugin payloads, and normalized error text.

The stored form is a fixed-size digest plus byte length and kind, not the raw string.

**Rationale:** Operators need readable tool/plugin/model names, but raw commands and paths would explode cardinality and duplicate sensitive data already present in transcripts or audit logs. BLAKE3 gives a fast fixed-size fingerprint for grouping repeated shapes without keeping the original value.

**Alternative:** Store all fields raw and rely on redaction later. Rejected because the storage cost and privacy risk are front-loaded. Once raw strings land in metrics tables, they are hard to reason about.

### 3. Persist three views in redb: session summary, daily rollup, recent-event log

**Choice:** Add three metrics tables:

- `metrics_session_summary`: `session_id -> SessionMetricsSummary`
- `metrics_daily_rollup`: `YYYY-MM-DD -> DailyMetricsRollup`
- `metrics_recent_events`: `{session_id}:{seq}` -> `MetricEventRecord`

The session summary is the main query surface. The daily rollup supports history and trends. The recent-event log keeps the last bounded slice of raw metric events for debugging without needing to join audit, usage, and logs every time.

**Rationale:** This split keeps reads fast and bounded. Most UI reads hit the session summary. Historical dashboards hit the daily rollup. Raw recent events stay available for "what just happened?" questions.

**Alternative:** Store only raw events and rebuild summaries on demand. Rejected because attach mode and slash commands need cheap reads, and on-demand joins across many event types would make the reporting path much heavier.

### 4. Use fixed histograms and bounded top-N maps

**Choice:** Store latency and size distributions in fixed histograms with compile-time bucket counts. Store top tools, plugins, models, error digests, and command/path digests in capped heavy-hitter maps with explicit `other_count` overflow fields.

Likely structures:
- `LatencyHistogram<[u64; N]>` for tool, turn, and plugin dispatch latency
- `SizeHistogram<[u64; N]>` for streamed result bytes or payload size
- capped `TopKCounter` maps for tools, plugins, models, and digests

**Rationale:** Fixed structures keep memory and serialized size predictable. Queries stay cheap. Tiger Style wants explicit bounds, not open-ended maps keyed by whatever the agent happened to do.

**Alternative:** Plain `HashMap<String, u64>` everywhere. Rejected because tool inputs, paths, and error strings would create unbounded growth.

### 5. Make recent-event retention explicit and lossy by design

**Choice:** Recent metric events are capped per session and dropped in batches when the cap is exceeded. The summary tracks `recent_events_stored` and `recent_events_dropped` so operators can see when truncation happened.

A reasonable starting shape is:
- `MAX_RECENT_EVENTS_PER_SESSION`
- batch drop of the oldest 10% when full
- separate counters for dropped-on-overflow and dropped-on-write-failure

**Rationale:** Raw recent events are for local debugging, not long-term truth. The truth lives in the summary and rollup. Bounded retention avoids accidental time-series storage hidden inside redb.

**Alternative:** Keep all metric events forever. Rejected because that recreates an unbounded event store inside the local agent database.

### 6. Reuse existing audit and usage stores instead of replacing them

**Choice:** Keep `audit`, `usage`, and `skill_usage` tables. The new metrics layer derives its own summaries from the same runtime facts and may link back by `session_id`, `call_id`, model ID, or day.

**Rationale:** Existing user features like `/usage` and audit review should keep working while metrics grows. This also lowers migration risk: old data stays readable, new data starts fresh.

**Alternative:** Fold audit and usage into one new table family. Rejected because it forces a larger migration, breaks current commands, and makes the feature harder to land incrementally.

### 7. Reporting uses one shared query API, then fans out to TUI, attach, and JSON

**Choice:** Add a shared metrics query layer in `clankers-db` or a thin service above it. Standalone `/metrics` calls it directly. Attach mode uses a session/daemon query path but returns the same summary model. JSON mode and tests serialize the same structs.

**Rationale:** One summary model avoids drift between standalone and daemon reports. The attach client should not invent its own aggregation logic.

**Alternative:** Build separate reporting code for standalone and attach. Rejected because metrics definitions would drift fast.

### 8. Metrics persistence is best-effort and must not abort user work

**Choice:** Capture code updates in-memory summaries first. redb flush failures become warnings plus dropped-write counters; they do not fail the session, tool, or turn.

**Rationale:** Metrics are observability, not the primary product. A broken metrics write must not make `bash`, `read`, or a whole agent session fail.

**Alternative:** Treat metrics writes as required and bubble failures. Rejected because it couples user work to local observability storage.

## Risks / Trade-offs

- **[Cardinality blow-up]** -> Mitigate with raw-vs-digest separation, fixed top-N caps, and `other_count` buckets.
- **[redb write amplification]** -> Mitigate with batched flushes, session summaries as the primary read path, and bounded recent-event retention.
- **[Metrics drift from runtime reality]** -> Mitigate by deriving from `AgentEvent` and plugin/session seams already used for UI and audit, then adding reducer tests from real event traces.
- **[Too much overlap with audit/usage]** -> Mitigate by keeping a clear split: audit stores forensic tool details, usage stores daily totals, metrics stores cross-cutting summaries and bounded rollups.
- **[Crash before final flush]** -> Mitigate with periodic batch flushes during the session and a final flush on clean shutdown. A crash may lose the most recent in-memory events, but not prior flushed summaries.
- **[Over-instrumented plugin paths]** -> Mitigate by treating plugin metrics as one counted category with bounded fields, not a free-form label bag.

## Migration Plan

1. Add new redb tables and bump `clankers-db` schema version.
2. Land the pure reducer, histogram, heavy-hitter, and fingerprint helpers with unit tests.
3. Instrument agent, controller, tool, plugin, and usage seams to emit `MetricEvent`s.
4. Add periodic and final flush paths.
5. Add shared query/reporting surfaces and wire `/metrics` in standalone and attach mode.
6. Ship without backfill. Existing databases keep old audit/usage history; metrics start at rollout time.
7. If rollout causes regressions, disable capture behind settings or no-op the writer. Old tables remain intact.

## Open Questions

- Should the first reporting surface include a read-only agent tool as well as `/metrics`, or should v1 stay user-facing only?
- For procmon data, should the summary keep every sample bucket or only aggregates like peak RSS, average CPU, and process count?
- Should attach mode query historical metrics through the daemon control plane, the session stream, or both?
