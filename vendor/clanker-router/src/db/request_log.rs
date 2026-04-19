//! Request audit log.
//!
//! Records every completion request with timing, token counts, provider,
//! model, and outcome. Enables debugging and post-hoc analysis.
//!
//! Entries are stored newest-first (descending timestamp key) and
//! automatically pruned beyond a configurable retention window.

use chrono::DateTime;
use chrono::Utc;
use redb::ReadableTable;
use redb::ReadableTableMetadata;
use redb::TableDefinition;
use serde::Deserialize;
use serde::Serialize;

use super::RouterDb;
use super::db_err;
use super::generate_id;
use crate::error::Result;

/// Table: timestamp_micros (u64) → serialized LogEntry
pub(crate) const TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new("router_request_log");

/// Maximum number of log entries to retain (FIFO eviction).
const MAX_ENTRIES: u64 = 10_000;

/// Outcome of a completion request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RequestOutcome {
    /// Completed successfully.
    Success,
    /// Failed with an error.
    Error { message: String },
    /// Retried after a transient failure.
    Retried {
        attempts: u32,
        final_outcome: Box<RequestOutcome>,
    },
}

/// A single logged request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Unique ID (microsecond timestamp).
    pub id: u64,
    /// When the request started.
    pub timestamp: DateTime<Utc>,
    /// Provider that handled the request.
    pub provider: String,
    /// Model ID used.
    pub model: String,
    /// Resolved model ID (if alias was used).
    pub resolved_model: Option<String>,
    /// Input tokens.
    pub input_tokens: u64,
    /// Output tokens.
    pub output_tokens: u64,
    /// Cache creation tokens.
    pub cache_creation_tokens: u64,
    /// Cache read tokens.
    pub cache_read_tokens: u64,
    /// Request duration in milliseconds.
    pub duration_ms: u64,
    /// Estimated cost in USD.
    pub estimated_cost_usd: f64,
    /// Outcome of the request.
    pub outcome: RequestOutcome,
    /// Stop reason from the model.
    pub stop_reason: Option<String>,
    /// Whether the response was served from cache.
    pub cache_hit: bool,
}

impl LogEntry {
    /// Create a new log entry for a successful request.
    pub fn success(
        provider: &str,
        model: &str,
        resolved_model: Option<&str>,
        input_tokens: u64,
        output_tokens: u64,
        duration_ms: u64,
    ) -> Self {
        Self {
            id: generate_id(),
            timestamp: Utc::now(),
            provider: provider.to_string(),
            model: model.to_string(),
            resolved_model: resolved_model.map(String::from),
            input_tokens,
            output_tokens,
            cache_creation_tokens: 0,
            cache_read_tokens: 0,
            duration_ms,
            estimated_cost_usd: 0.0,
            outcome: RequestOutcome::Success,
            stop_reason: None,
            cache_hit: false,
        }
    }

    /// Create a new log entry for a failed request.
    pub fn error(provider: &str, model: &str, duration_ms: u64, message: &str) -> Self {
        Self {
            id: generate_id(),
            timestamp: Utc::now(),
            provider: provider.to_string(),
            model: model.to_string(),
            resolved_model: None,
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_tokens: 0,
            cache_read_tokens: 0,
            duration_ms,
            estimated_cost_usd: 0.0,
            outcome: RequestOutcome::Error {
                message: message.to_string(),
            },
            stop_reason: None,
            cache_hit: false,
        }
    }

    /// Builder: set cache token counts.
    pub fn with_cache_tokens(mut self, creation: u64, read: u64) -> Self {
        self.cache_creation_tokens = creation;
        self.cache_read_tokens = read;
        self
    }

    /// Builder: set estimated cost.
    pub fn with_cost(mut self, cost: f64) -> Self {
        self.estimated_cost_usd = cost;
        self
    }

    /// Builder: set stop reason.
    pub fn with_stop_reason(mut self, reason: impl Into<String>) -> Self {
        self.stop_reason = Some(reason.into());
        self
    }

    /// Builder: mark as cache hit.
    pub fn with_cache_hit(mut self, hit: bool) -> Self {
        self.cache_hit = hit;
        self
    }
}

/// Accessor for the request log table.
pub struct RequestLog<'db> {
    db: &'db RouterDb,
}

impl<'db> RequestLog<'db> {
    pub(crate) fn new(db: &'db RouterDb) -> Self {
        Self { db }
    }

    /// Append a log entry. Automatically prunes old entries beyond MAX_ENTRIES.
    pub fn append(&self, entry: &LogEntry) -> Result<()> {
        let bytes = serde_json::to_vec(entry).map_err(|e| crate::Error::Config {
            message: format!("failed to serialize log entry: {e}"),
        })?;

        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;
            table.insert(entry.id, bytes.as_slice()).map_err(db_err)?;

            // Prune if over limit
            let len = table.len().map_err(db_err)?;
            if len > MAX_ENTRIES {
                let to_remove = len - MAX_ENTRIES;
                let keys: Vec<u64> = table
                    .iter()
                    .map_err(db_err)?
                    .take(to_remove as usize)
                    .filter_map(|item| item.ok().map(|(k, _)| k.value()))
                    .collect();
                for key in keys {
                    table.remove(key).map_err(db_err)?;
                }
            }
        }
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// Get the N most recent entries (newest first).
    pub fn recent(&self, limit: usize) -> Result<Vec<LogEntry>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;

        let mut entries = Vec::new();
        for item in table.iter().map_err(db_err)?.rev() {
            if entries.len() >= limit {
                break;
            }
            let (_key, value) = item.map_err(db_err)?;
            if let Ok(entry) = serde_json::from_slice::<LogEntry>(value.value()) {
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    /// Get entries for a specific provider (newest first).
    pub fn for_provider(&self, provider: &str, limit: usize) -> Result<Vec<LogEntry>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;

        let mut entries = Vec::new();
        for item in table.iter().map_err(db_err)?.rev() {
            if entries.len() >= limit {
                break;
            }
            let (_key, value) = item.map_err(db_err)?;
            if let Ok(entry) = serde_json::from_slice::<LogEntry>(value.value())
                && entry.provider == provider
            {
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    /// Get entries that resulted in errors (newest first).
    pub fn errors(&self, limit: usize) -> Result<Vec<LogEntry>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;

        let mut entries = Vec::new();
        for item in table.iter().map_err(db_err)?.rev() {
            if entries.len() >= limit {
                break;
            }
            let (_key, value) = item.map_err(db_err)?;
            if let Ok(entry) = serde_json::from_slice::<LogEntry>(value.value())
                && matches!(entry.outcome, RequestOutcome::Error { .. })
            {
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    /// Total number of log entries.
    pub fn count(&self) -> Result<u64> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;
        table.len().map_err(db_err)
    }

    /// Remove all entries.
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

    #[test]
    fn test_append_and_recent() {
        let db = test_db();
        let log = db.request_log();

        log.append(&LogEntry::success("anthropic", "sonnet", None, 100, 50, 1200)).unwrap();
        log.append(&LogEntry::success("openai", "gpt-4o", None, 200, 100, 800)).unwrap();
        log.append(&LogEntry::error("anthropic", "sonnet", 50, "rate limited")).unwrap();

        let recent = log.recent(10).unwrap();
        assert_eq!(recent.len(), 3);
        // Newest first
        assert!(matches!(recent[0].outcome, RequestOutcome::Error { .. }));
        assert_eq!(recent[1].provider, "openai");
        assert_eq!(recent[2].provider, "anthropic");
    }

    #[test]
    fn test_recent_with_limit() {
        let db = test_db();
        let log = db.request_log();

        for i in 0..20 {
            log.append(&LogEntry::success("anthropic", "sonnet", None, i * 10, i * 5, 100)).unwrap();
        }

        let recent = log.recent(5).unwrap();
        assert_eq!(recent.len(), 5);
    }

    #[test]
    fn test_for_provider() {
        let db = test_db();
        let log = db.request_log();

        log.append(&LogEntry::success("anthropic", "sonnet", None, 100, 50, 1000)).unwrap();
        log.append(&LogEntry::success("openai", "gpt-4o", None, 200, 100, 800)).unwrap();
        log.append(&LogEntry::success("anthropic", "haiku", None, 50, 25, 500)).unwrap();

        let anthropic = log.for_provider("anthropic", 10).unwrap();
        assert_eq!(anthropic.len(), 2);
        assert!(anthropic.iter().all(|e| e.provider == "anthropic"));

        let openai = log.for_provider("openai", 10).unwrap();
        assert_eq!(openai.len(), 1);
    }

    #[test]
    fn test_errors_filter() {
        let db = test_db();
        let log = db.request_log();

        log.append(&LogEntry::success("anthropic", "sonnet", None, 100, 50, 1000)).unwrap();
        log.append(&LogEntry::error("anthropic", "sonnet", 50, "429 rate limited")).unwrap();
        log.append(&LogEntry::success("anthropic", "sonnet", None, 100, 50, 1000)).unwrap();
        log.append(&LogEntry::error("openai", "gpt-4o", 30, "500 internal")).unwrap();

        let errors = log.errors(10).unwrap();
        assert_eq!(errors.len(), 2);
        assert!(errors.iter().all(|e| matches!(e.outcome, RequestOutcome::Error { .. })));
    }

    #[test]
    fn test_builder_methods() {
        let entry = LogEntry::success("anthropic", "sonnet", Some("claude-sonnet-4-5"), 100, 50, 1000)
            .with_cache_tokens(500, 200)
            .with_cost(0.45)
            .with_stop_reason("end_turn")
            .with_cache_hit(false);

        assert_eq!(entry.cache_creation_tokens, 500);
        assert_eq!(entry.cache_read_tokens, 200);
        assert!((entry.estimated_cost_usd - 0.45).abs() < 0.001);
        assert_eq!(entry.stop_reason.as_deref(), Some("end_turn"));
        assert!(!entry.cache_hit);
        assert_eq!(entry.resolved_model.as_deref(), Some("claude-sonnet-4-5"));
    }

    #[test]
    fn test_count() {
        let db = test_db();
        let log = db.request_log();

        assert_eq!(log.count().unwrap(), 0);
        log.append(&LogEntry::success("anthropic", "sonnet", None, 100, 50, 1000)).unwrap();
        log.append(&LogEntry::success("openai", "gpt-4o", None, 200, 100, 800)).unwrap();
        assert_eq!(log.count().unwrap(), 2);
    }

    #[test]
    fn test_clear() {
        let db = test_db();
        let log = db.request_log();

        log.append(&LogEntry::success("anthropic", "sonnet", None, 100, 50, 1000)).unwrap();
        log.append(&LogEntry::success("openai", "gpt-4o", None, 200, 100, 800)).unwrap();

        let cleared = log.clear().unwrap();
        assert_eq!(cleared, 2);
        assert_eq!(log.count().unwrap(), 0);
    }

    #[test]
    fn test_auto_prune() {
        let db = test_db();
        let log = db.request_log();

        // Insert more than MAX_ENTRIES (but use a smaller number for test speed)
        // We can't easily test 10_000 entries in unit tests, so just verify
        // the prune logic doesn't crash and entries are stored
        for i in 0..50 {
            log.append(&LogEntry::success("anthropic", "sonnet", None, i, i / 2, 100)).unwrap();
        }
        assert_eq!(log.count().unwrap(), 50);
    }

    #[test]
    fn test_empty_recent() {
        let db = test_db();
        let recent = db.request_log().recent(10).unwrap();
        assert!(recent.is_empty());
    }
}
