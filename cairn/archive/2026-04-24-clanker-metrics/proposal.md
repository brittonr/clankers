## Why

Clankers already records some observability data, but it is split across unrelated stores: tool audit entries in `clankers-db::audit`, daily token totals in `clankers-db::usage`, skill outcomes in `clankers-db::skill_usage`, and plugin activity mostly in logs. There is no single session-level metrics model that can answer basic questions like which tools dominate latency, which plugins are noisy or failing, how model switches affect cost, or what changed between a cheap successful session and an expensive failing one.

This change adds a real metrics layer. It keeps the existing audit and usage features, but adds bounded per-session metrics capture, redb-backed rollups, and query surfaces built for operator questions instead of raw forensics.

## What Changes

- Add a unified metrics event pipeline for session lifecycle, turn lifecycle, model changes, compaction, tool execution, plugin activity, token usage, and process-monitoring signals when available.
- Persist metrics to redb as bounded session summaries, daily rollups, and a recent-event log instead of scattering data across unrelated ad hoc tables.
- Fingerprint high-cardinality or sensitive dimensions with BLAKE3 before storing them in metrics tables so reports stay useful without duplicating raw prompt text, bash commands, file paths, or plugin payloads.
- Add reporting surfaces for current-session and historical metrics in standalone mode and attach mode.
- Keep the implementation Tiger Style: pure reducer core, bounded buffers, fixed histograms, explicit limits, and best-effort persistence that never aborts a user session.

## Capabilities

### New Capabilities
- `session-metrics-capture`: Capture unified runtime metrics for sessions, turns, tools, plugins, models, tokens, compaction, and related execution events.
- `metrics-storage`: Store session summaries, daily rollups, bounded recent events, and BLAKE3 fingerprints in redb with explicit retention limits.
- `metrics-reporting`: Query and display current-session and historical metrics in standalone mode, attach mode, and machine-readable outputs.

### Modified Capabilities
None. I checked the current `openspec/specs/` tree and there is no existing base capability for metrics capture, metrics storage, or metrics reporting. This change adds new contracts instead of changing an existing one.

## Impact

- `crates/clankers-agent/src/events.rs` and turn execution paths — add or reuse event hooks for metrics capture
- `crates/clankers-controller/` — collect session-level metrics at the transport-agnostic seam
- `src/modes/event_loop_runner/`, `src/modes/plugin_dispatch.rs`, and daemon session plumbing — instrument plugin activity and attach-mode reporting
- `crates/clankers-db/src/` — add metrics tables, schema migration, query API, and retention logic on top of redb
- `src/slash_commands/handlers/` and protocol/reporting surfaces — add user-visible metrics summaries and historical queries
- `Cargo.toml` / `crates/clankers-db/Cargo.toml` — add BLAKE3 dependency for fingerprinting if not already present elsewhere in the workspace
