//! Embedded database (redb) for router-level persistent storage.
//!
//! Single database file holds:
//! - **usage** — per-day token/cost aggregates by provider and model
//! - **request_log** — recent request audit trail
//! - **rate_limits** — per-provider/model rate-limit and error state
//! - **cache** — response cache keyed by content hash

pub mod cache;
pub mod rate_limits;
pub mod request_log;
pub mod usage;

use std::path::Path;
use std::sync::Arc;

use redb::Database;

use crate::error::Result;

/// Shared handle to the router database.
///
/// Cheaply cloneable (wraps `Arc<Database>`). Pass this around
/// to any component that needs persistent storage.
#[derive(Clone)]
pub struct RouterDb {
    inner: Arc<Database>,
}

impl RouterDb {
    /// Open (or create) the database at the given path.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| crate::Error::Config {
                message: format!("failed to create database directory: {e}"),
            })?;
        }

        let db = Database::create(path).map_err(|e| crate::Error::Config {
            message: format!("failed to open router database: {e}"),
        })?;

        let this = Self { inner: Arc::new(db) };
        this.init_tables()?;
        Ok(this)
    }

    /// Open an in-memory database (for tests).
    #[cfg(test)]
    pub fn in_memory() -> Result<Self> {
        let db = Database::builder().create_with_backend(redb::backends::InMemoryBackend::new()).map_err(|e| {
            crate::Error::Config {
                message: format!("failed to create in-memory database: {e}"),
            }
        })?;
        let this = Self { inner: Arc::new(db) };
        this.init_tables()?;
        Ok(this)
    }

    /// Create all tables if they don't exist yet.
    fn init_tables(&self) -> Result<()> {
        let tx = self.inner.begin_write().map_err(db_err)?;
        tx.open_table(usage::TABLE).map_err(db_err)?;
        tx.open_table(request_log::TABLE).map_err(db_err)?;
        tx.open_table(rate_limits::TABLE).map_err(db_err)?;
        tx.open_table(cache::TABLE).map_err(db_err)?;
        tx.open_table(cache::TTL_TABLE).map_err(db_err)?;
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// Raw read transaction.
    pub(crate) fn begin_read(&self) -> Result<redb::ReadTransaction> {
        self.inner.begin_read().map_err(db_err)
    }

    /// Raw write transaction.
    pub(crate) fn begin_write(&self) -> Result<redb::WriteTransaction> {
        self.inner.begin_write().map_err(db_err)
    }

    /// Usage tracker accessor.
    pub fn usage(&self) -> usage::UsageTracker<'_> {
        usage::UsageTracker::new(self)
    }

    /// Request log accessor.
    pub fn request_log(&self) -> request_log::RequestLog<'_> {
        request_log::RequestLog::new(self)
    }

    /// Rate limit state accessor.
    pub fn rate_limits(&self) -> rate_limits::RateLimitStore<'_> {
        rate_limits::RateLimitStore::new(self)
    }

    /// Response cache accessor.
    pub fn cache(&self) -> cache::ResponseCache<'_> {
        cache::ResponseCache::new(self)
    }
}

impl std::fmt::Debug for RouterDb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RouterDb").finish_non_exhaustive()
    }
}

/// Convert any redb error into our error type.
fn db_err(e: impl std::fmt::Display) -> crate::Error {
    crate::Error::Config {
        message: format!("database error: {e}"),
    }
}

/// Generate a monotonic ID from current time in microseconds.
pub(crate) fn generate_id() -> u64 {
    use std::sync::atomic::AtomicU64;
    use std::sync::atomic::Ordering;

    static LAST: AtomicU64 = AtomicU64::new(0);

    let now = chrono::Utc::now().timestamp_micros() as u64;
    let mut last = LAST.load(Ordering::Relaxed);
    loop {
        let next = now.max(last + 1);
        match LAST.compare_exchange_weak(last, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return next,
            Err(actual) => last = actual,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_creates_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("subdir").join("router.db");
        let db = RouterDb::open(&path).unwrap();
        assert!(path.exists());
        drop(db);
    }

    #[test]
    fn test_open_idempotent() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("router.db");
        let _db1 = RouterDb::open(&path).unwrap();
        drop(_db1);
        let _db2 = RouterDb::open(&path).unwrap();
    }

    #[test]
    fn test_in_memory() {
        let db = RouterDb::in_memory().unwrap();
        let _ = db.usage();
        let _ = db.request_log();
        let _ = db.rate_limits();
        let _ = db.cache();
    }

    #[test]
    fn test_clone_is_cheap() {
        let db = RouterDb::in_memory().unwrap();
        let db2 = db.clone();
        // Both should work
        db.usage().today().unwrap();
        db2.usage().today().unwrap();
    }

    #[test]
    fn test_ids_are_monotonic() {
        let id1 = generate_id();
        let id2 = generate_id();
        let id3 = generate_id();
        assert!(id1 < id2);
        assert!(id2 < id3);
    }
}
