//! Tool result content store.
//!
//! Persists full tool result content to redb so that in-memory
//! conversation history can carry compact summaries while the
//! original output remains recoverable. Keyed by session + call_id.

use redb::ReadableTableMetadata;
use redb::TableDefinition;
use serde::Deserialize;
use serde::Serialize;

use super::Db;
use crate::error::Result;
use crate::error::db_err;

pub(crate) const TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("tool_result_content");

/// A stored tool result with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredToolResult {
    pub session_id: String,
    pub call_id: String,
    pub tool_name: String,
    /// The original text content (concatenated from all Content::Text blocks)
    pub content_text: String,
    /// Whether the result had image content
    pub has_image: bool,
    /// Whether the tool returned an error
    pub is_error: bool,
    /// Total byte count of original content
    pub byte_count: usize,
    /// Line count of original content
    pub line_count: usize,
}

pub struct ToolResultStore<'db> {
    db: &'db Db,
}

impl<'db> ToolResultStore<'db> {
    pub(crate) fn new(db: &'db Db) -> Self {
        Self { db }
    }

    /// Store a tool result.
    pub fn store(&self, entry: &StoredToolResult) -> Result<()> {
        let key = format!("{}:{}", entry.session_id, entry.call_id);
        let bytes = serde_json::to_vec(entry).map_err(|e| crate::error::DbError {
            message: format!("failed to serialize tool result: {e}"),
        })?;

        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;
            table.insert(key.as_str(), bytes.as_slice()).map_err(db_err)?;
        }
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// Retrieve a tool result by session and call ID.
    pub fn get(&self, session_id: &str, call_id: &str) -> Result<Option<StoredToolResult>> {
        let key = format!("{session_id}:{call_id}");
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;
        match table.get(key.as_str()).map_err(db_err)? {
            Some(value) => {
                let entry = serde_json::from_slice(value.value()).map_err(|e| crate::error::DbError {
                    message: format!("failed to deserialize tool result: {e}"),
                })?;
                Ok(Some(entry))
            }
            None => Ok(None),
        }
    }

    /// List all tool results for a session.
    pub fn for_session(&self, session_id: &str) -> Result<Vec<StoredToolResult>> {
        let prefix = format!("{session_id}:");
        let end = format!("{session_id};"); // ';' is ':' + 1
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;

        let mut entries = Vec::new();
        for item in table.range(prefix.as_str()..end.as_str()).map_err(db_err)? {
            let (_key, value) = item.map_err(db_err)?;
            if let Ok(entry) = serde_json::from_slice::<StoredToolResult>(value.value()) {
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    /// Count results for a session.
    pub fn count_for_session(&self, session_id: &str) -> Result<usize> {
        Ok(self.for_session(session_id)?.len())
    }

    /// Count total stored results.
    pub fn count(&self) -> Result<u64> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;
        table.len().map_err(db_err)
    }

    /// Remove all results for a session.
    pub fn clear_session(&self, session_id: &str) -> Result<usize> {
        let entries = self.for_session(session_id)?;
        let count = entries.len();
        if count == 0 {
            return Ok(0);
        }

        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;
            for entry in &entries {
                let key = format!("{}:{}", entry.session_id, entry.call_id);
                table.remove(key.as_str()).map_err(db_err)?;
            }
        }
        tx.commit().map_err(db_err)?;
        Ok(count)
    }

    /// Remove all results (for testing).
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

    fn make_tool_result(session_id: &str, call_id: &str, tool_name: &str) -> StoredToolResult {
        StoredToolResult {
            session_id: session_id.to_string(),
            call_id: call_id.to_string(),
            tool_name: tool_name.to_string(),
            content_text: "test content".to_string(),
            has_image: false,
            is_error: false,
            byte_count: 12,
            line_count: 1,
        }
    }

    fn make_bash_result(session_id: &str, call_id: &str, content: &str) -> StoredToolResult {
        StoredToolResult {
            session_id: session_id.to_string(),
            call_id: call_id.to_string(),
            tool_name: "bash".to_string(),
            content_text: content.to_string(),
            has_image: false,
            is_error: false,
            byte_count: content.len(),
            line_count: content.lines().count(),
        }
    }

    // ── Basic CRUD ──────────────────────────────────────────────────

    #[test]
    fn store_and_get_roundtrip() -> Result<()> {
        let db = test_db()?;
        let store = db.tool_results();

        let result = make_tool_result("sess-1", "call-123", "read");
        store.store(&result)?;

        let retrieved = store.get("sess-1", "call-123")?;
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.session_id, "sess-1");
        assert_eq!(retrieved.call_id, "call-123");
        assert_eq!(retrieved.tool_name, "read");
        assert_eq!(retrieved.content_text, "test content");
        assert_eq!(retrieved.byte_count, 12);
        Ok(())
    }

    #[test]
    fn get_nonexistent_returns_none() -> Result<()> {
        let db = test_db()?;
        let store = db.tool_results();

        let result = store.get("sess-1", "call-999")?;
        assert!(result.is_none());
        Ok(())
    }

    #[test]
    fn overwrite_same_key() -> Result<()> {
        let db = test_db()?;
        let store = db.tool_results();

        let result1 = make_tool_result("sess-1", "call-1", "read");
        store.store(&result1)?;

        let mut result2 = make_tool_result("sess-1", "call-1", "write");
        result2.content_text = "different content".to_string();
        store.store(&result2)?;

        let retrieved = store.get("sess-1", "call-1")?.unwrap();
        assert_eq!(retrieved.tool_name, "write");
        assert_eq!(retrieved.content_text, "different content");
        Ok(())
    }

    // ── Range scan for_session ──────────────────────────────────────

    #[test]
    fn for_session_returns_all_results() -> Result<()> {
        let db = test_db()?;
        let store = db.tool_results();

        store.store(&make_tool_result("sess-1", "call-1", "read"))?;
        store.store(&make_tool_result("sess-1", "call-2", "write"))?;
        store.store(&make_tool_result("sess-1", "call-3", "bash"))?;
        store.store(&make_tool_result("sess-2", "call-1", "edit"))?;

        let sess1_results = store.for_session("sess-1")?;
        assert_eq!(sess1_results.len(), 3);

        let sess2_results = store.for_session("sess-2")?;
        assert_eq!(sess2_results.len(), 1);

        let sess3_results = store.for_session("sess-3")?;
        assert_eq!(sess3_results.len(), 0);
        Ok(())
    }

    #[test]
    fn session_isolation() -> Result<()> {
        let db = test_db()?;
        let store = db.tool_results();

        store.store(&make_tool_result("sess-a", "call-1", "read"))?;
        store.store(&make_tool_result("sess-b", "call-1", "write"))?;

        let sess_a = store.for_session("sess-a")?;
        assert_eq!(sess_a.len(), 1);
        assert_eq!(sess_a[0].tool_name, "read");

        let sess_b = store.for_session("sess-b")?;
        assert_eq!(sess_b.len(), 1);
        assert_eq!(sess_b[0].tool_name, "write");
        Ok(())
    }

    // ── Counting ────────────────────────────────────────────────────

    #[test]
    fn count_for_session() -> Result<()> {
        let db = test_db()?;
        let store = db.tool_results();

        assert_eq!(store.count_for_session("sess-1")?, 0);

        store.store(&make_tool_result("sess-1", "call-1", "read"))?;
        store.store(&make_tool_result("sess-1", "call-2", "write"))?;

        assert_eq!(store.count_for_session("sess-1")?, 2);
        assert_eq!(store.count_for_session("sess-2")?, 0);
        Ok(())
    }

    #[test]
    fn count_total() -> Result<()> {
        let db = test_db()?;
        let store = db.tool_results();

        assert_eq!(store.count()?, 0);

        store.store(&make_tool_result("sess-1", "call-1", "read"))?;
        store.store(&make_tool_result("sess-2", "call-1", "write"))?;

        assert_eq!(store.count()?, 2);
        Ok(())
    }

    // ── Clear operations ────────────────────────────────────────────

    #[test]
    fn clear_session() -> Result<()> {
        let db = test_db()?;
        let store = db.tool_results();

        store.store(&make_tool_result("sess-1", "call-1", "read"))?;
        store.store(&make_tool_result("sess-1", "call-2", "write"))?;
        store.store(&make_tool_result("sess-2", "call-1", "bash"))?;

        let cleared = store.clear_session("sess-1")?;
        assert_eq!(cleared, 2);

        assert_eq!(store.for_session("sess-1")?.len(), 0);
        assert_eq!(store.for_session("sess-2")?.len(), 1);
        Ok(())
    }

    #[test]
    fn clear_nonexistent_session() -> Result<()> {
        let db = test_db()?;
        let store = db.tool_results();

        let cleared = store.clear_session("nonexistent")?;
        assert_eq!(cleared, 0);
        Ok(())
    }

    #[test]
    fn clear_all() -> Result<()> {
        let db = test_db()?;
        let store = db.tool_results();

        store.store(&make_tool_result("sess-1", "call-1", "read"))?;
        store.store(&make_tool_result("sess-2", "call-1", "write"))?;

        let cleared = store.clear()?;
        assert_eq!(cleared, 2);
        assert_eq!(store.count()?, 0);
        Ok(())
    }

    // ── Unicode content ─────────────────────────────────────────────

    #[test]
    fn unicode_content() -> Result<()> {
        let db = test_db()?;
        let store = db.tool_results();

        let mut result = make_tool_result("sess-日本語", "call-🚀", "bash");
        result.content_text = "🚀 héllo wörld 日本語".to_string();
        result.byte_count = result.content_text.len();

        store.store(&result)?;

        let retrieved = store.get("sess-日本語", "call-🚀")?.unwrap();
        assert_eq!(retrieved.content_text, "🚀 héllo wörld 日本語");
        Ok(())
    }

    // ── Large content ───────────────────────────────────────────────

    #[test]
    fn large_content() -> Result<()> {
        let db = test_db()?;
        let store = db.tool_results();

        let large_content = "x".repeat(100_000);
        let result = make_bash_result("sess-1", "call-1", &large_content);

        store.store(&result)?;

        let retrieved = store.get("sess-1", "call-1")?.unwrap();
        assert_eq!(retrieved.content_text.len(), 100_000);
        assert_eq!(retrieved.byte_count, 100_000);
        Ok(())
    }

    // ── Clear then reuse ────────────────────────────────────────────

    #[test]
    fn clear_then_reuse() -> Result<()> {
        let db = test_db()?;
        let store = db.tool_results();

        store.store(&make_tool_result("sess-1", "call-1", "read"))?;
        store.clear()?;

        // Can still write after clear
        store.store(&make_tool_result("sess-1", "call-1", "write"))?;
        let results = store.for_session("sess-1")?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tool_name, "write");
        Ok(())
    }

    // ── Error and image flags ───────────────────────────────────────

    #[test]
    fn error_and_image_flags() -> Result<()> {
        let db = test_db()?;
        let store = db.tool_results();

        let mut result = make_tool_result("sess-1", "call-1", "bash");
        result.is_error = true;
        result.has_image = true;
        result.content_text = "command not found".to_string();

        store.store(&result)?;

        let retrieved = store.get("sess-1", "call-1")?.unwrap();
        assert!(retrieved.is_error);
        assert!(retrieved.has_image);
        assert_eq!(retrieved.content_text, "command not found");
        Ok(())
    }

    // ── Serialization roundtrip ─────────────────────────────────────

    #[test]
    fn serialization_roundtrip() -> Result<()> {
        let result = StoredToolResult {
            session_id: "test-session".to_string(),
            call_id: "call-42".to_string(),
            tool_name: "bash".to_string(),
            content_text: "total 42\ndrwxr-xr-x ...".to_string(),
            has_image: false,
            is_error: false,
            byte_count: 25,
            line_count: 2,
        };

        let json = serde_json::to_string(&result).map_err(|e| crate::error::DbError {
            message: format!("serialization failed: {e}"),
        })?;
        let parsed: StoredToolResult = serde_json::from_str(&json).map_err(|e| crate::error::DbError {
            message: format!("deserialization failed: {e}"),
        })?;

        assert_eq!(parsed.session_id, "test-session");
        assert_eq!(parsed.call_id, "call-42");
        assert_eq!(parsed.tool_name, "bash");
        assert_eq!(parsed.content_text, "total 42\ndrwxr-xr-x ...");
        assert_eq!(parsed.byte_count, 25);
        assert_eq!(parsed.line_count, 2);
        assert!(!parsed.has_image);
        assert!(!parsed.is_error);
        Ok(())
    }

    // ── Empty content ───────────────────────────────────────────────

    #[test]
    fn empty_content() -> Result<()> {
        let db = test_db()?;
        let store = db.tool_results();

        let mut result = make_tool_result("sess-1", "call-1", "write");
        result.content_text = String::new();
        result.byte_count = 0;
        result.line_count = 0;

        store.store(&result)?;

        let retrieved = store.get("sess-1", "call-1")?.unwrap();
        assert_eq!(retrieved.content_text, "");
        assert_eq!(retrieved.byte_count, 0);
        assert_eq!(retrieved.line_count, 0);
        Ok(())
    }

    // ── Line counting edge cases ────────────────────────────────────

    #[test]
    fn line_counting() -> Result<()> {
        let db = test_db()?;
        let store = db.tool_results();

        let test_cases = vec![
            ("", 0),
            ("single line", 1),
            ("line 1\nline 2", 2),
            ("line 1\nline 2\n", 2),
            ("line 1\nline 2\nline 3", 3),
        ];

        for (i, (content, expected_lines)) in test_cases.iter().enumerate() {
            let result = make_bash_result("sess-1", &format!("call-{}", i), content);
            store.store(&result)?;

            let retrieved = store.get("sess-1", &format!("call-{}", i))?.unwrap();
            assert_eq!(retrieved.line_count, *expected_lines, "content: {:?}", content);
        }
        Ok(())
    }
}
