//! Persistent work item store (redb-backed)
//!
//! Provides CRUD, dependency queries, and the `ready()` function that
//! returns items with no open blockers (the Beads `bd ready` equivalent).

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;

use redb::ReadableTable;
use tracing::debug;

use super::item::*;

const WORK_TABLE: redb::TableDefinition<&str, &[u8]> = redb::TableDefinition::new("work_items");

/// Persistent store for work items.
pub struct WorkStore {
    db: redb::Database,
}

impl WorkStore {
    /// Open or create the work store.
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
        }
        let db = redb::Database::create(path).map_err(|e| format!("redb open: {e}"))?;

        // Ensure table exists
        let txn = db.begin_write().map_err(|e| format!("begin_write: {e}"))?;
        {
            let _ = txn.open_table(WORK_TABLE).map_err(|e| format!("open_table: {e}"))?;
        }
        txn.commit().map_err(|e| format!("commit: {e}"))?;

        Ok(Self { db })
    }

    /// Insert or update a work item.
    pub fn put(&self, item: &WorkItem) -> Result<(), String> {
        let bytes = serde_json::to_vec(item).map_err(|e| format!("serialize: {e}"))?;
        let txn = self.db.begin_write().map_err(|e| format!("begin_write: {e}"))?;
        {
            let mut table = txn.open_table(WORK_TABLE).map_err(|e| format!("open_table: {e}"))?;
            table.insert(item.id.as_str(), bytes.as_slice()).map_err(|e| format!("insert: {e}"))?;
        }
        txn.commit().map_err(|e| format!("commit: {e}"))?;
        Ok(())
    }

    /// Get a work item by ID.
    pub fn get(&self, id: &str) -> Result<Option<WorkItem>, String> {
        let txn = self.db.begin_read().map_err(|e| format!("begin_read: {e}"))?;
        let table = txn.open_table(WORK_TABLE).map_err(|e| format!("open_table: {e}"))?;
        match table.get(id).map_err(|e| format!("get: {e}"))? {
            Some(bytes) => {
                let item: WorkItem = serde_json::from_slice(bytes.value()).map_err(|e| format!("deserialize: {e}"))?;
                Ok(Some(item))
            }
            None => Ok(None),
        }
    }

    /// List all work items.
    pub fn list_all(&self) -> Result<Vec<WorkItem>, String> {
        let txn = self.db.begin_read().map_err(|e| format!("begin_read: {e}"))?;
        let table = txn.open_table(WORK_TABLE).map_err(|e| format!("open_table: {e}"))?;
        let mut items = Vec::new();
        for entry in table.iter().map_err(|e| format!("iter: {e}"))? {
            let (_, value) = entry.map_err(|e| format!("entry: {e}"))?;
            let item: WorkItem = serde_json::from_slice(value.value()).map_err(|e| format!("deserialize: {e}"))?;
            items.push(item);
        }
        Ok(items)
    }

    /// List open (non-terminal) work items, sorted by priority then created_at.
    pub fn list_open(&self) -> Result<Vec<WorkItem>, String> {
        let mut items: Vec<WorkItem> = self.list_all()?.into_iter().filter(|i| i.status.is_open()).collect();
        items.sort_by(|a, b| a.priority.cmp(&b.priority).then(a.created_at.cmp(&b.created_at)));
        Ok(items)
    }

    /// **Ready items** — open items with no open blockers.
    ///
    /// This is the Beads `bd ready` equivalent. An item is ready when:
    /// - Its status is `Open` (not in_progress, not terminal)
    /// - All items in its `blocked_by` list have terminal status (Done/Failed/Cancelled)
    ///
    /// Sorted by priority (P0 first), then by creation time.
    pub fn ready(&self) -> Result<Vec<WorkItem>, String> {
        let all = self.list_all()?;

        // Build status lookup (owned keys to avoid borrow conflict)
        let status_map: HashMap<String, Status> = all.iter().map(|i| (i.id.clone(), i.status)).collect();

        let mut ready: Vec<WorkItem> = all
            .into_iter()
            .filter(|item| {
                if item.status != Status::Open {
                    return false;
                }
                // Check all blockers are terminal
                item.blocked_by.iter().all(|dep_id| {
                    status_map.get(dep_id).map(|s| s.is_terminal()).unwrap_or(true) // unknown dep = assume satisfied
                })
            })
            .collect();

        ready.sort_by(|a, b| a.priority.cmp(&b.priority).then(a.created_at.cmp(&b.created_at)));
        Ok(ready)
    }

    /// Atomically claim a work item (assign + set in_progress).
    /// Returns the updated item, or None if already claimed or not found.
    pub fn claim(&self, id: &str, agent_id: &str) -> Result<Option<WorkItem>, String> {
        let txn = self.db.begin_write().map_err(|e| format!("begin_write: {e}"))?;
        let result = {
            let mut table = txn.open_table(WORK_TABLE).map_err(|e| format!("open_table: {e}"))?;
            // Read → deserialize → drop borrow before writing
            let existing: Option<Vec<u8>> = table.get(id).map_err(|e| format!("get: {e}"))?.map(|v| v.value().to_vec());
            match existing {
                Some(bytes) => {
                    let mut item: WorkItem = serde_json::from_slice(&bytes).map_err(|e| format!("deserialize: {e}"))?;
                    if item.status != Status::Open {
                        debug!("claim rejected: {} is {:?}", id, item.status);
                        None
                    } else {
                        item.claim(agent_id);
                        let new_bytes = serde_json::to_vec(&item).map_err(|e| format!("serialize: {e}"))?;
                        table.insert(id, new_bytes.as_slice()).map_err(|e| format!("insert: {e}"))?;
                        Some(item)
                    }
                }
                None => None,
            }
        };
        txn.commit().map_err(|e| format!("commit: {e}"))?;
        Ok(result)
    }

    /// Update an item's status.
    pub fn update_status(&self, id: &str, status: Status, notes: Option<&str>) -> Result<Option<WorkItem>, String> {
        let txn = self.db.begin_write().map_err(|e| format!("begin_write: {e}"))?;
        let result = {
            let mut table = txn.open_table(WORK_TABLE).map_err(|e| format!("open_table: {e}"))?;
            let existing: Option<Vec<u8>> = table.get(id).map_err(|e| format!("get: {e}"))?.map(|v| v.value().to_vec());
            match existing {
                Some(bytes) => {
                    let mut item: WorkItem = serde_json::from_slice(&bytes).map_err(|e| format!("deserialize: {e}"))?;
                    match status {
                        Status::Done => item.complete(notes),
                        Status::Failed => item.fail(notes.unwrap_or("failed")),
                        Status::Cancelled => item.cancel(notes),
                        Status::InProgress => {
                            item.status = Status::InProgress;
                            item.updated_at = chrono::Utc::now();
                        }
                        Status::Open => {
                            item.status = Status::Open;
                            item.updated_at = chrono::Utc::now();
                        }
                    }
                    let new_bytes = serde_json::to_vec(&item).map_err(|e| format!("serialize: {e}"))?;
                    table.insert(id, new_bytes.as_slice()).map_err(|e| format!("insert: {e}"))?;
                    Some(item)
                }
                None => None,
            }
        };
        txn.commit().map_err(|e| format!("commit: {e}"))?;
        Ok(result)
    }

    /// Get children of a parent item (epic → tasks).
    pub fn children_of(&self, parent_id: &str) -> Result<Vec<WorkItem>, String> {
        Ok(self.list_all()?.into_iter().filter(|i| i.parent_id.as_deref() == Some(parent_id)).collect())
    }

    /// Get items assigned to a specific agent.
    pub fn assigned_to(&self, agent_id: &str) -> Result<Vec<WorkItem>, String> {
        Ok(self
            .list_all()?
            .into_iter()
            .filter(|i| i.assignee.as_deref() == Some(agent_id) && i.status.is_open())
            .collect())
    }

    /// Check for dependency cycles (returns true if adding dep would create a cycle).
    pub fn would_cycle(&self, item_id: &str, dep_id: &str) -> Result<bool, String> {
        let all = self.list_all()?;
        let deps: HashMap<&str, Vec<&str>> =
            all.iter().map(|i| (i.id.as_str(), i.blocked_by.iter().map(|s| s.as_str()).collect())).collect();

        // DFS from dep_id — can we reach item_id?
        let mut visited = HashSet::new();
        let mut stack = vec![dep_id];
        while let Some(current) = stack.pop() {
            if current == item_id {
                return Ok(true);
            }
            if visited.insert(current) {
                if let Some(current_deps) = deps.get(current) {
                    stack.extend(current_deps.iter());
                }
            }
        }
        Ok(false)
    }

    /// Add a dependency (with cycle check).
    pub fn add_dependency(&self, item_id: &str, blocked_by_id: &str) -> Result<(), String> {
        if self.would_cycle(item_id, blocked_by_id)? {
            return Err(format!("adding dependency {item_id} → {blocked_by_id} would create a cycle"));
        }

        let txn = self.db.begin_write().map_err(|e| format!("begin_write: {e}"))?;
        {
            let mut table = txn.open_table(WORK_TABLE).map_err(|e| format!("open_table: {e}"))?;
            let existing: Option<Vec<u8>> =
                table.get(item_id).map_err(|e| format!("get: {e}"))?.map(|v| v.value().to_vec());
            match existing {
                Some(bytes) => {
                    let mut item: WorkItem = serde_json::from_slice(&bytes).map_err(|e| format!("deserialize: {e}"))?;
                    if !item.blocked_by.contains(&blocked_by_id.to_string()) {
                        item.blocked_by.push(blocked_by_id.to_string());
                        item.updated_at = chrono::Utc::now();
                        let new_bytes = serde_json::to_vec(&item).map_err(|e| format!("serialize: {e}"))?;
                        table.insert(item_id, new_bytes.as_slice()).map_err(|e| format!("insert: {e}"))?;
                    }
                }
                None => return Err(format!("item {item_id} not found")),
            }
        }
        txn.commit().map_err(|e| format!("commit: {e}"))?;
        Ok(())
    }

    /// Delete a work item.
    pub fn delete(&self, id: &str) -> Result<bool, String> {
        let txn = self.db.begin_write().map_err(|e| format!("begin_write: {e}"))?;
        let removed = {
            let mut table = txn.open_table(WORK_TABLE).map_err(|e| format!("open_table: {e}"))?;
            table.remove(id).map_err(|e| format!("remove: {e}"))?.is_some()
        };
        txn.commit().map_err(|e| format!("commit: {e}"))?;
        Ok(removed)
    }

    /// Summary stats.
    pub fn stats(&self) -> Result<WorkStats, String> {
        let all = self.list_all()?;
        let mut stats = WorkStats::default();
        for item in &all {
            stats.total += 1;
            match item.status {
                Status::Open => stats.open += 1,
                Status::InProgress => stats.in_progress += 1,
                Status::Done => stats.done += 1,
                Status::Failed => stats.failed += 1,
                Status::Cancelled => stats.cancelled += 1,
            }
        }
        Ok(stats)
    }
}

/// Aggregate work stats.
#[derive(Debug, Default)]
pub struct WorkStats {
    pub total: usize,
    pub open: usize,
    pub in_progress: usize,
    pub done: usize,
    pub failed: usize,
    pub cancelled: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_store() -> (tempfile::TempDir, WorkStore) {
        let tmp = tempfile::TempDir::new().unwrap();
        let store = WorkStore::open(&tmp.path().join("work.redb")).unwrap();
        (tmp, store)
    }

    #[test]
    fn test_put_and_get() {
        let (_tmp, store) = make_store();
        let item = WorkItem::new("test task", Priority::P1);
        let id = item.id.clone();
        store.put(&item).unwrap();

        let loaded = store.get(&id).unwrap().unwrap();
        assert_eq!(loaded.title, "test task");
        assert_eq!(loaded.priority, Priority::P1);
        assert_eq!(loaded.status, Status::Open);
    }

    #[test]
    fn test_list_open() {
        let (_tmp, store) = make_store();

        let mut t1 = WorkItem::new("task 1", Priority::P2);
        let mut t2 = WorkItem::new("task 2", Priority::P0);
        let mut t3 = WorkItem::new("task 3 done", Priority::P1);
        t3.complete(None);

        store.put(&t1).unwrap();
        store.put(&t2).unwrap();
        store.put(&t3).unwrap();

        let open = store.list_open().unwrap();
        assert_eq!(open.len(), 2);
        // Should be sorted by priority: P0 first
        assert_eq!(open[0].priority, Priority::P0);
        assert_eq!(open[1].priority, Priority::P2);
    }

    #[test]
    fn test_ready_with_dependencies() {
        let (_tmp, store) = make_store();

        // dep1 (done) → task (open, blocked_by dep1) → should be ready
        // dep2 (open) → task2 (open, blocked_by dep2) → should NOT be ready

        let mut dep1 = WorkItem::new("dep 1", Priority::P1);
        dep1.complete(None);
        let dep1_id = dep1.id.clone();

        let dep2 = WorkItem::new("dep 2", Priority::P1);
        let dep2_id = dep2.id.clone();

        let task1 = WorkItem::new("ready task", Priority::P1).blocked_by(&[&dep1_id]);
        let task2 = WorkItem::new("blocked task", Priority::P1).blocked_by(&[&dep2_id]);
        let task3 = WorkItem::new("no deps task", Priority::P0);

        store.put(&dep1).unwrap();
        store.put(&dep2).unwrap();
        store.put(&task1).unwrap();
        store.put(&task2).unwrap();
        store.put(&task3).unwrap();

        let ready = store.ready().unwrap();
        let ready_titles: Vec<&str> = ready.iter().map(|i| i.title.as_str()).collect();
        assert!(ready_titles.contains(&"ready task"));
        assert!(ready_titles.contains(&"no deps task"));
        assert!(!ready_titles.contains(&"blocked task"));
        assert!(ready_titles.contains(&"dep 2")); // dep2 is open with no blockers, so it IS ready
    }

    #[test]
    fn test_claim_atomicity() {
        let (_tmp, store) = make_store();

        let item = WorkItem::new("claimable", Priority::P1);
        let id = item.id.clone();
        store.put(&item).unwrap();

        // First claim should succeed
        let claimed = store.claim(&id, "agent-1").unwrap();
        assert!(claimed.is_some());
        assert_eq!(claimed.unwrap().status, Status::InProgress);

        // Second claim should fail (already in_progress)
        let claimed2 = store.claim(&id, "agent-2").unwrap();
        assert!(claimed2.is_none());
    }

    #[test]
    fn test_children_of() {
        let (_tmp, store) = make_store();

        let epic = WorkItem::new("epic", Priority::P1).with_kind(WorkKind::Epic);
        let epic_id = epic.id.clone();

        let child1 = WorkItem::new("child 1", Priority::P2).with_parent(&epic_id);
        let child2 = WorkItem::new("child 2", Priority::P2).with_parent(&epic_id);
        let unrelated = WorkItem::new("unrelated", Priority::P3);

        store.put(&epic).unwrap();
        store.put(&child1).unwrap();
        store.put(&child2).unwrap();
        store.put(&unrelated).unwrap();

        let children = store.children_of(&epic_id).unwrap();
        assert_eq!(children.len(), 2);
    }

    #[test]
    fn test_assigned_to() {
        let (_tmp, store) = make_store();

        let mut t1 = WorkItem::new("task 1", Priority::P1);
        t1.claim("agent-a");
        let mut t2 = WorkItem::new("task 2", Priority::P1);
        t2.claim("agent-b");
        let mut t3 = WorkItem::new("task 3 done", Priority::P1);
        t3.claim("agent-a");
        t3.complete(None);

        store.put(&t1).unwrap();
        store.put(&t2).unwrap();
        store.put(&t3).unwrap();

        let assigned = store.assigned_to("agent-a").unwrap();
        assert_eq!(assigned.len(), 1); // only the open one
        assert_eq!(assigned[0].title, "task 1");
    }

    #[test]
    fn test_cycle_detection() {
        let (_tmp, store) = make_store();

        let a = WorkItem::new("A", Priority::P1);
        let b = WorkItem::new("B", Priority::P1).blocked_by(&[&a.id]);
        let c = WorkItem::new("C", Priority::P1).blocked_by(&[&b.id]);

        store.put(&a).unwrap();
        store.put(&b).unwrap();
        store.put(&c).unwrap();

        // A → B → C; adding C → A would create a cycle
        assert!(store.would_cycle(&a.id, &c.id).unwrap());

        // Adding D → A would NOT create a cycle
        let d = WorkItem::new("D", Priority::P1);
        store.put(&d).unwrap();
        assert!(!store.would_cycle(&d.id, &a.id).unwrap());
    }

    #[test]
    fn test_add_dependency_with_cycle_check() {
        let (_tmp, store) = make_store();

        let a = WorkItem::new("A", Priority::P1);
        let b = WorkItem::new("B", Priority::P1).blocked_by(&[&a.id]);

        store.put(&a).unwrap();
        store.put(&b).unwrap();

        // Adding a → b should fail (b already depends on a, so a depending on b = cycle)
        let result = store.add_dependency(&a.id, &b.id);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cycle"));

        // Adding c → a should succeed
        let c = WorkItem::new("C", Priority::P1);
        store.put(&c).unwrap();
        store.add_dependency(&c.id, &a.id).unwrap();
    }

    #[test]
    fn test_update_status() {
        let (_tmp, store) = make_store();

        let item = WorkItem::new("task", Priority::P1);
        let id = item.id.clone();
        store.put(&item).unwrap();

        store.update_status(&id, Status::Done, Some("all good")).unwrap();
        let loaded = store.get(&id).unwrap().unwrap();
        assert_eq!(loaded.status, Status::Done);
        assert_eq!(loaded.notes.as_deref(), Some("all good"));
    }

    #[test]
    fn test_delete() {
        let (_tmp, store) = make_store();

        let item = WorkItem::new("to delete", Priority::P3);
        let id = item.id.clone();
        store.put(&item).unwrap();

        assert!(store.delete(&id).unwrap());
        assert!(store.get(&id).unwrap().is_none());
        assert!(!store.delete(&id).unwrap());
    }

    #[test]
    fn test_stats() {
        let (_tmp, store) = make_store();

        let mut t1 = WorkItem::new("open", Priority::P1);
        let mut t2 = WorkItem::new("done", Priority::P1);
        t2.complete(None);
        let mut t3 = WorkItem::new("failed", Priority::P1);
        t3.fail("oops");
        let mut t4 = WorkItem::new("in progress", Priority::P1);
        t4.claim("agent");

        store.put(&t1).unwrap();
        store.put(&t2).unwrap();
        store.put(&t3).unwrap();
        store.put(&t4).unwrap();

        let stats = store.stats().unwrap();
        assert_eq!(stats.total, 4);
        assert_eq!(stats.open, 1);
        assert_eq!(stats.done, 1);
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.in_progress, 1);
    }
}
