//! Embedded database (redb) for structured persistent storage.
//!
//! Single database file at `~/.clankers/agent/clankers.db` holds:
//! - **audit** — append-only log of every tool invocation
//! - **memory** — cross-session learned facts and preferences
//! - **session_index** — fast session listing without scanning JSONL files
//! - **history** — prompt history for Ctrl+R search
//! - **usage** — daily token usage / cost tracking
//!
//! ## Async safety
//!
//! redb is synchronous. Use [`Db::blocking`] to run operations on the
//! tokio blocking threadpool, avoiding stalls on the async runtime.

pub mod audit;
pub mod error;
pub mod history;

pub use error::DbError;
pub use error::db_err;
pub mod memory;
pub mod schema;
pub mod session_index;
pub mod usage;

use std::path::Path;
use std::sync::Arc;

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
