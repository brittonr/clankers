//! Session index — fast lookup without scanning JSONL files.
//!
//! The session JSONL files remain the source of truth. This table is a
//! read-optimized index populated on session create/close. Listing and
//! searching sessions queries redb instead of stat-ing thousands of files.

use std::cmp::Reverse;

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

/// Table: session_id (string) → serialized SessionIndexEntry
pub(crate) const TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("session_index");

/// Indexed metadata about a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionIndexEntry {
    /// Unique session ID.
    pub session_id: String,
    /// Working directory the session ran in.
    pub cwd: String,
    /// Model used.
    pub model: String,
    /// When the session started.
    pub created_at: DateTime<Utc>,
    /// Approximate message count (updated on close).
    pub message_count: u32,
    /// First 120 chars of the first user prompt (for preview).
    pub first_prompt: String,
    /// Path to the JSONL file on disk.
    pub file_path: String,
    /// Agent name (if the session used a named agent).
    pub agent: Option<String>,
    /// When the index entry was last updated.
    pub updated_at: DateTime<Utc>,
}

/// Accessor for the session index table.
pub struct SessionIndex<'db> {
    db: &'db Db,
}

impl<'db> SessionIndex<'db> {
    pub(crate) fn new(db: &'db Db) -> Self {
        Self { db }
    }

    /// Insert or update a session index entry.
    pub fn upsert(&self, entry: &SessionIndexEntry) -> Result<()> {
        let bytes = serde_json::to_vec(entry).map_err(|e| crate::error::Error::Database {
            message: format!("failed to serialize session index entry: {e}"),
        })?;

        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;
            table.insert(entry.session_id.as_str(), bytes.as_slice()).map_err(db_err)?;
        }
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// Get a session by ID.
    pub fn get(&self, session_id: &str) -> Result<Option<SessionIndexEntry>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;
        match table.get(session_id).map_err(db_err)? {
            Some(value) => {
                let entry = serde_json::from_slice(value.value()).map_err(|e| crate::error::Error::Database {
                    message: format!("failed to deserialize session index: {e}"),
                })?;
                Ok(Some(entry))
            }
            None => Ok(None),
        }
    }

    /// Remove a session from the index.
    pub fn remove(&self, session_id: &str) -> Result<bool> {
        let tx = self.db.begin_write()?;
        let removed = {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;
            table.remove(session_id).map_err(db_err)?.is_some()
        };
        tx.commit().map_err(db_err)?;
        Ok(removed)
    }

    /// List sessions for a given cwd, newest first.
    pub fn list_by_cwd(&self, cwd: &str) -> Result<Vec<SessionIndexEntry>> {
        let mut entries = Vec::new();
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;

        for item in table.iter().map_err(db_err)? {
            let (_key, value) = item.map_err(db_err)?;
            if let Ok(entry) = serde_json::from_slice::<SessionIndexEntry>(value.value())
                && entry.cwd == cwd
            {
                entries.push(entry);
            }
        }

        // Newest first
        entries.sort_by_key(|e| Reverse(e.created_at));
        Ok(entries)
    }

    /// List all sessions, newest first.
    pub fn list_all(&self) -> Result<Vec<SessionIndexEntry>> {
        let mut entries = Vec::new();
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;

        for item in table.iter().map_err(db_err)? {
            let (_key, value) = item.map_err(db_err)?;
            if let Ok(entry) = serde_json::from_slice::<SessionIndexEntry>(value.value()) {
                entries.push(entry);
            }
        }

        entries.sort_by_key(|e| Reverse(e.created_at));
        Ok(entries)
    }

    /// Search sessions by substring in the first prompt (case-insensitive).
    pub fn search(&self, query: &str) -> Result<Vec<SessionIndexEntry>> {
        let lower = query.to_lowercase();
        let mut entries = Vec::new();
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;

        for item in table.iter().map_err(db_err)? {
            let (_key, value) = item.map_err(db_err)?;
            if let Ok(entry) = serde_json::from_slice::<SessionIndexEntry>(value.value())
                && (entry.first_prompt.to_lowercase().contains(&lower)
                    || entry.session_id.contains(&lower)
                    || entry.model.to_lowercase().contains(&lower))
            {
                entries.push(entry);
            }
        }

        entries.sort_by_key(|e| Reverse(e.created_at));
        Ok(entries)
    }

    /// Find a session by partial ID match.
    pub fn find_by_partial_id(&self, partial: &str) -> Result<Option<SessionIndexEntry>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;

        for item in table.iter().map_err(db_err)? {
            let (key, value) = item.map_err(db_err)?;
            if key.value().contains(partial)
                && let Ok(entry) = serde_json::from_slice::<SessionIndexEntry>(value.value())
            {
                return Ok(Some(entry));
            }
        }
        Ok(None)
    }

    /// Total number of indexed sessions.
    pub fn count(&self) -> Result<u64> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;
        table.len().map_err(db_err)
    }

    /// Remove all entries (for testing / re-index).
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

    fn make_entry(id: &str, cwd: &str, prompt: &str) -> SessionIndexEntry {
        SessionIndexEntry {
            session_id: id.to_string(),
            cwd: cwd.to_string(),
            model: "claude-sonnet".to_string(),
            created_at: Utc::now(),
            message_count: 5,
            first_prompt: prompt.to_string(),
            file_path: format!("/sessions/{id}.jsonl"),
            agent: None,
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_upsert_and_get() -> Result<()> {
        let db = test_db()?;
        let idx = db.sessions();

        let entry = make_entry("abc123", "/home/user/proj", "fix the bug");
        idx.upsert(&entry)?;

        let got = idx.get("abc123")?.expect("entry should exist");
        assert_eq!(got.session_id, "abc123");
        assert_eq!(got.cwd, "/home/user/proj");
        assert_eq!(got.first_prompt, "fix the bug");
        Ok(())
    }

    #[test]
    fn test_upsert_overwrites() -> Result<()> {
        let db = test_db()?;
        let idx = db.sessions();

        let mut entry = make_entry("abc123", "/proj", "original");
        idx.upsert(&entry)?;

        entry.message_count = 20;
        entry.first_prompt = "updated".into();
        idx.upsert(&entry)?;

        let got = idx.get("abc123")?.expect("entry should exist");
        assert_eq!(got.message_count, 20);
        assert_eq!(got.first_prompt, "updated");
        Ok(())
    }

    #[test]
    fn test_get_missing() -> Result<()> {
        let db = test_db()?;
        assert!(db.sessions().get("nonexistent")?.is_none());
        Ok(())
    }

    #[test]
    fn test_remove() -> Result<()> {
        let db = test_db()?;
        let idx = db.sessions();

        idx.upsert(&make_entry("abc", "/proj", "prompt"))?;
        assert!(idx.remove("abc")?);
        assert!(!idx.remove("abc")?);
        assert!(idx.get("abc")?.is_none());
        Ok(())
    }

    #[test]
    fn test_list_by_cwd() -> Result<()> {
        let db = test_db()?;
        let idx = db.sessions();

        idx.upsert(&make_entry("s1", "/proj-a", "prompt 1"))?;
        idx.upsert(&make_entry("s2", "/proj-a", "prompt 2"))?;
        idx.upsert(&make_entry("s3", "/proj-b", "prompt 3"))?;

        let proj_a = idx.list_by_cwd("/proj-a")?;
        assert_eq!(proj_a.len(), 2);

        let proj_b = idx.list_by_cwd("/proj-b")?;
        assert_eq!(proj_b.len(), 1);

        let proj_c = idx.list_by_cwd("/proj-c")?;
        assert!(proj_c.is_empty());
        Ok(())
    }

    #[test]
    fn test_list_all() -> Result<()> {
        let db = test_db()?;
        let idx = db.sessions();

        idx.upsert(&make_entry("s1", "/a", "p1"))?;
        idx.upsert(&make_entry("s2", "/b", "p2"))?;

        let all = idx.list_all()?;
        assert_eq!(all.len(), 2);
        Ok(())
    }

    #[test]
    fn test_search() -> Result<()> {
        let db = test_db()?;
        let idx = db.sessions();

        idx.upsert(&make_entry("s1", "/proj", "fix the login bug"))?;
        idx.upsert(&make_entry("s2", "/proj", "add new feature"))?;
        idx.upsert(&make_entry("s3", "/proj", "refactor auth module"))?;

        let results = idx.search("bug")?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].session_id, "s1");

        let results = idx.search("auth")?;
        assert_eq!(results.len(), 1);
        Ok(())
    }

    #[test]
    fn test_search_by_session_id() -> Result<()> {
        let db = test_db()?;
        let idx = db.sessions();

        idx.upsert(&make_entry("abc123", "/proj", "some prompt"))?;

        let results = idx.search("abc")?;
        assert_eq!(results.len(), 1);
        Ok(())
    }

    #[test]
    fn test_find_by_partial_id() -> Result<()> {
        let db = test_db()?;
        let idx = db.sessions();

        idx.upsert(&make_entry("abc123def", "/proj", "prompt"))?;
        idx.upsert(&make_entry("xyz789ghi", "/proj", "prompt"))?;

        let found = idx.find_by_partial_id("abc123")?.expect("entry should exist");
        assert_eq!(found.session_id, "abc123def");

        assert!(idx.find_by_partial_id("qqq")?.is_none());
        Ok(())
    }

    #[test]
    fn test_count() -> Result<()> {
        let db = test_db()?;
        let idx = db.sessions();

        assert_eq!(idx.count()?, 0);
        idx.upsert(&make_entry("s1", "/a", "p"))?;
        idx.upsert(&make_entry("s2", "/b", "p"))?;
        assert_eq!(idx.count()?, 2);
        Ok(())
    }

    #[test]
    fn test_clear() -> Result<()> {
        let db = test_db()?;
        let idx = db.sessions();

        idx.upsert(&make_entry("s1", "/a", "p"))?;
        idx.upsert(&make_entry("s2", "/b", "p"))?;

        let cleared = idx.clear()?;
        assert_eq!(cleared, 2);
        assert_eq!(idx.count()?, 0);
        Ok(())
    }
}
