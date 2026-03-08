//! Prompt history — replaces the JSONL-backed `HistoryStore`.
//!
//! Stores every user prompt with its session and cwd. Supports:
//! - Reverse-chronological listing (for Ctrl+R)
//! - Substring search (case-insensitive)
//! - Per-project filtering
//! - No cap on entries (redb handles the size)

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

/// Table: timestamp_micros (u64) → serialized HistoryEntry
///
/// Using timestamp as key gives us natural chronological ordering
/// via redb's sorted iteration.
pub(crate) const TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new("prompt_history");

/// A single prompt history entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Prompt text.
    pub text: String,
    /// When this was submitted.
    pub timestamp: DateTime<Utc>,
    /// Session that produced it.
    pub session_id: String,
    /// Working directory at the time.
    pub cwd: String,
}

/// Accessor for the prompt history table.
pub struct HistoryDb<'db> {
    db: &'db Db,
}

impl<'db> HistoryDb<'db> {
    pub(crate) fn new(db: &'db Db) -> Self {
        Self { db }
    }

    /// Add a prompt to history. Deduplicates consecutive identical prompts.
    pub fn add(&self, text: &str, session_id: &str, cwd: &str) -> Result<()> {
        // Check last entry for dedup
        if let Some(last) = self.most_recent(1)?.first()
            && last.text == text
        {
            return Ok(());
        }

        let entry = HistoryEntry {
            text: text.to_string(),
            timestamp: Utc::now(),
            session_id: session_id.to_string(),
            cwd: cwd.to_string(),
        };
        let key = crate::db::memory::generate_id();
        let bytes = serde_json::to_vec(&entry).map_err(|e| crate::error::Error::Database {
            message: format!("failed to serialize history entry: {e}"),
        })?;

        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;
            table.insert(key, bytes.as_slice()).map_err(db_err)?;
        }
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// Get the N most recent entries (newest first).
    pub fn most_recent(&self, limit: usize) -> Result<Vec<HistoryEntry>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;

        let mut entries = Vec::new();
        // Reverse iteration: redb range returns ascending, so we use rev()
        for item in table.iter().map_err(db_err)?.rev() {
            if entries.len() >= limit {
                break;
            }
            let (_key, value) = item.map_err(db_err)?;
            if let Ok(entry) = serde_json::from_slice::<HistoryEntry>(value.value()) {
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    /// Search history by substring (case-insensitive), newest first.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<HistoryEntry>> {
        if query.is_empty() {
            return self.most_recent(limit);
        }

        let lower = query.to_lowercase();
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;

        let mut entries = Vec::new();
        for item in table.iter().map_err(db_err)?.rev() {
            if entries.len() >= limit {
                break;
            }
            let (_key, value) = item.map_err(db_err)?;
            if let Ok(entry) = serde_json::from_slice::<HistoryEntry>(value.value())
                && entry.text.to_lowercase().contains(&lower)
            {
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    /// Search history filtered by cwd (for project-relevant results).
    pub fn search_in_cwd(&self, query: &str, cwd: &str, limit: usize) -> Result<Vec<HistoryEntry>> {
        let lower = query.to_lowercase();
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;

        let mut entries = Vec::new();
        for item in table.iter().map_err(db_err)?.rev() {
            if entries.len() >= limit {
                break;
            }
            let (_key, value) = item.map_err(db_err)?;
            if let Ok(entry) = serde_json::from_slice::<HistoryEntry>(value.value())
                && entry.cwd == cwd
                && (query.is_empty() || entry.text.to_lowercase().contains(&lower))
            {
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    /// Total number of history entries.
    pub fn count(&self) -> Result<u64> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;
        table.len().map_err(db_err)
    }

    /// Remove all entries (for testing / reset).
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

    fn test_db() -> Result<Db> {
        Db::in_memory()
    }

    #[test]
    fn test_add_and_recent() -> Result<()> {
        let db = test_db()?;
        let hist = db.history();

        hist.add("hello world", "sess1", "/proj")?;
        hist.add("fix the bug", "sess1", "/proj")?;
        hist.add("add feature", "sess2", "/proj")?;

        let recent = hist.most_recent(10)?;
        assert_eq!(recent.len(), 3);
        // Newest first
        assert_eq!(recent[0].text, "add feature");
        assert_eq!(recent[1].text, "fix the bug");
        assert_eq!(recent[2].text, "hello world");
        Ok(())
    }

    #[test]
    fn test_recent_with_limit() -> Result<()> {
        let db = test_db()?;
        let hist = db.history();

        for i in 0..20 {
            hist.add(&format!("prompt {i}"), "s1", "/proj")?;
        }

        let recent = hist.most_recent(5)?;
        assert_eq!(recent.len(), 5);
        assert_eq!(recent[0].text, "prompt 19");
        Ok(())
    }

    #[test]
    fn test_dedup_consecutive() -> Result<()> {
        let db = test_db()?;
        let hist = db.history();

        hist.add("same prompt", "s1", "/proj")?;
        hist.add("same prompt", "s1", "/proj")?;
        hist.add("same prompt", "s1", "/proj")?;

        assert_eq!(hist.count()?, 1);
        Ok(())
    }

    #[test]
    fn test_dedup_allows_non_consecutive() -> Result<()> {
        let db = test_db()?;
        let hist = db.history();

        hist.add("first", "s1", "/proj")?;
        hist.add("second", "s1", "/proj")?;
        hist.add("first", "s1", "/proj")?; // not consecutive dup

        assert_eq!(hist.count()?, 3);
        Ok(())
    }

    #[test]
    fn test_search() -> Result<()> {
        let db = test_db()?;
        let hist = db.history();

        hist.add("fix the login bug", "s1", "/proj")?;
        hist.add("add new feature", "s1", "/proj")?;
        hist.add("fix the signup bug", "s2", "/proj")?;

        let results = hist.search("bug", 10)?;
        assert_eq!(results.len(), 2);
        // Newest first
        assert!(results[0].text.contains("signup"));
        assert!(results[1].text.contains("login"));
        Ok(())
    }

    #[test]
    fn test_search_case_insensitive() -> Result<()> {
        let db = test_db()?;
        let hist = db.history();

        hist.add("Fix the BUG", "s1", "/proj")?;

        assert_eq!(hist.search("bug", 10)?.len(), 1);
        assert_eq!(hist.search("FIX", 10)?.len(), 1);
        Ok(())
    }

    #[test]
    fn test_search_empty_returns_recent() -> Result<()> {
        let db = test_db()?;
        let hist = db.history();

        hist.add("one", "s1", "/proj")?;
        hist.add("two", "s1", "/proj")?;

        let results = hist.search("", 10)?;
        assert_eq!(results.len(), 2);
        Ok(())
    }

    #[test]
    fn test_search_in_cwd() -> Result<()> {
        let db = test_db()?;
        let hist = db.history();

        hist.add("fix bug", "s1", "/proj-a")?;
        hist.add("fix bug", "s1", "/proj-b")?;
        hist.add("add feature", "s1", "/proj-a")?;

        let results = hist.search_in_cwd("fix", "/proj-a", 10)?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].cwd, "/proj-a");
        Ok(())
    }

    #[test]
    fn test_count() -> Result<()> {
        let db = test_db()?;
        let hist = db.history();

        assert_eq!(hist.count()?, 0);
        hist.add("one", "s1", "/proj")?;
        hist.add("two", "s1", "/proj")?;
        assert_eq!(hist.count()?, 2);
        Ok(())
    }

    #[test]
    fn test_clear() -> Result<()> {
        let db = test_db()?;
        let hist = db.history();

        hist.add("one", "s1", "/proj")?;
        hist.add("two", "s1", "/proj")?;

        let cleared = hist.clear()?;
        assert_eq!(cleared, 2);
        assert_eq!(hist.count()?, 0);
        Ok(())
    }
}
