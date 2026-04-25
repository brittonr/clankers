## Context

`clankers-db/src/usage.rs` tracks daily token counts and estimated costs per model. `clankers-db/src/audit.rs` logs every tool invocation with timestamps. `clankers-db/src/session_index.rs` indexes session metadata. This data exists but is not queryable by the user or agent — there's no command or tool to produce aggregate reports.

Hermes' `InsightsEngine` queries its SQLite database for sessions within a time window and computes: overview (total sessions, tokens, cost, avg session length), model breakdown, platform breakdown, tool usage frequency, activity patterns (by hour/day), and top sessions by token consumption.

## Goals / Non-Goals

**Goals:**
- Aggregate queries over existing redb tables (usage, audit, session_index)
- Structured report with: token totals, cost estimates, tool call frequency, model distribution, activity by day
- `/insights` slash command in the TUI with configurable time window (default 30 days)
- Terminal-friendly rendering: aligned tables, simple bar charts using block characters
- Per-model cost estimation using known pricing data

**Non-Goals:**
- Web dashboard or GUI visualization
- Real-time streaming metrics
- Comparison across users or machines
- Exporting to external analytics services

## Decisions

**Query engine in clankers-db:** Add an `insights` module that reads across the existing usage, audit, and session_index tables. All queries are range scans over the time dimension (redb keys include timestamps). No new tables needed.

**Report structure:**
```
Overview:     sessions, total tokens (in/out/cache), estimated cost, avg session length
Models:       table of model → sessions, tokens, cost, % of total
Tools:        table of tool → call count, ranked by frequency
Activity:     daily session counts for the time window (bar chart)
Top Sessions: 5 sessions with highest token consumption (id, date, tokens, model)
```

**Cost estimation:** Reuse pricing data from `clankers-model-selection/src/cost_tracker.rs`. For unknown models or local endpoints, show "unknown" instead of guessing.

**Slash command:** `/insights [days]` — defaults to 30. Renders the report inline in the TUI conversation view. No separate screen needed.

**No agent tool for now:** This is a user-facing command, not an agent tool. The agent doesn't need to query its own usage statistics during task execution. Can be added later if needed.

## Risks / Trade-offs

- **redb scan performance:** Range scans over audit log could be slow for users with millions of tool calls. Mitigate by scanning only the time window and stopping early. If needed, add a daily-aggregate cache table.
- **Pricing accuracy:** Token prices change and custom endpoints have unknown pricing. Always label cost estimates as approximate and note when pricing data is missing.
- **TUI rendering:** Long reports could overflow the visible area. Rely on the existing scroll mechanism in the conversation view.
