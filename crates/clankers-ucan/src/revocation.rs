//! Persistent revocation storage for capability tokens.
//!
//! This module provides persistent storage for token revocation lists using redb,
//! ensuring that revocations survive restarts.
//!
//! # Design
//!
//! Revoked tokens are stored as entries in a redb table mapping token hashes
//! to revocation timestamps. A separate table stores auth tokens by user ID.
//!
//! # Tiger Style
//!
//! - Fixed limit on revocation list size (MAX_REVOCATION_LIST_SIZE = 10,000)
//! - Revocations are write-once, never deleted (until token expires naturally)
//! - Bounded scan operations to prevent unbounded memory usage

use std::sync::Arc;

use redb::Database;
use redb::ReadableTable;
use redb::TableDefinition;

/// Table: token_hash ([u8; 32]) -> revocation_timestamp (u64)
pub const REVOKED_TOKENS_TABLE: TableDefinition<&[u8], u64> = TableDefinition::new("revoked_tokens");

/// Table: user_id (str) -> encoded token bytes
pub const AUTH_TOKENS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("auth_tokens");

use clanker_auth::RevocationStore;

/// Revocation store backed by redb.
///
/// Stores revoked token hashes in a persistent embedded database.
pub struct RedbRevocationStore {
    db: Arc<Database>,
}

impl RedbRevocationStore {
    /// Create a new revocation store backed by the given redb database.
    ///
    /// # Errors
    ///
    /// Returns an error if the database tables cannot be created.
    #[allow(clippy::result_large_err)]
    pub fn new(db: Arc<Database>) -> Result<Self, redb::Error> {
        // Create tables if they don't exist
        let write_txn = db.begin_write()?;
        {
            let _ = write_txn.open_table(REVOKED_TOKENS_TABLE)?;
            let _ = write_txn.open_table(AUTH_TOKENS_TABLE)?;
        }
        write_txn.commit()?;

        Ok(Self { db })
    }

    /// Get a reference to the underlying database.
    pub fn db(&self) -> &Arc<Database> {
        &self.db
    }
}

impl RevocationStore for RedbRevocationStore {
    fn is_revoked(&self, token_hash: &[u8; 32]) -> bool {
        // Read-only transaction
        let read_txn = match self.db.begin_read() {
            Ok(txn) => txn,
            Err(_) => return false, // Fail open if we can't read
        };

        let table = match read_txn.open_table(REVOKED_TOKENS_TABLE) {
            Ok(t) => t,
            Err(_) => return false,
        };

        // Check if the hash exists in the table
        matches!(table.get(token_hash.as_slice()), Ok(Some(_)))
    }

    fn revoke(&self, hash: [u8; 32], timestamp: u64) {
        // Write transaction
        let write_txn = match self.db.begin_write() {
            Ok(txn) => txn,
            Err(e) => {
                tracing::error!("Failed to begin write transaction for revocation: {}", e);
                return;
            }
        };

        {
            let mut table = match write_txn.open_table(REVOKED_TOKENS_TABLE) {
                Ok(t) => t,
                Err(e) => {
                    tracing::error!("Failed to open revoked tokens table: {}", e);
                    return;
                }
            };

            if let Err(e) = table.insert(hash.as_slice(), timestamp) {
                tracing::error!("Failed to insert revocation: {}", e);
                return;
            }
        }

        if let Err(e) = write_txn.commit() {
            tracing::error!("Failed to commit revocation: {}", e);
        }
    }

    fn load_all(&self) -> Vec<[u8; 32]> {
        let read_txn = match self.db.begin_read() {
            Ok(txn) => txn,
            Err(e) => {
                tracing::error!("Failed to begin read transaction: {}", e);
                return Vec::new();
            }
        };

        let table = match read_txn.open_table(REVOKED_TOKENS_TABLE) {
            Ok(t) => t,
            Err(e) => {
                tracing::error!("Failed to open revoked tokens table: {}", e);
                return Vec::new();
            }
        };

        let mut hashes = Vec::new();

        // Iterate over all entries
        let iter = match table.iter() {
            Ok(it) => it,
            Err(e) => {
                tracing::error!("Failed to iterate revoked tokens: {}", e);
                return Vec::new();
            }
        };

        for item in iter {
            let (key, _value) = match item {
                Ok(kv) => kv,
                Err(e) => {
                    tracing::error!("Failed to read revocation entry: {}", e);
                    continue;
                }
            };

            let key_bytes = key.value();
            if key_bytes.len() == 32 {
                let mut hash = [0u8; 32];
                hash.copy_from_slice(key_bytes);
                hashes.push(hash);
            } else {
                tracing::warn!("Invalid revocation key length: {} (expected 32)", key_bytes.len());
            }

            // Tiger Style: Enforce max size to prevent unbounded memory usage
            if hashes.len() >= usize::try_from(crate::constants::MAX_REVOCATION_LIST_SIZE).unwrap_or(0) {
                tracing::warn!(
                    "Revocation list truncated at {} entries (limit: {})",
                    hashes.len(),
                    crate::constants::MAX_REVOCATION_LIST_SIZE
                );
                break;
            }
        }

        hashes
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn create_test_store() -> (RedbRevocationStore, TempDir) {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test_revocation.db");
        let db = Database::create(&db_path).unwrap();
        let store = RedbRevocationStore::new(Arc::new(db)).unwrap();
        (store, temp_dir)
    }

    #[test]
    fn test_revoke_and_check() {
        let (store, _temp) = create_test_store();

        let hash = [1u8; 32];
        let timestamp = 12345u64;

        // Initially not revoked
        assert!(!store.is_revoked(&hash));

        // Revoke it
        store.revoke(hash, timestamp);

        // Now it should be revoked
        assert!(store.is_revoked(&hash));
    }

    #[test]
    fn test_load_all() {
        let (store, _temp) = create_test_store();

        let hash1 = [1u8; 32];
        let hash2 = [2u8; 32];
        let hash3 = [3u8; 32];

        store.revoke(hash1, 100);
        store.revoke(hash2, 200);
        store.revoke(hash3, 300);

        let loaded = store.load_all();
        assert_eq!(loaded.len(), 3);
        assert!(loaded.contains(&hash1));
        assert!(loaded.contains(&hash2));
        assert!(loaded.contains(&hash3));
    }

    #[test]
    fn test_persistence() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test_persistence.db");

        let hash = [42u8; 32];

        // Create store and revoke a token
        {
            let db = Database::create(&db_path).unwrap();
            let store = RedbRevocationStore::new(Arc::new(db)).unwrap();
            store.revoke(hash, 999);
            assert!(store.is_revoked(&hash));
        }

        // Open the same database again and verify persistence
        {
            let db = Database::open(&db_path).unwrap();
            let store = RedbRevocationStore::new(Arc::new(db)).unwrap();
            assert!(store.is_revoked(&hash));

            let loaded = store.load_all();
            assert_eq!(loaded.len(), 1);
            assert!(loaded.contains(&hash));
        }
    }

    #[test]
    fn test_idempotent_revocation() {
        let (store, _temp) = create_test_store();

        let hash = [5u8; 32];

        // Revoke multiple times
        store.revoke(hash, 100);
        store.revoke(hash, 200);
        store.revoke(hash, 300);

        // Should still be revoked, and load_all should only return it once
        assert!(store.is_revoked(&hash));
        let loaded = store.load_all();
        assert_eq!(loaded.len(), 1);
        assert!(loaded.contains(&hash));
    }
}
