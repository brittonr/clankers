//! High-level metrics query API.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use serde::Serialize;

use super::storage::MetricsStore;
use super::types::MetricEventKind;
use super::types::MetricEventRecord;
use super::types::SessionMetricsSummary;
use crate::error::Result;

#[derive(Debug, Clone, Serialize)]
pub struct CurrentSessionReport {
    pub session_id: String,
    pub turns: u32,
    pub turns_cancelled: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub compactions: u32,
    pub compaction_tokens_saved: u64,
    pub model_switches: u32,
    pub top_models: Vec<(String, u64)>,
    pub tool_calls: u64,
    pub tool_errors: u32,
    pub tool_mean_latency_ms: Option<f64>,
    pub top_tools: Vec<(String, u64)>,
    pub plugin_events: u32,
    pub plugin_errors: u32,
    pub plugin_hook_denials: u32,
    pub duration_secs: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HistoricalReport {
    pub days: Vec<DaySummary>,
    pub total_sessions: u32,
    pub total_turns: u32,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_tool_calls: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DaySummary {
    pub date: String,
    pub sessions: u32,
    pub turns: u32,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub tool_calls: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecentEvent {
    pub seq: u32,
    pub timestamp: String,
    pub kind: String,
    pub detail: String,
}

impl MetricsStore<'_> {
    pub fn current_session_report(&self, session_id: &str) -> Result<Option<CurrentSessionReport>> {
        let summary = match self.get_session_summary(session_id)? {
            Some(s) => s,
            None => return Ok(None),
        };
        Ok(Some(session_to_report(&summary)))
    }

    pub fn historical_report(&self, days: usize) -> Result<HistoricalReport> {
        let rollups = self.recent_daily_rollups(days)?;
        let mut report = HistoricalReport {
            days: Vec::new(),
            total_sessions: 0,
            total_turns: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_tool_calls: 0,
        };
        for r in &rollups {
            report.total_sessions += r.sessions;
            report.total_turns += r.turns;
            report.total_input_tokens += r.input_tokens;
            report.total_output_tokens += r.output_tokens;
            report.total_tool_calls += r.tool_calls;
            report.days.push(DaySummary {
                date: r.date.clone(),
                sessions: r.sessions,
                turns: r.turns,
                input_tokens: r.input_tokens,
                output_tokens: r.output_tokens,
                tool_calls: r.tool_calls,
            });
        }
        Ok(report)
    }

    pub fn recent_events_report(&self, session_id: &str, limit: usize) -> Result<Vec<RecentEvent>> {
        let events = self.recent_events_for_session(session_id, limit)?;
        Ok(events.iter().map(event_to_report).collect())
    }
}

fn session_to_report(s: &SessionMetricsSummary) -> CurrentSessionReport {
    let mut top_models: Vec<_> = s.models.top().iter().map(|(k, v)| (k.clone(), *v)).collect();
    top_models.sort_by(|a, b| b.1.cmp(&a.1));
    let mut top_tools: Vec<_> = s.tools.top().iter().map(|(k, v)| (k.clone(), *v)).collect();
    top_tools.sort_by(|a, b| b.1.cmp(&a.1));

    CurrentSessionReport {
        session_id: s.session_id.clone(),
        turns: s.turns_total,
        turns_cancelled: s.turns_cancelled,
        input_tokens: s.input_tokens,
        output_tokens: s.output_tokens,
        total_tokens: s.total_tokens(),
        cache_creation_tokens: s.cache_creation_tokens,
        cache_read_tokens: s.cache_read_tokens,
        compactions: s.compactions,
        compaction_tokens_saved: s.compaction_tokens_saved,
        model_switches: s.model_switches,
        top_models,
        tool_calls: s.tool_latency.count(),
        tool_errors: s.tool_errors,
        tool_mean_latency_ms: s.tool_latency.mean_ms(),
        top_tools,
        plugin_events: s.plugin_events,
        plugin_errors: s.plugin_errors,
        plugin_hook_denials: s.plugin_hook_denials,
        duration_secs: s.duration_secs(),
    }
}

fn event_to_report(e: &MetricEventRecord) -> RecentEvent {
    let (kind, detail) = match &e.kind {
        MetricEventKind::SessionStart => ("session_start", String::new()),
        MetricEventKind::SessionEnd => ("session_end", String::new()),
        MetricEventKind::TurnStart { index } => ("turn_start", format!("turn {index}")),
        MetricEventKind::TurnEnd { index, tool_calls } => {
            ("turn_end", format!("turn {index}, {tool_calls} tool calls"))
        }
        MetricEventKind::TurnCancel => ("turn_cancel", String::new()),
        MetricEventKind::ModelChange { from, to } => ("model_change", format!("{from} -> {to}")),
        MetricEventKind::Compaction { tokens_saved } => ("compaction", format!("{tokens_saved} tokens saved")),
        MetricEventKind::ToolExec {
            tool,
            duration_ms,
            is_error,
        } => {
            let status = if *is_error { "error" } else { "ok" };
            ("tool_exec", format!("{tool} {duration_ms}ms [{status}]"))
        }
        MetricEventKind::PluginLoad { plugin, ok } => {
            let status = if *ok { "ok" } else { "error" };
            ("plugin_load", format!("{plugin} [{status}]"))
        }
        MetricEventKind::PluginEvent { plugin } => ("plugin_event", plugin.clone()),
        MetricEventKind::PluginError { plugin } => ("plugin_error", plugin.clone()),
        MetricEventKind::PluginHookDenial { plugin, hook } => ("plugin_hook_denial", format!("{plugin}:{hook}")),
        MetricEventKind::UsageUpdate {
            input_tokens,
            output_tokens,
            model,
        } => ("usage_update", format!("{model} {input_tokens}in/{output_tokens}out")),
        MetricEventKind::ProcessSpawn { pid } => ("process_spawn", format!("pid {pid}")),
        MetricEventKind::ProcessExit { pid, peak_rss } => ("process_exit", format!("pid {pid} peak_rss {peak_rss}")),
    };
    RecentEvent {
        seq: e.seq,
        timestamp: e.timestamp.to_rfc3339(),
        kind: kind.to_string(),
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Db;
    use crate::metrics::types::DailyMetricsRollup;

    fn test_db() -> Result<Db> {
        Db::in_memory()
    }

    #[test]
    fn current_session_report_not_found() -> Result<()> {
        let db = test_db()?;
        assert!(db.metrics().current_session_report("nope")?.is_none());
        Ok(())
    }

    #[test]
    fn current_session_report_from_summary() -> Result<()> {
        let db = test_db()?;
        let store = db.metrics();
        let mut s = SessionMetricsSummary::new("s1".into());
        s.turns_total = 10;
        s.input_tokens = 5000;
        s.output_tokens = 2000;
        s.tool_latency.record(100);
        s.tool_latency.record(200);
        s.tools.increment("bash");
        s.tools.increment("bash");
        s.tools.increment("read");
        store.save_session_summary(&s)?;

        let report = store.current_session_report("s1")?.unwrap();
        assert_eq!(report.turns, 10);
        assert_eq!(report.total_tokens, 7000);
        assert_eq!(report.tool_calls, 2);
        assert!((report.tool_mean_latency_ms.unwrap() - 150.0).abs() < 0.01);
        assert_eq!(report.top_tools[0].0, "bash");
        Ok(())
    }

    #[test]
    fn historical_report_aggregates() -> Result<()> {
        let db = test_db()?;
        let store = db.metrics();
        let mut r1 = DailyMetricsRollup::new("2026-04-23".into());
        r1.sessions = 2;
        r1.turns = 10;
        r1.input_tokens = 1000;
        let mut r2 = DailyMetricsRollup::new("2026-04-24".into());
        r2.sessions = 3;
        r2.turns = 15;
        r2.input_tokens = 2000;
        store.save_daily_rollup(&r1)?;
        store.save_daily_rollup(&r2)?;

        let report = store.historical_report(7)?;
        assert_eq!(report.total_sessions, 5);
        assert_eq!(report.total_turns, 25);
        assert_eq!(report.total_input_tokens, 3000);
        assert_eq!(report.days.len(), 2);
        Ok(())
    }

    #[test]
    fn recent_events_report_formatting() -> Result<()> {
        let db = test_db()?;
        let store = db.metrics();
        store.append_recent_event(&MetricEventRecord {
            session_id: "s1".into(),
            seq: 0,
            timestamp: chrono::Utc::now(),
            kind: MetricEventKind::ToolExec {
                tool: "bash".into(),
                duration_ms: 150,
                is_error: false,
            },
        })?;
        let events = store.recent_events_report("s1", 10)?;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, "tool_exec");
        assert!(events[0].detail.contains("bash"));
        assert!(events[0].detail.contains("150ms"));
        Ok(())
    }
}
