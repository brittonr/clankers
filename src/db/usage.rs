//! Token usage and cost tracking.
//!
//! Records per-request usage and aggregates daily totals.
//! Enables `/usage` to show stats without scanning session files.

use std::cmp::Reverse;
use std::collections::HashMap;

use chrono::DateTime;
use chrono::Utc;
use redb::ReadableTable;
use redb::ReadableTableMetadata;
use redb::TableDefinition;
use serde::Deserialize;
use serde::Serialize;

use super::Db;
use super::db_err;
use crate::error::Result;

/// Table: date string "2026-02-25" → serialized DailyUsage
pub(crate) const TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("usage_daily");

/// Aggregated usage for a single day.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DailyUsage {
    /// Date string (YYYY-MM-DD).
    pub date: String,
    /// Total input tokens.
    pub input_tokens: u64,
    /// Total output tokens.
    pub output_tokens: u64,
    /// Cache creation tokens.
    pub cache_creation_tokens: u64,
    /// Cache read tokens.
    pub cache_read_tokens: u64,
    /// Number of API requests.
    pub requests: u32,
    /// Token breakdown by model.
    pub by_model: HashMap<String, ModelUsage>,
}

impl DailyUsage {
    /// Total tokens (input + output).
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }
}

/// Per-model usage within a day.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub requests: u32,
}

/// A single request's usage, to be recorded.
pub struct RequestUsage {
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub timestamp: DateTime<Utc>,
}

impl RequestUsage {
    /// Create from a provider Usage struct.
    pub fn from_provider(model: &str, usage: &crate::provider::Usage) -> Self {
        Self {
            model: model.to_string(),
            input_tokens: usage.input_tokens as u64,
            output_tokens: usage.output_tokens as u64,
            cache_creation_tokens: usage.cache_creation_input_tokens as u64,
            cache_read_tokens: usage.cache_read_input_tokens as u64,
            timestamp: Utc::now(),
        }
    }
}

/// Accessor for usage tracking.
pub struct UsageTracker<'db> {
    db: &'db Db,
}

impl<'db> UsageTracker<'db> {
    pub(crate) fn new(db: &'db Db) -> Self {
        Self { db }
    }

    /// Record a single request's usage. Merges into the day's totals.
    pub fn record(&self, req: &RequestUsage) -> Result<()> {
        let date = req.timestamp.format("%Y-%m-%d").to_string();

        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;

            // Read existing day or start fresh
            let mut daily = match table.get(date.as_str()).map_err(db_err)? {
                Some(value) => serde_json::from_slice::<DailyUsage>(value.value()).unwrap_or_default(),
                None => DailyUsage {
                    date: date.clone(),
                    ..Default::default()
                },
            };

            // Merge
            daily.date = date.clone();
            daily.input_tokens += req.input_tokens;
            daily.output_tokens += req.output_tokens;
            daily.cache_creation_tokens += req.cache_creation_tokens;
            daily.cache_read_tokens += req.cache_read_tokens;
            daily.requests += 1;

            let model_entry = daily.by_model.entry(req.model.clone()).or_default();
            model_entry.input_tokens += req.input_tokens;
            model_entry.output_tokens += req.output_tokens;
            model_entry.requests += 1;

            let bytes = serde_json::to_vec(&daily).map_err(|e| crate::error::Error::Database {
                message: format!("failed to serialize usage: {e}"),
            })?;
            table.insert(date.as_str(), bytes.as_slice()).map_err(db_err)?;
        }
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// Get usage for a specific date.
    pub fn daily(&self, date: &str) -> Result<Option<DailyUsage>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;
        match table.get(date).map_err(db_err)? {
            Some(value) => {
                let entry = serde_json::from_slice(value.value()).map_err(|e| crate::error::Error::Database {
                    message: format!("failed to deserialize usage: {e}"),
                })?;
                Ok(Some(entry))
            }
            None => Ok(None),
        }
    }

    /// Get today's usage.
    pub fn today(&self) -> Result<Option<DailyUsage>> {
        let date = Utc::now().format("%Y-%m-%d").to_string();
        self.daily(&date)
    }

    /// Get usage for the last N days (newest first).
    pub fn recent_days(&self, n: usize) -> Result<Vec<DailyUsage>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;

        let mut entries = Vec::new();
        // Date strings sort lexicographically = chronologically
        for item in table.iter().map_err(db_err)?.rev() {
            if entries.len() >= n {
                break;
            }
            let (_key, value) = item.map_err(db_err)?;
            if let Ok(entry) = serde_json::from_slice::<DailyUsage>(value.value()) {
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    /// Sum usage across all recorded days.
    pub fn total(&self) -> Result<DailyUsage> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;

        let mut total = DailyUsage {
            date: "all-time".into(),
            ..Default::default()
        };

        for item in table.iter().map_err(db_err)? {
            let (_key, value) = item.map_err(db_err)?;
            if let Ok(day) = serde_json::from_slice::<DailyUsage>(value.value()) {
                total.input_tokens += day.input_tokens;
                total.output_tokens += day.output_tokens;
                total.cache_creation_tokens += day.cache_creation_tokens;
                total.cache_read_tokens += day.cache_read_tokens;
                total.requests += day.requests;

                for (model, mu) in &day.by_model {
                    let entry = total.by_model.entry(model.clone()).or_default();
                    entry.input_tokens += mu.input_tokens;
                    entry.output_tokens += mu.output_tokens;
                    entry.requests += mu.requests;
                }
            }
        }
        Ok(total)
    }

    /// Format usage summary as text (for `/usage` command).
    pub fn format_summary(&self) -> Result<String> {
        let today = self.today()?;
        let week = self.recent_days(7)?;
        let all = self.total()?;

        let mut out = String::new();

        // Today
        out.push_str("## Today\n");
        if let Some(ref t) = today {
            out.push_str(&format_daily(t));
        } else {
            out.push_str("  No usage recorded today.\n");
        }

        // Last 7 days
        if week.len() > 1 {
            out.push_str("\n## Last 7 days\n");
            let week_total: u64 = week.iter().map(|d| d.total_tokens()).sum();
            let week_reqs: u32 = week.iter().map(|d| d.requests).sum();
            out.push_str(&format!("  {week_total} tokens across {week_reqs} requests ({} days active)\n", week.len()));
        }

        // All time
        if all.requests > 0 {
            out.push_str("\n## All time\n");
            out.push_str(&format_daily(&all));

            if all.by_model.len() > 1 {
                out.push_str("\n  By model:\n");
                let mut models: Vec<_> = all.by_model.iter().collect();
                models.sort_by_key(|e| Reverse(e.1.input_tokens));
                for (model, mu) in models {
                    out.push_str(&format!(
                        "    {model}: {} in + {} out ({} reqs)\n",
                        format_tokens(mu.input_tokens),
                        format_tokens(mu.output_tokens),
                        mu.requests,
                    ));
                }
            }
        }

        Ok(out)
    }

    /// Remove all usage data (for testing / reset).
    pub fn clear(&self) -> Result<u64> {
        let tx = self.db.begin_write()?;
        let count = {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;
            let count = table.len().map_err(db_err)?;
            table.retain(|_, _| false).map_err(db_err)?;
            count
        };
        tx.commit().map_err(db_err)?;
        Ok(count)
    }
}

fn format_daily(d: &DailyUsage) -> String {
    format!(
        "  {} in + {} out = {} total ({} requests)\n",
        format_tokens(d.input_tokens),
        format_tokens(d.output_tokens),
        format_tokens(d.total_tokens()),
        d.requests,
    )
}

fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{n}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Db {
        Db::in_memory().unwrap()
    }

    fn make_request(model: &str, input: u64, output: u64) -> RequestUsage {
        RequestUsage {
            model: model.to_string(),
            input_tokens: input,
            output_tokens: output,
            cache_creation_tokens: 0,
            cache_read_tokens: 0,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_record_and_today() {
        let db = test_db();
        let usage = db.usage();

        usage.record(&make_request("sonnet", 100, 50)).unwrap();
        usage.record(&make_request("sonnet", 200, 100)).unwrap();

        let today = usage.today().unwrap().unwrap();
        assert_eq!(today.input_tokens, 300);
        assert_eq!(today.output_tokens, 150);
        assert_eq!(today.requests, 2);
        assert_eq!(today.total_tokens(), 450);
    }

    #[test]
    fn test_record_multi_model() {
        let db = test_db();
        let usage = db.usage();

        usage.record(&make_request("sonnet", 100, 50)).unwrap();
        usage.record(&make_request("haiku", 200, 100)).unwrap();
        usage.record(&make_request("sonnet", 300, 150)).unwrap();

        let today = usage.today().unwrap().unwrap();
        assert_eq!(today.requests, 3);
        assert_eq!(today.by_model.len(), 2);

        let sonnet = &today.by_model["sonnet"];
        assert_eq!(sonnet.input_tokens, 400);
        assert_eq!(sonnet.requests, 2);

        let haiku = &today.by_model["haiku"];
        assert_eq!(haiku.input_tokens, 200);
        assert_eq!(haiku.requests, 1);
    }

    #[test]
    fn test_daily_specific_date() {
        let db = test_db();
        let usage = db.usage();

        // Record for today
        usage.record(&make_request("sonnet", 100, 50)).unwrap();

        let date = Utc::now().format("%Y-%m-%d").to_string();
        let daily = usage.daily(&date).unwrap().unwrap();
        assert_eq!(daily.input_tokens, 100);

        // Non-existent date
        assert!(usage.daily("2020-01-01").unwrap().is_none());
    }

    #[test]
    fn test_today_empty() {
        let db = test_db();
        assert!(db.usage().today().unwrap().is_none());
    }

    #[test]
    fn test_total() {
        let db = test_db();
        let usage = db.usage();

        usage.record(&make_request("sonnet", 100, 50)).unwrap();
        usage.record(&make_request("haiku", 200, 100)).unwrap();

        let total = usage.total().unwrap();
        assert_eq!(total.input_tokens, 300);
        assert_eq!(total.output_tokens, 150);
        assert_eq!(total.requests, 2);
    }

    #[test]
    fn test_total_empty() {
        let db = test_db();
        let total = db.usage().total().unwrap();
        assert_eq!(total.requests, 0);
        assert_eq!(total.total_tokens(), 0);
    }

    #[test]
    fn test_format_tokens() {
        assert_eq!(format_tokens(500), "500");
        assert_eq!(format_tokens(1_500), "1.5K");
        assert_eq!(format_tokens(1_500_000), "1.5M");
    }

    #[test]
    fn test_format_summary_empty() {
        let db = test_db();
        let summary = db.usage().format_summary().unwrap();
        assert!(summary.contains("No usage recorded today"));
    }

    #[test]
    fn test_format_summary_with_data() {
        let db = test_db();
        let usage = db.usage();

        usage.record(&make_request("sonnet", 10_000, 5_000)).unwrap();

        let summary = usage.format_summary().unwrap();
        assert!(summary.contains("Today"));
        assert!(summary.contains("10.0K"));
    }

    #[test]
    fn test_clear() {
        let db = test_db();
        let usage = db.usage();

        usage.record(&make_request("sonnet", 100, 50)).unwrap();
        let cleared = usage.clear().unwrap();
        assert_eq!(cleared, 1);
        assert!(usage.today().unwrap().is_none());
    }

    #[test]
    fn test_cache_tokens() {
        let db = test_db();
        let usage = db.usage();

        let req = RequestUsage {
            model: "sonnet".into(),
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_tokens: 500,
            cache_read_tokens: 200,
            timestamp: Utc::now(),
        };
        usage.record(&req).unwrap();

        let today = usage.today().unwrap().unwrap();
        assert_eq!(today.cache_creation_tokens, 500);
        assert_eq!(today.cache_read_tokens, 200);
    }
}
