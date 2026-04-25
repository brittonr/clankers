//! Usage insights engine — aggregate queries over usage, audit, and session data.

use std::collections::BTreeMap;
use std::collections::HashMap;

use chrono::DateTime;
use chrono::Duration;
use chrono::Utc;

use crate::Db;
use crate::error::Result;

// ── Report types ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct InsightsReport {
    pub window_days: u32,
    pub overview: Overview,
    pub model_breakdown: Vec<ModelEntry>,
    pub tool_breakdown: Vec<ToolEntry>,
    pub daily_activity: Vec<DayActivity>,
    pub top_sessions: Vec<SessionEntry>,
}

#[derive(Debug, Clone, Default)]
pub struct Overview {
    pub sessions: u32,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_creation_tokens: u64,
    pub total_cache_read_tokens: u64,
    pub total_requests: u32,
    pub estimated_cost: Option<f64>,
    pub avg_session_messages: f64,
}

impl Overview {
    pub fn total_tokens(&self) -> u64 {
        self.total_input_tokens + self.total_output_tokens
    }
}

#[derive(Debug, Clone)]
pub struct ModelEntry {
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub requests: u32,
    pub estimated_cost: Option<f64>,
    pub pct_of_total: f64,
}

#[derive(Debug, Clone)]
pub struct ToolEntry {
    pub tool: String,
    pub call_count: u64,
}

#[derive(Debug, Clone)]
pub struct DayActivity {
    pub date: String,
    pub sessions: u32,
    pub tokens: u64,
}

#[derive(Debug, Clone)]
pub struct SessionEntry {
    pub session_id: String,
    pub date: String,
    pub tokens: u64,
    pub model: String,
    pub prompt_preview: String,
}

// ── Query functions ─────────────────────────────────────────────────

pub fn generate_insights(db: &Db, days: u32) -> Result<InsightsReport> {
    let cutoff = Utc::now() - Duration::days(i64::from(days));

    let usage_data = query_usage_in_window(db, &cutoff)?;
    let tool_data = query_tool_calls_in_window(db, &cutoff)?;
    let session_data = query_sessions_in_window(db, &cutoff)?;

    let mut overview = Overview::default();
    let mut model_map: HashMap<String, (u64, u64, u32)> = HashMap::new();
    let mut daily_map: BTreeMap<String, (u32, u64)> = BTreeMap::new();

    for day in &usage_data {
        overview.total_input_tokens += day.input_tokens;
        overview.total_output_tokens += day.output_tokens;
        overview.total_cache_creation_tokens += day.cache_creation_tokens;
        overview.total_cache_read_tokens += day.cache_read_tokens;
        overview.total_requests += day.requests;

        for (model, mu) in &day.by_model {
            let entry = model_map.entry(model.clone()).or_default();
            entry.0 += mu.input_tokens;
            entry.1 += mu.output_tokens;
            entry.2 += mu.requests;
        }

        daily_map
            .entry(day.date.clone())
            .and_modify(|(_, t)| *t += day.input_tokens + day.output_tokens)
            .or_insert((0, day.input_tokens + day.output_tokens));
    }

    overview.sessions = session_data.len() as u32;
    if !session_data.is_empty() {
        let total_msgs: u64 = session_data.iter().map(|s| u64::from(s.message_count)).sum();
        overview.avg_session_messages = total_msgs as f64 / session_data.len() as f64;
    }

    for s in &session_data {
        let date = s.created_at.format("%Y-%m-%d").to_string();
        daily_map.entry(date).and_modify(|(c, _)| *c += 1).or_insert((1, 0));
    }

    let total_tokens = overview.total_tokens();
    let model_breakdown: Vec<ModelEntry> = {
        let mut entries: Vec<_> = model_map
            .into_iter()
            .map(|(model, (input, output, requests))| {
                let pct = if total_tokens > 0 {
                    (input + output) as f64 / total_tokens as f64 * 100.0
                } else {
                    0.0
                };
                ModelEntry {
                    model,
                    input_tokens: input,
                    output_tokens: output,
                    requests,
                    estimated_cost: None,
                    pct_of_total: pct,
                }
            })
            .collect();
        entries.sort_by(|a, b| {
            (b.input_tokens + b.output_tokens).cmp(&(a.input_tokens + a.output_tokens))
        });
        entries
    };

    let tool_breakdown: Vec<ToolEntry> = {
        let mut entries: Vec<_> = tool_data
            .into_iter()
            .map(|(tool, count)| ToolEntry {
                tool,
                call_count: count,
            })
            .collect();
        entries.sort_by(|a, b| b.call_count.cmp(&a.call_count));
        entries.truncate(15);
        entries
    };

    let daily_activity: Vec<DayActivity> = daily_map
        .into_iter()
        .map(|(date, (sessions, tokens))| DayActivity {
            date,
            sessions,
            tokens,
        })
        .collect();

    let top_sessions = build_top_sessions(&session_data, &usage_data);

    Ok(InsightsReport {
        window_days: days,
        overview,
        model_breakdown,
        tool_breakdown,
        daily_activity,
        top_sessions,
    })
}

fn query_usage_in_window(
    db: &Db,
    cutoff: &DateTime<Utc>,
) -> Result<Vec<crate::usage::DailyUsage>> {
    let cutoff_date = cutoff.format("%Y-%m-%d").to_string();
    let all = db.usage().recent_days(365)?;
    Ok(all.into_iter().filter(|d| d.date >= cutoff_date).collect())
}

fn query_tool_calls_in_window(
    db: &Db,
    cutoff: &DateTime<Utc>,
) -> Result<Vec<(String, u64)>> {
    let entries = db.audit().recent(10_000)?;
    let mut counts: HashMap<String, u64> = HashMap::new();
    for e in entries {
        if e.timestamp >= *cutoff {
            *counts.entry(e.tool.clone()).or_default() += 1;
        }
    }
    let mut result: Vec<_> = counts.into_iter().collect();
    result.sort_by(|a, b| b.1.cmp(&a.1));
    Ok(result)
}

fn query_sessions_in_window(
    db: &Db,
    cutoff: &DateTime<Utc>,
) -> Result<Vec<crate::session_index::SessionIndexEntry>> {
    let all = db.sessions().list_all()?;
    Ok(all.into_iter().filter(|s| s.created_at >= *cutoff).collect())
}

fn build_top_sessions(
    sessions: &[crate::session_index::SessionIndexEntry],
    _usage: &[crate::usage::DailyUsage],
) -> Vec<SessionEntry> {
    let mut entries: Vec<SessionEntry> = sessions
        .iter()
        .map(|s| SessionEntry {
            session_id: if s.session_id.len() > 12 {
                s.session_id[..12].to_string()
            } else {
                s.session_id.clone()
            },
            date: s.created_at.format("%Y-%m-%d").to_string(),
            tokens: 0,
            model: s.model.clone(),
            prompt_preview: s.first_prompt.chars().take(60).collect(),
        })
        .collect();
    entries.sort_by(|a, b| b.tokens.cmp(&a.tokens));
    entries.truncate(5);
    entries
}

// ── Terminal rendering ──────────────────────────────────────────────

pub fn format_insights_terminal(report: &InsightsReport) -> String {
    let mut out = String::new();
    use std::fmt::Write;

    writeln!(out, "## Usage Insights (last {} days)", report.window_days).ok();
    writeln!(out).ok();

    // Overview
    let o = &report.overview;
    writeln!(out, "### Overview").ok();
    writeln!(out, "  Sessions: {}", o.sessions).ok();
    writeln!(
        out,
        "  Tokens: {} in + {} out = {} total",
        fmt_count(o.total_input_tokens),
        fmt_count(o.total_output_tokens),
        fmt_count(o.total_tokens())
    )
    .ok();
    if o.total_cache_creation_tokens > 0 || o.total_cache_read_tokens > 0 {
        writeln!(
            out,
            "  Cache: {} created, {} read",
            fmt_count(o.total_cache_creation_tokens),
            fmt_count(o.total_cache_read_tokens)
        )
        .ok();
    }
    writeln!(out, "  Requests: {}", o.total_requests).ok();
    if let Some(cost) = o.estimated_cost {
        writeln!(out, "  Estimated cost: ${cost:.2}").ok();
    }
    if o.avg_session_messages > 0.0 {
        writeln!(out, "  Avg messages/session: {:.1}", o.avg_session_messages).ok();
    }
    writeln!(out).ok();

    // Model breakdown
    if !report.model_breakdown.is_empty() {
        writeln!(out, "### Models").ok();
        writeln!(out, "  {:20} {:>10} {:>10} {:>6} {:>6}", "Model", "Input", "Output", "Reqs", "%").ok();
        for m in &report.model_breakdown {
            writeln!(
                out,
                "  {:20} {:>10} {:>10} {:>6} {:>5.1}%",
                truncate(&m.model, 20),
                fmt_count(m.input_tokens),
                fmt_count(m.output_tokens),
                m.requests,
                m.pct_of_total
            )
            .ok();
        }
        writeln!(out).ok();
    }

    // Tool breakdown
    if !report.tool_breakdown.is_empty() {
        writeln!(out, "### Top Tools").ok();
        let max_count = report.tool_breakdown.first().map(|t| t.call_count).unwrap_or(1);
        for t in &report.tool_breakdown {
            let bar_len = if max_count > 0 {
                ((t.call_count as f64 / max_count as f64) * 20.0) as usize
            } else {
                0
            };
            let bar: String = "\u{2588}".repeat(bar_len);
            writeln!(out, "  {:16} {:>6}  {bar}", truncate(&t.tool, 16), t.call_count).ok();
        }
        writeln!(out).ok();
    }

    // Daily activity
    if !report.daily_activity.is_empty() {
        writeln!(out, "### Daily Activity").ok();
        let max_tokens = report.daily_activity.iter().map(|d| d.tokens).max().unwrap_or(1);
        for d in &report.daily_activity {
            let bar_len = if max_tokens > 0 {
                ((d.tokens as f64 / max_tokens as f64) * 30.0) as usize
            } else {
                0
            };
            let bar: String = "\u{2588}".repeat(bar_len);
            writeln!(
                out,
                "  {} {:>3}s {:>8}  {bar}",
                d.date,
                d.sessions,
                fmt_count(d.tokens)
            )
            .ok();
        }
        writeln!(out).ok();
    }

    // Top sessions
    if !report.top_sessions.is_empty() {
        writeln!(out, "### Top Sessions").ok();
        writeln!(out, "  {:12} {:10} {:>8} {:20} {}", "ID", "Date", "Tokens", "Model", "Prompt").ok();
        for s in &report.top_sessions {
            writeln!(
                out,
                "  {:12} {:10} {:>8} {:20} {}",
                s.session_id,
                s.date,
                fmt_count(s.tokens),
                truncate(&s.model, 20),
                truncate(&s.prompt_preview, 40)
            )
            .ok();
        }
    }

    out
}

fn fmt_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{n}")
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}

#[cfg(test)]
mod tests {
    use crate::session_index::SessionIndexEntry;
    use crate::usage::RequestUsage;

    use super::*;

    fn setup_test_db() -> Result<Db> {
        let db = Db::in_memory()?;

        // Add usage data
        db.usage().record(&RequestUsage::new("sonnet", 10_000, 5_000, 100, 50))?;
        db.usage().record(&RequestUsage::new("opus", 20_000, 10_000, 200, 100))?;
        db.usage().record(&RequestUsage::new("sonnet", 5_000, 2_000, 0, 0))?;

        // Add session index entries
        let now = Utc::now();
        db.sessions().upsert(&SessionIndexEntry {
            session_id: "sess-001".into(),
            cwd: "/home/user".into(),
            model: "sonnet".into(),
            created_at: now,
            message_count: 20,
            first_prompt: "fix the bug in auth module".into(),
            file_path: "/tmp/sess-001.jsonl".into(),
            agent: None,
            updated_at: now,
        })?;
        db.sessions().upsert(&SessionIndexEntry {
            session_id: "sess-002".into(),
            cwd: "/home/user".into(),
            model: "opus".into(),
            created_at: now,
            message_count: 50,
            first_prompt: "refactor the database layer".into(),
            file_path: "/tmp/sess-002.jsonl".into(),
            agent: None,
            updated_at: now,
        })?;

        Ok(db)
    }

    #[test]
    fn generate_insights_basic() -> Result<()> {
        let db = setup_test_db()?;
        let report = generate_insights(&db, 30)?;

        assert_eq!(report.overview.sessions, 2);
        assert!(report.overview.total_input_tokens > 0);
        assert!(report.overview.total_output_tokens > 0);
        assert_eq!(report.overview.total_requests, 3);
        assert!(report.overview.avg_session_messages > 0.0);
        Ok(())
    }

    #[test]
    fn generate_insights_empty() -> Result<()> {
        let db = Db::in_memory()?;
        let report = generate_insights(&db, 30)?;

        assert_eq!(report.overview.sessions, 0);
        assert_eq!(report.overview.total_tokens(), 0);
        assert_eq!(report.overview.total_requests, 0);
        assert!(report.model_breakdown.is_empty());
        assert!(report.tool_breakdown.is_empty());
        Ok(())
    }

    #[test]
    fn model_breakdown_sorted_by_tokens() -> Result<()> {
        let db = setup_test_db()?;
        let report = generate_insights(&db, 30)?;

        if report.model_breakdown.len() >= 2 {
            let first_total =
                report.model_breakdown[0].input_tokens + report.model_breakdown[0].output_tokens;
            let second_total =
                report.model_breakdown[1].input_tokens + report.model_breakdown[1].output_tokens;
            assert!(first_total >= second_total);
        }
        Ok(())
    }

    #[test]
    fn format_insights_produces_output() -> Result<()> {
        let db = setup_test_db()?;
        let report = generate_insights(&db, 30)?;
        let output = format_insights_terminal(&report);

        assert!(output.contains("Usage Insights"));
        assert!(output.contains("Overview"));
        assert!(output.contains("Sessions: 2"));
        assert!(output.contains("Models"));
        Ok(())
    }

    #[test]
    fn format_empty_report() -> Result<()> {
        let db = Db::in_memory()?;
        let report = generate_insights(&db, 30)?;
        let output = format_insights_terminal(&report);

        assert!(output.contains("Usage Insights"));
        assert!(output.contains("Sessions: 0"));
        Ok(())
    }
}
