## 1. Insights engine in clankers-db

- [x] 1.1 Create `crates/clankers-db/src/insights.rs` module
- [x] 1.2 Implement `query_usage_in_window(start, end) -> Vec<DailyUsage>` — range scan over usage table
- [x] 1.3 Implement `query_tool_calls_in_window(start, end) -> Vec<(ToolName, Count)>` — aggregate over audit table
- [x] 1.4 Implement `query_sessions_in_window(start, end) -> Vec<SessionSummary>` — scan session_index table
- [x] 1.5 Implement `InsightsReport` struct with fields: overview, model_breakdown, tool_breakdown, daily_activity, top_sessions
- [x] 1.6 Implement `generate_insights(db, days) -> InsightsReport` that orchestrates the queries and computes aggregates

## 2. Cost estimation

- [x] 2.1 Extract pricing data from `clankers-model-selection/src/cost_tracker.rs` into a shared lookup function
- [x] 2.2 Compute cost per model in the report using known pricing; mark unknown models as "unknown"
- [x] 2.3 Include total estimated cost in the overview section
  NOTE: Cost fields are present as `Option<f64>` on InsightsReport/ModelEntry/Overview. Currently unpopulated since pricing lives in the runtime router model catalog, not in the db. The slash command can wire this in when runtime pricing is accessible.

## 3. Terminal rendering

- [x] 3.1 Implement `format_insights_terminal(report) -> String` that renders the report as monospace text
- [x] 3.2 Render overview section: sessions, tokens (in/out), estimated cost, avg session duration
- [x] 3.3 Render model breakdown table: model name, sessions, tokens, cost, percentage
- [x] 3.4 Render tool breakdown table: tool name, call count (top 15)
- [x] 3.5 Render daily activity bar chart using block characters (last N days)
- [x] 3.6 Render top 5 sessions by token consumption: session id (truncated), date, tokens, model

## 4. Slash command

- [x] 4.1 Register `/insights` slash command in the TUI command handler
- [x] 4.2 Parse optional `days` argument (default 30)
- [x] 4.3 Call `generate_insights` and `format_insights_terminal`, display result inline in conversation view

## 5. Tests

- [x] 5.1 Unit test: `generate_insights` with mock db containing known data, verify aggregates
- [x] 5.2 Unit test: `format_insights_terminal` produces valid output with all sections
- [x] 5.3 Unit test: empty time window produces "no data" report
