//! Token usage and cost tracking.
//!
//! Records per-request usage and aggregates daily totals by provider
//! and model. Enables cost-aware routing decisions and spend reporting.

use std::collections::HashMap;

use chrono::DateTime;
use chrono::Utc;
use redb::ReadableTable;
use redb::ReadableTableMetadata;
use redb::TableDefinition;
use serde::Deserialize;
use serde::Serialize;

use super::RouterDb;
use super::db_err;
use crate::error::Result;

/// Table: date string "2026-02-27" → serialized DailyUsage
pub(crate) const TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("router_usage_daily");

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
    /// Estimated cost in USD.
    pub estimated_cost_usd: f64,
    /// Breakdown by provider.
    pub by_provider: HashMap<String, ProviderUsage>,
}

impl DailyUsage {
    /// Total tokens (input + output).
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }
}

/// Per-provider usage within a day.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub requests: u32,
    pub estimated_cost_usd: f64,
    /// Further breakdown by model.
    pub by_model: HashMap<String, ModelUsage>,
}

/// Per-model usage within a provider within a day.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub requests: u32,
    pub estimated_cost_usd: f64,
}

/// A single request's usage, to be recorded.
pub struct RequestUsage {
    pub provider: String,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub estimated_cost_usd: f64,
    pub timestamp: DateTime<Utc>,
}

impl RequestUsage {
    /// Create from a provider Usage struct with cost estimation.
    pub fn from_provider_usage(provider: &str, model: &str, usage: &crate::provider::Usage, cost: Option<f64>) -> Self {
        Self {
            provider: provider.to_string(),
            model: model.to_string(),
            input_tokens: usage.input_tokens as u64,
            output_tokens: usage.output_tokens as u64,
            cache_creation_tokens: usage.cache_creation_input_tokens as u64,
            cache_read_tokens: usage.cache_read_input_tokens as u64,
            estimated_cost_usd: cost.unwrap_or(0.0),
            timestamp: Utc::now(),
        }
    }
}

/// Accessor for usage tracking.
pub struct UsageTracker<'db> {
    db: &'db RouterDb,
}

impl<'db> UsageTracker<'db> {
    pub(crate) fn new(db: &'db RouterDb) -> Self {
        Self { db }
    }

    /// Record a single request's usage. Merges into the day's totals.
    pub fn record(&self, req: &RequestUsage) -> Result<()> {
        let date = req.timestamp.format("%Y-%m-%d").to_string();

        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;

            let mut daily = match table.get(date.as_str()).map_err(db_err)? {
                Some(value) => serde_json::from_slice::<DailyUsage>(value.value()).unwrap_or_default(),
                None => DailyUsage {
                    date: date.clone(),
                    ..Default::default()
                },
            };

            // Aggregate at the day level
            daily.date = date.clone();
            daily.input_tokens += req.input_tokens;
            daily.output_tokens += req.output_tokens;
            daily.cache_creation_tokens += req.cache_creation_tokens;
            daily.cache_read_tokens += req.cache_read_tokens;
            daily.requests += 1;
            daily.estimated_cost_usd += req.estimated_cost_usd;

            // Aggregate at the provider level
            let prov = daily.by_provider.entry(req.provider.clone()).or_default();
            prov.input_tokens += req.input_tokens;
            prov.output_tokens += req.output_tokens;
            prov.requests += 1;
            prov.estimated_cost_usd += req.estimated_cost_usd;

            // Aggregate at the model level
            let model = prov.by_model.entry(req.model.clone()).or_default();
            model.input_tokens += req.input_tokens;
            model.output_tokens += req.output_tokens;
            model.requests += 1;
            model.estimated_cost_usd += req.estimated_cost_usd;

            let bytes = serde_json::to_vec(&daily).map_err(|e| crate::Error::Config {
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
                let entry = serde_json::from_slice(value.value()).map_err(|e| crate::Error::Config {
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
                total.estimated_cost_usd += day.estimated_cost_usd;

                for (prov_name, prov) in &day.by_provider {
                    let entry = total.by_provider.entry(prov_name.clone()).or_default();
                    entry.input_tokens += prov.input_tokens;
                    entry.output_tokens += prov.output_tokens;
                    entry.requests += prov.requests;
                    entry.estimated_cost_usd += prov.estimated_cost_usd;

                    for (model_name, mu) in &prov.by_model {
                        let me = entry.by_model.entry(model_name.clone()).or_default();
                        me.input_tokens += mu.input_tokens;
                        me.output_tokens += mu.output_tokens;
                        me.requests += mu.requests;
                        me.estimated_cost_usd += mu.estimated_cost_usd;
                    }
                }
            }
        }
        Ok(total)
    }

    /// Remove all usage data.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> RouterDb {
        RouterDb::in_memory().unwrap()
    }

    fn make_request(provider: &str, model: &str, input: u64, output: u64) -> RequestUsage {
        RequestUsage {
            provider: provider.to_string(),
            model: model.to_string(),
            input_tokens: input,
            output_tokens: output,
            cache_creation_tokens: 0,
            cache_read_tokens: 0,
            estimated_cost_usd: 0.0,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_record_and_today() {
        let db = test_db();
        let usage = db.usage();

        usage.record(&make_request("anthropic", "sonnet", 100, 50)).unwrap();
        usage.record(&make_request("anthropic", "sonnet", 200, 100)).unwrap();

        let today = usage.today().unwrap().unwrap();
        assert_eq!(today.input_tokens, 300);
        assert_eq!(today.output_tokens, 150);
        assert_eq!(today.requests, 2);
        assert_eq!(today.total_tokens(), 450);
    }

    #[test]
    fn test_record_multi_provider() {
        let db = test_db();
        let usage = db.usage();

        usage.record(&make_request("anthropic", "sonnet", 100, 50)).unwrap();
        usage.record(&make_request("openai", "gpt-4o", 200, 100)).unwrap();
        usage.record(&make_request("anthropic", "haiku", 300, 150)).unwrap();

        let today = usage.today().unwrap().unwrap();
        assert_eq!(today.requests, 3);
        assert_eq!(today.by_provider.len(), 2);

        let anthropic = &today.by_provider["anthropic"];
        assert_eq!(anthropic.input_tokens, 400);
        assert_eq!(anthropic.requests, 2);
        assert_eq!(anthropic.by_model.len(), 2);

        let openai = &today.by_provider["openai"];
        assert_eq!(openai.input_tokens, 200);
        assert_eq!(openai.requests, 1);
    }

    #[test]
    fn test_cost_tracking() {
        let db = test_db();
        let usage = db.usage();

        let req = RequestUsage {
            provider: "anthropic".into(),
            model: "sonnet".into(),
            input_tokens: 1_000_000,
            output_tokens: 10_000,
            cache_creation_tokens: 0,
            cache_read_tokens: 0,
            estimated_cost_usd: 3.15,
            timestamp: Utc::now(),
        };
        usage.record(&req).unwrap();

        let today = usage.today().unwrap().unwrap();
        assert!((today.estimated_cost_usd - 3.15).abs() < 0.001);
        assert!((today.by_provider["anthropic"].estimated_cost_usd - 3.15).abs() < 0.001);
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

        usage.record(&make_request("anthropic", "sonnet", 100, 50)).unwrap();
        usage.record(&make_request("openai", "gpt-4o", 200, 100)).unwrap();

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
    fn test_clear() {
        let db = test_db();
        let usage = db.usage();

        usage.record(&make_request("anthropic", "sonnet", 100, 50)).unwrap();
        let cleared = usage.clear().unwrap();
        assert_eq!(cleared, 1);
        assert!(usage.today().unwrap().is_none());
    }

    #[test]
    fn test_cache_tokens() {
        let db = test_db();
        let usage = db.usage();

        let req = RequestUsage {
            provider: "anthropic".into(),
            model: "sonnet".into(),
            input_tokens: 100,
            output_tokens: 50,
            cache_creation_tokens: 500,
            cache_read_tokens: 200,
            estimated_cost_usd: 0.0,
            timestamp: Utc::now(),
        };
        usage.record(&req).unwrap();

        let today = usage.today().unwrap().unwrap();
        assert_eq!(today.cache_creation_tokens, 500);
        assert_eq!(today.cache_read_tokens, 200);
    }
}
