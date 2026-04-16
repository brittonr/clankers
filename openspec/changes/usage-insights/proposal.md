## Why

Clankers tracks daily token usage and costs in `clankers-db/usage.rs` but provides no way to query or visualize this data. Users have no visibility into their token consumption patterns, cost trends, tool usage frequency, or model distribution. Hermes has a `/insights` command that generates comprehensive analytics from session history. This is table-stakes for users managing API spend across multiple providers and models.

## What Changes

- Add an insights engine that queries `clankers-db` for session, usage, and tool audit data
- Produce a structured report: token totals, cost estimates, tool call frequency, model breakdown, activity patterns, top sessions
- Expose as a `/insights` slash command in the TUI with configurable time window
- Render reports in terminal-friendly format (bar charts, tables)

## Capabilities

### New Capabilities
- `usage-insights`: Analytics engine producing token consumption, cost estimates, tool usage patterns, model distribution, and activity trends from historical session data.

### Modified Capabilities

## Impact

- `crates/clankers-db/` — new `insights` module with aggregate queries over usage, audit, and session_index tables
- `crates/clankers-tui/` or `src/commands/` — `/insights` slash command handler
- `crates/clankers-model-selection/src/cost_tracker.rs` — may need to expose pricing data for cost estimation
- No schema changes needed — queries existing redb tables
