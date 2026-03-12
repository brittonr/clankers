//! Session-scoped file read cache.
//!
//! Caches file content in redb to avoid redundant disk reads when the
//! model reads the same file multiple times within a session. Uses
//! mtime + size for staleness detection.

use redb::ReadableTableMetadata;
use redb::TableDefinition;
use serde::Deserialize;
use serde::Serialize;

use super::Db;
use crate::error::Result;
use crate::error::db_err;

pub(crate) const TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("file_read_cache");

/// Cached file read with staleness metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedFileRead {
    pub session_id: String,
    pub path: String,
    /// File modification time (seconds since epoch)
    pub mtime_secs: i64,
    /// File size in bytes
    pub file_size: u64,
    /// Total line count
    pub line_count: usize,
    /// Number of times this cache entry was hit
    pub hit_count: u32,
}

/// Result of a cache lookup.
#[derive(Debug)]
pub enum CacheLookup {
    /// Cache hit — file hasn't changed
    Hit(CachedFileRead),
    /// Cache miss — no entry or file changed
    Miss,
}

pub struct FileReadCache<'db> {
    db: &'db Db,
}

impl<'db> FileReadCache<'db> {
    pub(crate) fn new(db: &'db Db) -> Self {
        Self { db }
    }

    /// Check if we have a valid cache entry for this file.
    /// Returns Hit if the file mtime and size match, Miss otherwise.
    pub fn check(&self, session_id: &str, path: &str, current_mtime: i64, current_size: u64) -> Result<CacheLookup> {
        let key = make_key(session_id, path);
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;

        match table.get(key.as_str()).map_err(db_err)? {
            Some(value) => {
                let entry: CachedFileRead =
                    serde_json::from_slice(value.value()).map_err(|e| crate::error::DbError {
                        message: format!("failed to deserialize file cache entry: {e}"),
                    })?;
                if entry.mtime_secs == current_mtime && entry.file_size == current_size {
                    Ok(CacheLookup::Hit(entry))
                } else {
                    Ok(CacheLookup::Miss)
                }
            }
            None => Ok(CacheLookup::Miss),
        }
    }

    /// Record a file read (insert or update cache entry).
    pub fn record(&self, entry: &CachedFileRead) -> Result<()> {
        let key = make_key(&entry.session_id, &entry.path);
        let bytes = serde_json::to_vec(entry).map_err(|e| crate::error::DbError {
            message: format!("failed to serialize file cache entry: {e}"),
        })?;

        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;
            table.insert(key.as_str(), bytes.as_slice()).map_err(db_err)?;
        }
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// Increment the hit count for a cache entry.
    pub fn record_hit(&self, session_id: &str, path: &str) -> Result<()> {
        let key = make_key(session_id, path);

        // First read the current entry
        let current_entry = {
            let tx = self.db.begin_read()?;
            let table = tx.open_table(TABLE).map_err(db_err)?;
            if let Some(value) = table.get(key.as_str()).map_err(db_err)? {
                serde_json::from_slice::<CachedFileRead>(value.value()).map_err(|e| crate::error::DbError {
                    message: format!("failed to deserialize file cache entry: {e}"),
                })?
            } else {
                return Ok(()); // Entry doesn't exist, nothing to do
            }
        };

        // Then update it
        let mut updated_entry = current_entry;
        updated_entry.hit_count += 1;

        let bytes = serde_json::to_vec(&updated_entry).map_err(|e| crate::error::DbError {
            message: format!("failed to serialize file cache entry: {e}"),
        })?;

        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;
            table.insert(key.as_str(), bytes.as_slice()).map_err(db_err)?;
        }
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// List all cached files for a session.
    pub fn for_session(&self, session_id: &str) -> Result<Vec<CachedFileRead>> {
        let prefix = format!("{session_id}:");
        let end = format!("{session_id};");
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;

        let mut entries = Vec::new();
        for item in table.range(prefix.as_str()..end.as_str()).map_err(db_err)? {
            let (_key, value) = item.map_err(db_err)?;
            if let Ok(entry) = serde_json::from_slice::<CachedFileRead>(value.value()) {
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    /// Count total cached entries.
    pub fn count(&self) -> Result<u64> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;
        table.len().map_err(db_err)
    }

    /// Clear all entries for a session.
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
                let key = make_key(&entry.session_id, &entry.path);
                table.remove(key.as_str()).map_err(db_err)?;
            }
        }
        tx.commit().map_err(db_err)?;
        Ok(count)
    }

    /// Remove all entries (for testing).
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

/// Build the cache key from session ID and file path.
/// Uses a hash of the path to keep keys short and avoid special characters.
fn make_key(session_id: &str, path: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hash;
    use std::hash::Hasher;
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    format!("{}:{:016x}", session_id, hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Result<Db> {
        Db::in_memory()
    }

    fn make_cache_entry(session_id: &str, path: &str, mtime: i64, size: u64) -> CachedFileRead {
        CachedFileRead {
            session_id: session_id.to_string(),
            path: path.to_string(),
            mtime_secs: mtime,
            file_size: size,
            line_count: 42,
            hit_count: 1,
        }
    }

    // ── Basic CRUD ──────────────────────────────────────────────────

    #[test]
    fn store_and_check_roundtrip() -> Result<()> {
        let db = test_db()?;
        let cache = db.file_cache();

        let entry = make_cache_entry("sess-1", "/path/to/file.rs", 1234567890, 1024);
        cache.record(&entry)?;

        // Cache hit with matching mtime and size
        match cache.check("sess-1", "/path/to/file.rs", 1234567890, 1024)? {
            CacheLookup::Hit(retrieved) => {
                assert_eq!(retrieved.session_id, "sess-1");
                assert_eq!(retrieved.path, "/path/to/file.rs");
                assert_eq!(retrieved.mtime_secs, 1234567890);
                assert_eq!(retrieved.file_size, 1024);
                assert_eq!(retrieved.line_count, 42);
                assert_eq!(retrieved.hit_count, 1);
            }
            CacheLookup::Miss => panic!("Expected cache hit"),
        }
        Ok(())
    }

    #[test]
    fn cache_hit_with_matching_mtime_and_size() -> Result<()> {
        let db = test_db()?;
        let cache = db.file_cache();

        let entry = make_cache_entry("sess-1", "/file.txt", 1000, 500);
        cache.record(&entry)?;

        match cache.check("sess-1", "/file.txt", 1000, 500)? {
            CacheLookup::Hit(_) => {} // Expected
            CacheLookup::Miss => panic!("Expected cache hit"),
        }
        Ok(())
    }

    #[test]
    fn cache_miss_on_changed_mtime() -> Result<()> {
        let db = test_db()?;
        let cache = db.file_cache();

        let entry = make_cache_entry("sess-1", "/file.txt", 1000, 500);
        cache.record(&entry)?;

        match cache.check("sess-1", "/file.txt", 1001, 500)? {
            CacheLookup::Hit(_) => panic!("Expected cache miss due to mtime change"),
            CacheLookup::Miss => {} // Expected
        }
        Ok(())
    }

    #[test]
    fn cache_miss_on_changed_size() -> Result<()> {
        let db = test_db()?;
        let cache = db.file_cache();

        let entry = make_cache_entry("sess-1", "/file.txt", 1000, 500);
        cache.record(&entry)?;

        match cache.check("sess-1", "/file.txt", 1000, 501)? {
            CacheLookup::Hit(_) => panic!("Expected cache miss due to size change"),
            CacheLookup::Miss => {} // Expected
        }
        Ok(())
    }

    #[test]
    fn nonexistent_returns_miss() -> Result<()> {
        let db = test_db()?;
        let cache = db.file_cache();

        match cache.check("sess-1", "/nonexistent.txt", 1000, 500)? {
            CacheLookup::Hit(_) => panic!("Expected miss for nonexistent entry"),
            CacheLookup::Miss => {} // Expected
        }
        Ok(())
    }

    // ── for_session range scan ──────────────────────────────────────

    #[test]
    fn for_session_range_scan() -> Result<()> {
        let db = test_db()?;
        let cache = db.file_cache();

        cache.record(&make_cache_entry("sess-1", "/file1.rs", 1000, 100))?;
        cache.record(&make_cache_entry("sess-1", "/file2.rs", 2000, 200))?;
        cache.record(&make_cache_entry("sess-1", "/file3.rs", 3000, 300))?;
        cache.record(&make_cache_entry("sess-2", "/file1.rs", 4000, 400))?;

        let sess1_entries = cache.for_session("sess-1")?;
        assert_eq!(sess1_entries.len(), 3);

        let sess2_entries = cache.for_session("sess-2")?;
        assert_eq!(sess2_entries.len(), 1);

        let sess3_entries = cache.for_session("sess-3")?;
        assert_eq!(sess3_entries.len(), 0);
        Ok(())
    }

    // ── record_hit increments count ─────────────────────────────────

    #[test]
    fn record_hit_increments_count() -> Result<()> {
        let db = test_db()?;
        let cache = db.file_cache();

        let entry = make_cache_entry("sess-1", "/file.rs", 1000, 500);
        cache.record(&entry)?;

        // Initial hit count should be 1
        match cache.check("sess-1", "/file.rs", 1000, 500)? {
            CacheLookup::Hit(retrieved) => assert_eq!(retrieved.hit_count, 1),
            CacheLookup::Miss => panic!("Expected cache hit"),
        }

        // Record a hit and verify count increments
        cache.record_hit("sess-1", "/file.rs")?;

        match cache.check("sess-1", "/file.rs", 1000, 500)? {
            CacheLookup::Hit(retrieved) => assert_eq!(retrieved.hit_count, 2),
            CacheLookup::Miss => panic!("Expected cache hit"),
        }

        // Record another hit
        cache.record_hit("sess-1", "/file.rs")?;

        match cache.check("sess-1", "/file.rs", 1000, 500)? {
            CacheLookup::Hit(retrieved) => assert_eq!(retrieved.hit_count, 3),
            CacheLookup::Miss => panic!("Expected cache hit"),
        }
        Ok(())
    }

    #[test]
    fn record_hit_nonexistent_is_noop() -> Result<()> {
        let db = test_db()?;
        let cache = db.file_cache();

        // Recording hit on nonexistent entry should not error
        cache.record_hit("sess-1", "/nonexistent.rs")?;

        // Should still be a miss
        match cache.check("sess-1", "/nonexistent.rs", 1000, 500)? {
            CacheLookup::Hit(_) => panic!("Should not have created entry"),
            CacheLookup::Miss => {} // Expected
        }
        Ok(())
    }

    // ── clear_session ───────────────────────────────────────────────

    #[test]
    fn clear_session() -> Result<()> {
        let db = test_db()?;
        let cache = db.file_cache();

        cache.record(&make_cache_entry("sess-1", "/file1.rs", 1000, 100))?;
        cache.record(&make_cache_entry("sess-1", "/file2.rs", 2000, 200))?;
        cache.record(&make_cache_entry("sess-2", "/file1.rs", 3000, 300))?;

        let cleared = cache.clear_session("sess-1")?;
        assert_eq!(cleared, 2);

        assert_eq!(cache.for_session("sess-1")?.len(), 0);
        assert_eq!(cache.for_session("sess-2")?.len(), 1);
        Ok(())
    }

    #[test]
    fn clear_nonexistent_session() -> Result<()> {
        let db = test_db()?;
        let cache = db.file_cache();

        let cleared = cache.clear_session("nonexistent")?;
        assert_eq!(cleared, 0);
        Ok(())
    }

    // ── session isolation ───────────────────────────────────────────

    #[test]
    fn session_isolation() -> Result<()> {
        let db = test_db()?;
        let cache = db.file_cache();

        // Same file path in different sessions
        cache.record(&make_cache_entry("sess-a", "/file.rs", 1000, 100))?;
        cache.record(&make_cache_entry("sess-b", "/file.rs", 2000, 200))?;

        // Each session should only see its own entry
        let sess_a_entries = cache.for_session("sess-a")?;
        assert_eq!(sess_a_entries.len(), 1);
        assert_eq!(sess_a_entries[0].mtime_secs, 1000);

        let sess_b_entries = cache.for_session("sess-b")?;
        assert_eq!(sess_b_entries.len(), 1);
        assert_eq!(sess_b_entries[0].mtime_secs, 2000);

        // Check should only find entry from correct session
        match cache.check("sess-a", "/file.rs", 1000, 100)? {
            CacheLookup::Hit(entry) => assert_eq!(entry.mtime_secs, 1000),
            CacheLookup::Miss => panic!("Expected hit for sess-a"),
        }

        match cache.check("sess-a", "/file.rs", 2000, 200)? {
            CacheLookup::Hit(_) => panic!("Should not find sess-b entry from sess-a"),
            CacheLookup::Miss => {} // Expected
        }
        Ok(())
    }

    // ── Counting ────────────────────────────────────────────────────

    #[test]
    fn count_total() -> Result<()> {
        let db = test_db()?;
        let cache = db.file_cache();

        assert_eq!(cache.count()?, 0);

        cache.record(&make_cache_entry("sess-1", "/file1.rs", 1000, 100))?;
        cache.record(&make_cache_entry("sess-2", "/file2.rs", 2000, 200))?;

        assert_eq!(cache.count()?, 2);
        Ok(())
    }

    #[test]
    fn clear_all() -> Result<()> {
        let db = test_db()?;
        let cache = db.file_cache();

        cache.record(&make_cache_entry("sess-1", "/file1.rs", 1000, 100))?;
        cache.record(&make_cache_entry("sess-2", "/file2.rs", 2000, 200))?;

        let cleared = cache.clear()?;
        assert_eq!(cleared, 2);
        assert_eq!(cache.count()?, 0);
        Ok(())
    }

    // ── Key generation ──────────────────────────────────────────────

    #[test]
    fn make_key_is_deterministic() {
        let key1 = make_key("sess-1", "/path/to/file.rs");
        let key2 = make_key("sess-1", "/path/to/file.rs");
        assert_eq!(key1, key2);
    }

    #[test]
    fn make_key_different_paths_different_keys() {
        let key1 = make_key("sess-1", "/path/to/file1.rs");
        let key2 = make_key("sess-1", "/path/to/file2.rs");
        assert_ne!(key1, key2);
    }

    #[test]
    fn make_key_different_sessions_different_keys() {
        let key1 = make_key("sess-1", "/path/to/file.rs");
        let key2 = make_key("sess-2", "/path/to/file.rs");
        assert_ne!(key1, key2);
    }

    #[test]
    fn make_key_includes_session_prefix() {
        let key = make_key("sess-123", "/any/path");
        assert!(key.starts_with("sess-123:"));
    }

    // ── Unicode paths ───────────────────────────────────────────────

    #[test]
    fn unicode_paths() -> Result<()> {
        let db = test_db()?;
        let cache = db.file_cache();

        let entry = make_cache_entry("sess-日本語", "/パス/ファイル.rs", 1000, 500);
        cache.record(&entry)?;

        match cache.check("sess-日本語", "/パス/ファイル.rs", 1000, 500)? {
            CacheLookup::Hit(retrieved) => {
                assert_eq!(retrieved.path, "/パス/ファイル.rs");
                assert_eq!(retrieved.session_id, "sess-日本語");
            }
            CacheLookup::Miss => panic!("Expected cache hit with unicode paths"),
        }
        Ok(())
    }

    // ── Overwrite same entry ────────────────────────────────────────

    #[test]
    fn overwrite_same_entry() -> Result<()> {
        let db = test_db()?;
        let cache = db.file_cache();

        let entry1 = make_cache_entry("sess-1", "/file.rs", 1000, 100);
        cache.record(&entry1)?;

        let mut entry2 = make_cache_entry("sess-1", "/file.rs", 2000, 200);
        entry2.line_count = 99;
        cache.record(&entry2)?;

        // Should get the newer entry
        match cache.check("sess-1", "/file.rs", 2000, 200)? {
            CacheLookup::Hit(retrieved) => {
                assert_eq!(retrieved.mtime_secs, 2000);
                assert_eq!(retrieved.file_size, 200);
                assert_eq!(retrieved.line_count, 99);
            }
            CacheLookup::Miss => panic!("Expected cache hit"),
        }

        // Old entry should not match
        match cache.check("sess-1", "/file.rs", 1000, 100)? {
            CacheLookup::Hit(_) => panic!("Should not match old mtime/size"),
            CacheLookup::Miss => {} // Expected
        }
        Ok(())
    }
}
