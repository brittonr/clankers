//! Embedded database (redb) for structured persistent storage.
//!
//! Single database file at `~/.clankers/agent/clankers.db` holds:
//! - **audit** — append-only log of every tool invocation
//! - **memory** — cross-session learned facts and preferences
//! - **session_index** — fast session listing without scanning JSONL files
//! - **history** — prompt history for Ctrl+R search
//! - **usage** — daily token usage / cost tracking
//! - **tool_results** — full tool result content for compacted context recovery
//!
//! ## Async safety
//!
//! redb is synchronous. Use [`Db::blocking`] to run operations on the
//! tokio blocking threadpool, avoiding stalls on the async runtime.
#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", feature(register_tool), register_tool(tigerstyle))]
pub mod audit;
pub mod error;
pub mod file_cache;
pub mod history;
pub mod process_jobs;
pub mod registry;

pub use error::DbError;
pub use error::db_err;

#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(tigerstyle::ambient_clock, reason = "database shell-boundary timestamp source")
)]
pub(crate) fn db_clock_now() -> DateTime<Utc> {
    Utc::now()
}

pub(crate) fn db_collection_capacity(row_count: u64) -> usize {
    match usize::try_from(row_count) {
        Ok(capacity) => capacity,
        Err(_) => usize::MAX,
    }
}

pub(crate) fn db_limit_entries(limit_count: u32) -> usize {
    match usize::try_from(limit_count) {
        Ok(capacity) => capacity,
        Err(_) => usize::MAX,
    }
}

pub(crate) fn db_count_from_len(len_count: usize) -> u64 {
    match u64::try_from(len_count) {
        Ok(count) => count,
        Err(_) => u64::MAX,
    }
}
pub mod insights;
pub mod memory;
pub mod metrics;
pub mod schema;
pub mod search_index;
pub mod session_index;
pub mod skill_usage;
pub mod tool_results;
pub mod usage;

use std::path::Path;
use std::sync::Arc;

use chrono::DateTime;
use chrono::Utc;
use redb::Database;

use crate::error::Result;

/// Shared handle to the clankers database.
///
/// Cheaply cloneable (wraps `Arc<Database>`). Pass this around
/// to any component that needs persistent storage.
///
/// All public table accessors (`.memory()`, `.usage()`, etc.) perform
/// synchronous I/O under the hood. In async contexts, wrap calls with
/// [`Db::blocking`] to avoid blocking the tokio runtime.
#[derive(Clone)]
pub struct Db {
    inner: Arc<Database>,
}

impl Db {
    /// Open (or create) the database at the given path.
    ///
    /// Runs schema migrations on first open to ensure all tables exist
    /// and the schema is up to date. See [`schema`] for details.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| crate::error::DbError {
                message: format!("failed to create database directory: {e}"),
            })?;
        }

        let db = Database::create(path).map_err(|e| crate::error::DbError {
            message: format!("failed to open database: {e}"),
        })?;

        schema::migrate(&db)?;

        Ok(Self { inner: Arc::new(db) })
    }

    /// Open an in-memory database (useful for tests).
    pub fn in_memory() -> Result<Self> {
        let db = Database::builder().create_with_backend(redb::backends::InMemoryBackend::new()).map_err(|e| {
            crate::error::DbError {
                message: format!("failed to create in-memory database: {e}"),
            }
        })?;

        schema::migrate(&db)?;

        Ok(Self { inner: Arc::new(db) })
    }

    /// Current schema version of this database.
    pub fn schema_version(&self) -> Result<u32> {
        schema::version(&self.inner)
    }

    /// Raw read transaction.
    pub fn begin_read(&self) -> Result<redb::ReadTransaction> {
        self.inner.begin_read().map_err(db_err)
    }

    /// Raw write transaction.
    pub fn begin_write(&self) -> Result<redb::WriteTransaction> {
        self.inner.begin_write().map_err(db_err)
    }

    /// Run a synchronous database closure on the tokio blocking threadpool.
    ///
    /// redb transactions perform disk I/O (especially writes). This helper
    /// keeps the async runtime unblocked:
    ///
    /// ```ignore
    /// let count = db.blocking(|db| db.memory().count()).await?;
    /// ```
    pub async fn blocking<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Db) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let db = self.clone();
        tokio::task::spawn_blocking(move || f(&db)).await.map_err(|e| crate::error::DbError {
            message: format!("blocking task panicked: {e}"),
        })?
    }

    /// Fire-and-forget a database write on the blocking threadpool.
    ///
    /// Errors are logged via `tracing::warn` rather than returned.
    /// Ideal for non-critical writes like usage recording.
    pub fn spawn_write<F>(&self, f: F)
    where F: FnOnce(&Db) + Send + 'static {
        let db = self.clone();
        tokio::task::spawn_blocking(move || f(&db));
    }

    /// Audit log accessor.
    pub fn audit(&self) -> audit::AuditLog<'_> {
        audit::AuditLog::new(self)
    }

    /// Memory store accessor.
    pub fn memory(&self) -> memory::MemoryStore<'_> {
        memory::MemoryStore::new(self)
    }

    /// Session index accessor.
    pub fn sessions(&self) -> session_index::SessionIndex<'_> {
        session_index::SessionIndex::new(self)
    }

    /// Prompt history accessor.
    pub fn history(&self) -> history::HistoryDb<'_> {
        history::HistoryDb::new(self)
    }

    /// Usage tracker accessor.
    pub fn usage(&self) -> usage::UsageTracker<'_> {
        usage::UsageTracker::new(self)
    }

    /// Tool result content store accessor.
    pub fn tool_results(&self) -> tool_results::ToolResultStore<'_> {
        tool_results::ToolResultStore::new(self)
    }

    pub fn skill_usage(&self) -> skill_usage::SkillUsageStore<'_> {
        skill_usage::SkillUsageStore::new(self)
    }

    /// File read cache accessor.
    pub fn file_cache(&self) -> file_cache::FileReadCache<'_> {
        file_cache::FileReadCache::new(self)
    }

    /// Process/job metadata store accessor.
    pub fn process_jobs(&self) -> process_jobs::ProcessJobStore<'_> {
        process_jobs::ProcessJobStore::new(self)
    }

    /// Async process/job metadata facade that keeps redb I/O off async runtime workers.
    #[must_use]
    pub fn async_process_jobs(&self) -> process_jobs::AsyncProcessJobStore {
        process_jobs::AsyncProcessJobStore::new(self.clone())
    }

    /// Resource registry accessor.
    pub fn registry(&self) -> registry::Registry<'_> {
        registry::Registry::new(self)
    }

    /// Metrics store accessor.
    pub fn metrics(&self) -> metrics::MetricsStore<'_> {
        metrics::MetricsStore::new(self)
    }
}

impl std::fmt::Debug for Db {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Db").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_creates_file() {
        let tmp = tempfile::TempDir::new().expect("failed to create temp dir");
        let path = tmp.path().join("subdir").join("clankers.db");
        let db = Db::open(&path).expect("failed to open database");
        assert!(path.exists());
        drop(db);
    }

    #[test]
    fn test_open_idempotent() {
        let tmp = tempfile::TempDir::new().expect("failed to create temp dir");
        let path = tmp.path().join("clankers.db");
        let _db1 = Db::open(&path).expect("failed to open database first time");
        drop(_db1);
        let _db2 = Db::open(&path).expect("failed to open database second time");
    }

    #[test]
    fn test_in_memory() {
        let db = Db::in_memory().expect("failed to create in-memory db");
        // Should be able to access all stores
        let _ = db.memory();
        let _ = db.sessions();
        let _ = db.history();
        let _ = db.usage();
        let _ = db.file_cache();
    }

    #[test]
    fn test_clone_is_cheap() {
        let db = Db::in_memory().expect("failed to create in-memory db");
        let db2 = db.clone();
        // Both should work
        db.memory().list(None).expect("failed to list memory from db");
        db2.memory().list(None).expect("failed to list memory from db2");
    }
}
