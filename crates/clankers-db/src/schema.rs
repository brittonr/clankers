//! Schema versioning and migrations for the clankers database.
//!
//! Every database gets a version stamp in the `_meta` table. On open,
//! [`migrate`] compares the stored version against [`CURRENT_VERSION`]
//! and runs any outstanding migration steps inside a single write
//! transaction. This means migrations are atomic — they either fully
//! apply or fully roll back.
//!
//! ## Adding a migration
//!
//! 1. Write a function `fn migrate_N_to_N1(tx: &WriteTransaction) -> Result<()>`
//! 2. Add it to the `MIGRATIONS` array
//! 3. Bump `CURRENT_VERSION`
//! 4. Add a test

use redb::TableDefinition;
use redb::WriteTransaction;
use tracing::info;

use crate::error::Result;
use crate::error::db_err;

/// Worktree registry table (inlined from worktree::registry).
const WORKTREE_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("worktrees");

/// Metadata table: stores `"schema_version"` → version number (as u32 LE bytes).
const META_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("_meta");

const VERSION_KEY: &str = "schema_version";

/// Current schema version. Bump this when adding a migration.
pub const CURRENT_VERSION: u32 = 6;

/// Each migration function advances the schema by one version.
/// Index 0 = migration from v0→v1, index 1 = v1→v2, etc.
///
/// Migrations receive a write transaction that already has the `_meta`
/// table open. They must NOT commit — the caller commits once after
/// all pending migrations succeed.
const MIGRATIONS: &[fn(&WriteTransaction) -> Result<()>] = &[
    migrate_0_to_1,
    migrate_1_to_2,
    migrate_2_to_3,
    migrate_3_to_4,
    migrate_4_to_5,
    migrate_5_to_6,
];

/// Run all pending migrations. Called from [`Db::open`] on every startup.
///
/// - Fresh database (no `_meta` table): starts at version 0.
/// - Up-to-date database: no-op.
/// - Database newer than this binary: returns an error.
pub fn migrate(db: &redb::Database) -> Result<()> {
    let current = read_version(db)?;

    if current == CURRENT_VERSION {
        return Ok(());
    }

    if current > CURRENT_VERSION {
        return Err(crate::error::DbError {
            message: format!(
                "database schema version {current} is newer than this binary supports ({CURRENT_VERSION}). \
                 Upgrade clankers or use a matching version."
            ),
        });
    }

    info!(from = current, to = CURRENT_VERSION, "migrating database schema");

    let tx = db.begin_write().map_err(db_err)?;

    // Ensure _meta table exists (fresh databases won't have it yet).
    tx.open_table(META_TABLE).map_err(db_err)?;

    for (i, migration) in MIGRATIONS.iter().enumerate() {
        let from = i as u32;
        let to = from + 1;
        if from >= current {
            info!(from, to, "applying migration");
            migration(&tx)?;
            write_version_in_tx(&tx, to)?;
        }
    }

    tx.commit().map_err(db_err)?;

    info!(version = CURRENT_VERSION, "database schema up to date");
    Ok(())
}

/// Read the current schema version. Returns 0 if the `_meta` table
/// doesn't exist or has no version entry (legacy / fresh database).
fn read_version(db: &redb::Database) -> Result<u32> {
    let tx = db.begin_read().map_err(db_err)?;

    // If the _meta table doesn't exist at all, this is a fresh or legacy db.
    let table = match tx.open_table(META_TABLE) {
        Ok(t) => t,
        Err(redb::TableError::TableDoesNotExist(_)) => return Ok(0),
        Err(e) => return Err(db_err(e)),
    };

    match table.get(VERSION_KEY).map_err(db_err)? {
        Some(value) => {
            let bytes = value.value();
            if bytes.len() == 4 {
                Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
            } else {
                // Corrupted version entry — treat as 0 and re-migrate.
                Ok(0)
            }
        }
        None => Ok(0),
    }
}

/// Write the version inside an existing write transaction.
fn write_version_in_tx(tx: &WriteTransaction, version: u32) -> Result<()> {
    let mut table = tx.open_table(META_TABLE).map_err(db_err)?;
    let bytes = version.to_le_bytes();
    table.insert(VERSION_KEY, bytes.as_slice()).map_err(db_err)?;
    Ok(())
}

// ── Migrations ──────────────────────────────────────────────────────

/// v0 → v1: Create all initial tables.
///
/// This is the baseline migration. For fresh databases it creates
/// everything from scratch. For existing databases that predate schema
/// versioning, these are all no-ops (redb's `open_table` is idempotent).
fn migrate_0_to_1(tx: &WriteTransaction) -> Result<()> {
    use crate::audit;
    use crate::history;
    use crate::memory;
    use crate::session_index;
    use crate::usage;

    tx.open_table(audit::TABLE).map_err(db_err)?;
    tx.open_table(memory::TABLE).map_err(db_err)?;
    tx.open_table(session_index::TABLE).map_err(db_err)?;
    tx.open_table(history::TABLE).map_err(db_err)?;
    tx.open_table(usage::TABLE).map_err(db_err)?;
    tx.open_table(WORKTREE_TABLE).map_err(db_err)?;

    Ok(())
}

/// v1 → v2: Add tool result content store.
fn migrate_1_to_2(tx: &WriteTransaction) -> Result<()> {
    use crate::tool_results;
    tx.open_table(tool_results::TABLE).map_err(db_err)?;
    Ok(())
}

/// v2 → v3: Add file read cache.
fn migrate_2_to_3(tx: &WriteTransaction) -> Result<()> {
    use crate::file_cache;
    tx.open_table(file_cache::TABLE).map_err(db_err)?;
    Ok(())
}

/// v3 → v4: Add resource registry.
fn migrate_3_to_4(tx: &WriteTransaction) -> Result<()> {
    use crate::registry;
    tx.open_table(registry::TABLE).map_err(db_err)?;
    Ok(())
}

/// v4 → v5: Add skill usage tracking.
fn migrate_4_to_5(tx: &WriteTransaction) -> Result<()> {
    use crate::skill_usage;
    tx.open_table(skill_usage::TABLE).map_err(db_err)?;
    Ok(())
}

/// v5 → v6: Add metrics tables (session summaries, daily rollups, recent events).
fn migrate_5_to_6(tx: &WriteTransaction) -> Result<()> {
    use crate::metrics::storage;
    tx.open_table(storage::SESSION_SUMMARY_TABLE).map_err(db_err)?;
    tx.open_table(storage::DAILY_ROLLUP_TABLE).map_err(db_err)?;
    tx.open_table(storage::RECENT_EVENTS_TABLE).map_err(db_err)?;
    Ok(())
}

/// Read the schema version from an already-open database.
/// Useful for diagnostics / status commands.
pub fn version(db: &redb::Database) -> Result<u32> {
    read_version(db)
}

#[cfg(test)]
mod tests {
    use redb::backends::InMemoryBackend;

    use super::*;

    fn mem_db() -> Result<redb::Database> {
        redb::Database::builder().create_with_backend(InMemoryBackend::new()).map_err(db_err)
    }

    #[test]
    fn fresh_database_migrates_to_current() -> Result<()> {
        let db = mem_db()?;
        migrate(&db)?;
        assert_eq!(read_version(&db)?, CURRENT_VERSION);
        Ok(())
    }

    #[test]
    fn migration_is_idempotent() -> Result<()> {
        let db = mem_db()?;
        migrate(&db)?;
        migrate(&db)?;
        migrate(&db)?;
        assert_eq!(read_version(&db)?, CURRENT_VERSION);
        Ok(())
    }

    #[test]
    fn already_current_is_noop() -> Result<()> {
        let db = mem_db()?;
        migrate(&db)?;

        // Write something to a table, then re-migrate — data should survive.
        {
            let tx = db.begin_write().map_err(db_err)?;
            {
                let mut table = tx.open_table(crate::memory::TABLE).map_err(db_err)?;
                table.insert(42u64, b"test".as_slice()).map_err(db_err)?;
            }
            tx.commit().map_err(db_err)?;
        }

        migrate(&db)?;

        let tx = db.begin_read().map_err(db_err)?;
        let table = tx.open_table(crate::memory::TABLE).map_err(db_err)?;
        assert!(table.get(42u64).map_err(db_err)?.is_some());
        Ok(())
    }

    #[test]
    fn future_version_rejected() -> Result<()> {
        let db = mem_db()?;
        migrate(&db)?;

        // Manually set a future version.
        {
            let tx = db.begin_write().map_err(db_err)?;
            {
                let mut table = tx.open_table(META_TABLE).map_err(db_err)?;
                let future = (CURRENT_VERSION + 10).to_le_bytes();
                table.insert(VERSION_KEY, future.as_slice()).map_err(db_err)?;
            }
            tx.commit().map_err(db_err)?;
        }

        let err = migrate(&db).expect_err("should fail with future version");
        let msg = err.to_string();
        assert!(msg.contains("newer than this binary"), "got: {msg}");
        Ok(())
    }

    #[test]
    fn legacy_database_gets_versioned() -> Result<()> {
        let db = mem_db()?;

        // Simulate a legacy database: tables exist but no _meta table.
        {
            let tx = db.begin_write().map_err(db_err)?;
            tx.open_table(crate::audit::TABLE).map_err(db_err)?;
            tx.open_table(crate::memory::TABLE).map_err(db_err)?;
            tx.open_table(crate::session_index::TABLE).map_err(db_err)?;
            tx.open_table(crate::history::TABLE).map_err(db_err)?;
            tx.open_table(crate::usage::TABLE).map_err(db_err)?;
            tx.open_table(WORKTREE_TABLE).map_err(db_err)?;
            tx.commit().map_err(db_err)?;
        }

        // No _meta table → version 0.
        assert_eq!(read_version(&db)?, 0);

        // Migrate should stamp it as current version without breaking existing tables.
        migrate(&db)?;
        assert_eq!(read_version(&db)?, CURRENT_VERSION);
        Ok(())
    }

    #[test]
    fn all_tables_exist_after_migration() -> Result<()> {
        let db = mem_db()?;
        migrate(&db)?;

        let tx = db.begin_read().map_err(db_err)?;
        // All 6 data tables + _meta should be openable.
        tx.open_table(crate::audit::TABLE).map_err(db_err)?;
        tx.open_table(crate::memory::TABLE).map_err(db_err)?;
        tx.open_table(crate::session_index::TABLE).map_err(db_err)?;
        tx.open_table(crate::history::TABLE).map_err(db_err)?;
        tx.open_table(crate::usage::TABLE).map_err(db_err)?;
        tx.open_table(WORKTREE_TABLE).map_err(db_err)?;
        tx.open_table(META_TABLE).map_err(db_err)?;
        Ok(())
    }

    #[test]
    fn version_helper_reads_current() -> Result<()> {
        let db = mem_db()?;
        assert_eq!(version(&db)?, 0);
        migrate(&db)?;
        assert_eq!(version(&db)?, CURRENT_VERSION);
        Ok(())
    }

    #[test]
    fn corrupted_version_treated_as_zero() -> Result<()> {
        let db = mem_db()?;

        // Write garbage into the version key.
        {
            let tx = db.begin_write().map_err(db_err)?;
            {
                let mut table = tx.open_table(META_TABLE).map_err(db_err)?;
                table.insert(VERSION_KEY, b"bad".as_slice()).map_err(db_err)?;
            }
            tx.commit().map_err(db_err)?;
        }

        // Should read as 0 and re-migrate cleanly.
        assert_eq!(read_version(&db)?, 0);
        migrate(&db)?;
        assert_eq!(read_version(&db)?, CURRENT_VERSION);
        Ok(())
    }

    #[test]
    fn migration_count_matches_current_version() {
        // Safety check: the MIGRATIONS array length must equal CURRENT_VERSION.
        assert_eq!(
            MIGRATIONS.len() as u32,
            CURRENT_VERSION,
            "MIGRATIONS array length ({}) must match CURRENT_VERSION ({CURRENT_VERSION})",
            MIGRATIONS.len()
        );
    }
}
