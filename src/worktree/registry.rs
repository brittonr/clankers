//! Track active worktrees in redb (replaces the old JSON file).
//!
//! Key: branch name (string) → serialized WorktreeInfo (bytes).
//! Lives inside the global clankers database so there is a single source
//! of truth that doesn't depend on per-repo `.git/` files surviving.

use redb::ReadableTable;
use redb::ReadableTableMetadata;
use redb::TableDefinition;

use super::WorktreeInfo;
use super::WorktreeStatus;
use crate::db::Db;
use crate::db::db_err;
use crate::error::Result;

/// Table: branch_name (string) → serialized WorktreeInfo (bytes).
pub const TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("worktrees");

/// Redb-backed worktree registry.
///
/// Unlike the old JSON approach this is crash-safe (redb uses write-ahead
/// logging) and works across repos because it lives in the global clankers db.
pub struct WorktreeRegistry<'db> {
    db: &'db Db,
}

impl<'db> WorktreeRegistry<'db> {
    pub fn new(db: &'db Db) -> Self {
        Self { db }
    }

    /// Insert or update a worktree entry.
    pub fn upsert(&self, info: &WorktreeInfo) -> Result<()> {
        let bytes = serde_json::to_vec(info).map_err(|e| crate::error::Error::Database {
            message: format!("failed to serialize worktree info: {e}"),
        })?;
        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;
            table.insert(info.branch.as_str(), bytes.as_slice()).map_err(db_err)?;
        }
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// Get a worktree by branch name.
    pub fn get(&self, branch: &str) -> Result<Option<WorktreeInfo>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;
        match table.get(branch).map_err(db_err)? {
            Some(value) => {
                let info = serde_json::from_slice(value.value()).map_err(|e| crate::error::Error::Database {
                    message: format!("failed to deserialize worktree info: {e}"),
                })?;
                Ok(Some(info))
            }
            None => Ok(None),
        }
    }

    /// Remove a worktree by branch name. Returns true if it existed.
    pub fn remove(&self, branch: &str) -> Result<bool> {
        let tx = self.db.begin_write()?;
        let removed = {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;
            table.remove(branch).map_err(db_err)?.is_some()
        };
        tx.commit().map_err(db_err)?;
        Ok(removed)
    }

    /// Update the status of a worktree. Returns false if not found.
    pub fn set_status(&self, branch: &str, status: WorktreeStatus) -> Result<bool> {
        let mut info = match self.get(branch)? {
            Some(i) => i,
            None => return Ok(false),
        };
        info.status = status;
        self.upsert(&info)?;
        Ok(true)
    }

    /// Find a worktree by session ID.
    pub fn find_by_session(&self, session_id: &str) -> Result<Option<WorktreeInfo>> {
        for info in self.list_all()? {
            if info.session_id == session_id {
                return Ok(Some(info));
            }
        }
        Ok(None)
    }

    /// List all worktrees.
    pub fn list_all(&self) -> Result<Vec<WorktreeInfo>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;
        let mut entries = Vec::new();
        for item in table.iter().map_err(db_err)? {
            let (_key, value) = item.map_err(db_err)?;
            if let Ok(info) = serde_json::from_slice::<WorktreeInfo>(value.value()) {
                entries.push(info);
            }
        }
        Ok(entries)
    }

    /// List worktrees filtered by status.
    pub fn list_by_status(&self, status: WorktreeStatus) -> Result<Vec<WorktreeInfo>> {
        Ok(self.list_all()?.into_iter().filter(|w| w.status == status).collect())
    }

    /// List worktrees for a specific repo root.
    pub fn list_for_repo(&self, repo_root: &str) -> Result<Vec<WorktreeInfo>> {
        Ok(self.list_all()?.into_iter().filter(|w| w.path.to_string_lossy().contains(repo_root)).collect())
    }

    /// List only active worktrees.
    pub fn active(&self) -> Result<Vec<WorktreeInfo>> {
        self.list_by_status(WorktreeStatus::Active)
    }

    /// List completed worktrees awaiting merge.
    pub fn completed(&self) -> Result<Vec<WorktreeInfo>> {
        self.list_by_status(WorktreeStatus::Completed)
    }

    /// Total number of tracked worktrees.
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

    /// Batch-remove multiple branches in a single transaction.
    pub fn remove_batch(&self, branches: &[String]) -> Result<usize> {
        let tx = self.db.begin_write()?;
        let mut count = 0;
        {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;
            for branch in branches {
                if table.remove(branch.as_str()).map_err(db_err)?.is_some() {
                    count += 1;
                }
            }
        }
        tx.commit().map_err(db_err)?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use chrono::Utc;

    use super::*;

    fn test_db() -> Db {
        Db::in_memory().unwrap()
    }

    fn make_worktree(branch: &str, session_id: &str, status: WorktreeStatus) -> WorktreeInfo {
        WorktreeInfo {
            branch: branch.to_string(),
            path: PathBuf::from(format!("/tmp/{branch}")),
            session_id: session_id.to_string(),
            agent: "test".to_string(),
            status,
            created_at: Utc::now(),
            parent_branch: "main".to_string(),
        }
    }

    #[test]
    fn test_upsert_and_get() {
        let db = test_db();
        let reg = db.worktrees();
        let info = make_worktree("clankers/main-abc", "sess1", WorktreeStatus::Active);
        reg.upsert(&info).unwrap();

        let got = reg.get("clankers/main-abc").unwrap().unwrap();
        assert_eq!(got.session_id, "sess1");
        assert_eq!(got.status, WorktreeStatus::Active);
    }

    #[test]
    fn test_get_missing() {
        let db = test_db();
        assert!(db.worktrees().get("nonexistent").unwrap().is_none());
    }

    #[test]
    fn test_remove() {
        let db = test_db();
        let reg = db.worktrees();
        reg.upsert(&make_worktree("br", "s1", WorktreeStatus::Active)).unwrap();
        assert!(reg.remove("br").unwrap());
        assert!(!reg.remove("br").unwrap());
        assert!(reg.get("br").unwrap().is_none());
    }

    #[test]
    fn test_set_status() {
        let db = test_db();
        let reg = db.worktrees();
        reg.upsert(&make_worktree("br", "s1", WorktreeStatus::Active)).unwrap();
        assert!(reg.set_status("br", WorktreeStatus::Completed).unwrap());

        let got = reg.get("br").unwrap().unwrap();
        assert_eq!(got.status, WorktreeStatus::Completed);
    }

    #[test]
    fn test_set_status_missing() {
        let db = test_db();
        assert!(!db.worktrees().set_status("ghost", WorktreeStatus::Stale).unwrap());
    }

    #[test]
    fn test_list_by_status() {
        let db = test_db();
        let reg = db.worktrees();
        reg.upsert(&make_worktree("a", "s1", WorktreeStatus::Active)).unwrap();
        reg.upsert(&make_worktree("b", "s2", WorktreeStatus::Completed)).unwrap();
        reg.upsert(&make_worktree("c", "s3", WorktreeStatus::Active)).unwrap();

        assert_eq!(reg.active().unwrap().len(), 2);
        assert_eq!(reg.completed().unwrap().len(), 1);
    }

    #[test]
    fn test_find_by_session() {
        let db = test_db();
        let reg = db.worktrees();
        reg.upsert(&make_worktree("br1", "sess-42", WorktreeStatus::Active)).unwrap();
        reg.upsert(&make_worktree("br2", "sess-99", WorktreeStatus::Active)).unwrap();

        let found = reg.find_by_session("sess-42").unwrap().unwrap();
        assert_eq!(found.branch, "br1");
        assert!(reg.find_by_session("nonexistent").unwrap().is_none());
    }

    #[test]
    fn test_count() {
        let db = test_db();
        let reg = db.worktrees();
        assert_eq!(reg.count().unwrap(), 0);
        reg.upsert(&make_worktree("a", "s1", WorktreeStatus::Active)).unwrap();
        reg.upsert(&make_worktree("b", "s2", WorktreeStatus::Active)).unwrap();
        assert_eq!(reg.count().unwrap(), 2);
    }

    #[test]
    fn test_clear() {
        let db = test_db();
        let reg = db.worktrees();
        reg.upsert(&make_worktree("a", "s1", WorktreeStatus::Active)).unwrap();
        reg.upsert(&make_worktree("b", "s2", WorktreeStatus::Active)).unwrap();
        let cleared = reg.clear().unwrap();
        assert_eq!(cleared, 2);
        assert_eq!(reg.count().unwrap(), 0);
    }

    #[test]
    fn test_remove_batch() {
        let db = test_db();
        let reg = db.worktrees();
        reg.upsert(&make_worktree("a", "s1", WorktreeStatus::Active)).unwrap();
        reg.upsert(&make_worktree("b", "s2", WorktreeStatus::Active)).unwrap();
        reg.upsert(&make_worktree("c", "s3", WorktreeStatus::Active)).unwrap();

        let removed = reg.remove_batch(&["a".into(), "c".into(), "ghost".into()]).unwrap();
        assert_eq!(removed, 2);
        assert_eq!(reg.count().unwrap(), 1);
        assert!(reg.get("b").unwrap().is_some());
    }

    #[test]
    fn test_upsert_overwrites() {
        let db = test_db();
        let reg = db.worktrees();
        let mut info = make_worktree("br", "s1", WorktreeStatus::Active);
        reg.upsert(&info).unwrap();

        info.status = WorktreeStatus::Completed;
        info.agent = "updated".to_string();
        reg.upsert(&info).unwrap();

        let got = reg.get("br").unwrap().unwrap();
        assert_eq!(got.status, WorktreeStatus::Completed);
        assert_eq!(got.agent, "updated");
    }
}
