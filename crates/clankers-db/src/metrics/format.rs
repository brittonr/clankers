//! Terminal-friendly metrics formatting.

use std::fmt::Write;

use super::query::CurrentSessionReport;
use super::query::HistoricalReport;

pub fn format_current_session(r: &CurrentSessionReport) -> String {
    let mut out = String::new();
    writeln!(out, "## Current Session: {}", r.session_id).ok();
    if let Some(dur) = r.duration_secs {
        writeln!(out, "  Duration: {}", format_duration(dur)).ok();
    }
    writeln!(
        out,
        "  Turns: {} ({} cancelled)",
        r.turns, r.turns_cancelled
    )
    .ok();
    writeln!(out).ok();

    writeln!(out, "### Tokens").ok();
    writeln!(
        out,
        "  Input: {}  Output: {}  Total: {}",
        format_count(r.input_tokens),
        format_count(r.output_tokens),
        format_count(r.total_tokens)
    )
    .ok();
    if r.cache_creation_tokens > 0 || r.cache_read_tokens > 0 {
        writeln!(
            out,
            "  Cache: {} created, {} read",
            format_count(r.cache_creation_tokens),
            format_count(r.cache_read_tokens)
        )
        .ok();
    }
    writeln!(out).ok();

    writeln!(out, "### Tools").ok();
    writeln!(out, "  Calls: {}  Errors: {}", r.tool_calls, r.tool_errors).ok();
    if let Some(mean) = r.tool_mean_latency_ms {
        writeln!(out, "  Mean latency: {mean:.0}ms").ok();
    }
    if !r.top_tools.is_empty() {
        writeln!(out, "  Top tools:").ok();
        for (name, count) in r.top_tools.iter().take(10) {
            writeln!(out, "    {name}: {count}").ok();
        }
    }
    writeln!(out).ok();

    if !r.top_models.is_empty() {
        writeln!(out, "### Models ({} switches)", r.model_switches).ok();
        for (name, count) in &r.top_models {
            writeln!(out, "  {name}: {count}").ok();
        }
        writeln!(out).ok();
    }

    if r.compactions > 0 {
        writeln!(
            out,
            "### Compaction: {} times, {} tokens saved",
            r.compactions,
            format_count(r.compaction_tokens_saved)
        )
        .ok();
        writeln!(out).ok();
    }

    if r.plugin_events > 0 || r.plugin_errors > 0 {
        writeln!(out, "### Plugins").ok();
        writeln!(
            out,
            "  Events: {}  Errors: {}  Hook denials: {}",
            r.plugin_events, r.plugin_errors, r.plugin_hook_denials
        )
        .ok();
    }

    out
}

pub fn format_historical(r: &HistoricalReport) -> String {
    let mut out = String::new();
    writeln!(
        out,
        "## History ({} days, {} sessions)",
        r.days.len(),
        r.total_sessions
    )
    .ok();
    writeln!(
        out,
        "  Total turns: {}  Tokens: {} in / {} out  Tool calls: {}",
        r.total_turns,
        format_count(r.total_input_tokens),
        format_count(r.total_output_tokens),
        r.total_tool_calls
    )
    .ok();
    writeln!(out).ok();

    if !r.days.is_empty() {
        writeln!(out, "  Date        Sessions  Turns    Tokens       Tools").ok();
        for d in &r.days {
            writeln!(
                out,
                "  {}  {:>8}  {:>5}  {:>11}  {:>5}",
                d.date,
                d.sessions,
                d.turns,
                format_count(d.input_tokens + d.output_tokens),
                d.tool_calls
            )
            .ok();
        }
    }
    out
}

fn format_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{n}")
    }
}

fn format_duration(secs: f64) -> String {
    if secs >= 3600.0 {
        format!("{:.1}h", secs / 3600.0)
    } else if secs >= 60.0 {
        format!("{:.0}m", secs / 60.0)
    } else {
        format!("{secs:.0}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::query::DaySummary;

    #[test]
    fn format_current_session_basic() {
        let r = CurrentSessionReport {
            session_id: "test-123".into(),
            turns: 10,
            turns_cancelled: 1,
            input_tokens: 50_000,
            output_tokens: 20_000,
            total_tokens: 70_000,
            cache_creation_tokens: 0,
            cache_read_tokens: 0,
            compactions: 0,
            compaction_tokens_saved: 0,
            model_switches: 1,
            top_models: vec![("sonnet".into(), 8), ("opus".into(), 2)],
            tool_calls: 15,
            tool_errors: 1,
            tool_mean_latency_ms: Some(120.5),
            top_tools: vec![("bash".into(), 10), ("read".into(), 5)],
            plugin_events: 0,
            plugin_errors: 0,
            plugin_hook_denials: 0,
            duration_secs: Some(300.0),
        };
        let output = format_current_session(&r);
        assert!(output.contains("test-123"));
        assert!(output.contains("50.0K"));
        assert!(output.contains("bash: 10"));
        assert!(output.contains("5m"));
    }

    #[test]
    fn format_historical_table() {
        let r = HistoricalReport {
            days: vec![DaySummary {
                date: "2026-04-24".into(),
                sessions: 3,
                turns: 15,
                input_tokens: 10_000,
                output_tokens: 5_000,
                tool_calls: 20,
            }],
            total_sessions: 3,
            total_turns: 15,
            total_input_tokens: 10_000,
            total_output_tokens: 5_000,
            total_tool_calls: 20,
        };
        let output = format_historical(&r);
        assert!(output.contains("2026-04-24"));
        assert!(output.contains("3 sessions"));
    }
}
