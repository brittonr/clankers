//! Persistent agent identity
//!
//! Gives subagents and delegates durable identities that survive across
//! sessions. Each identity tracks:
//!
//! - **Name + capability tags** — what this agent is good at
//! - **Work history** — what tasks it has completed, with outcomes
//! - **Session log** — which session IDs this identity has been used in
//! - **Stats** — total tasks, success rate, average duration
//!
//! This is the clankers equivalent of Gas Town's "Polecat" persistent identity.
//! Identities live in redb alongside the session store.

use std::path::Path;
use std::time::Duration;

use chrono::DateTime;
use chrono::Utc;
use redb::ReadableTable;
use serde::Deserialize;
use serde::Serialize;

/// A persistent agent identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdentity {
    /// Unique stable ID (persists across sessions)
    pub id: String,
    /// Human-readable name (e.g. "scout-1", "builder-alpha")
    pub name: String,
    /// Agent definition used (e.g. "scout", "default")
    pub agent_type: String,
    /// Capability tags for routing (e.g. ["rust", "testing", "frontend"])
    pub tags: Vec<String>,
    /// When this identity was first created
    pub created_at: DateTime<Utc>,
    /// When this identity was last active
    pub last_active: DateTime<Utc>,
    /// Completed work items
    pub work_history: Vec<WorkRecord>,
    /// Session IDs this identity has participated in
    pub session_ids: Vec<String>,
    /// Aggregate stats
    pub stats: AgentStats,
}

/// A single completed work item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkRecord {
    /// Task description (truncated)
    pub task: String,
    /// When the task started
    pub started_at: DateTime<Utc>,
    /// When the task completed
    pub completed_at: DateTime<Utc>,
    /// Outcome
    pub outcome: WorkOutcome,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Session ID where this work was done
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkOutcome {
    Success,
    Failure,
    Timeout,
    Cancelled,
}

impl std::fmt::Display for WorkOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkOutcome::Success => write!(f, "success"),
            WorkOutcome::Failure => write!(f, "failure"),
            WorkOutcome::Timeout => write!(f, "timeout"),
            WorkOutcome::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Aggregate performance stats.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentStats {
    pub total_tasks: u64,
    pub successes: u64,
    pub failures: u64,
    pub timeouts: u64,
    pub total_duration_ms: u64,
}

impl AgentStats {
    pub fn success_rate(&self) -> f64 {
        if self.total_tasks == 0 {
            return 0.0;
        }
        self.successes as f64 / self.total_tasks as f64
    }

    pub fn avg_duration(&self) -> Duration {
        if self.total_tasks == 0 {
            return Duration::ZERO;
        }
        Duration::from_millis(self.total_duration_ms / self.total_tasks)
    }

    fn record(&mut self, outcome: &WorkOutcome, duration_ms: u64) {
        self.total_tasks += 1;
        self.total_duration_ms += duration_ms;
        match outcome {
            WorkOutcome::Success => self.successes += 1,
            WorkOutcome::Failure => self.failures += 1,
            WorkOutcome::Timeout => self.timeouts += 1,
            WorkOutcome::Cancelled => {} // don't count cancellations as failure
        }
    }
}

impl AgentIdentity {
    /// Create a new identity.
    pub fn new(name: impl Into<String>, agent_type: impl Into<String>) -> Self {
        let id = format!("agent-{}", crate::util::id::generate_id());
        let now = Utc::now();
        Self {
            id,
            name: name.into(),
            agent_type: agent_type.into(),
            tags: Vec::new(),
            created_at: now,
            last_active: now,
            work_history: Vec::new(),
            session_ids: Vec::new(),
            stats: AgentStats::default(),
        }
    }

    /// Add capability tags.
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags.extend(tags.into_iter().map(Into::into));
        self
    }

    /// Record a completed work item.
    pub fn record_work(&mut self, task: &str, outcome: WorkOutcome, duration: Duration, session_id: Option<&str>) {
        let now = Utc::now();
        let duration_ms = duration.as_millis() as u64;

        // Truncate task to 200 chars for storage
        let task_truncated: String = task.chars().take(200).collect();

        self.work_history.push(WorkRecord {
            task: task_truncated,
            started_at: now - duration,
            completed_at: now,
            outcome: outcome.clone(),
            duration_ms,
            session_id: session_id.map(String::from),
        });

        self.stats.record(&outcome, duration_ms);
        self.last_active = now;

        // Keep only last 100 work records to avoid unbounded growth
        if self.work_history.len() > 100 {
            self.work_history.drain(..self.work_history.len() - 100);
        }
    }

    /// Associate a session with this identity.
    pub fn add_session(&mut self, session_id: &str) {
        if !self.session_ids.contains(&session_id.to_string()) {
            self.session_ids.push(session_id.to_string());
            self.last_active = Utc::now();
        }
        // Keep only last 50 session IDs
        if self.session_ids.len() > 50 {
            self.session_ids.drain(..self.session_ids.len() - 50);
        }
    }

    /// Check if this agent has a specific tag.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t.eq_ignore_ascii_case(tag))
    }
}

// ── Persistent store ────────────────────────────────────────────────────

const IDENTITY_TABLE: redb::TableDefinition<&str, &[u8]> = redb::TableDefinition::new("agent_identities");

/// redb-backed persistent store for agent identities.
pub struct IdentityStore {
    db: redb::Database,
}

impl IdentityStore {
    /// Open or create the identity store.
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
        }
        let db = redb::Database::create(path).map_err(|e| format!("redb open: {e}"))?;

        // Ensure table exists
        let txn = db.begin_write().map_err(|e| format!("begin_write: {e}"))?;
        {
            let _ = txn.open_table(IDENTITY_TABLE).map_err(|e| format!("open_table: {e}"))?;
        }
        txn.commit().map_err(|e| format!("commit: {e}"))?;

        Ok(Self { db })
    }

    /// Get an identity by ID.
    pub fn get(&self, id: &str) -> Result<Option<AgentIdentity>, String> {
        let txn = self.db.begin_read().map_err(|e| format!("begin_read: {e}"))?;
        let table = txn.open_table(IDENTITY_TABLE).map_err(|e| format!("open_table: {e}"))?;

        match table.get(id).map_err(|e| format!("get: {e}"))? {
            Some(bytes) => {
                let identity: AgentIdentity =
                    serde_json::from_slice(bytes.value()).map_err(|e| format!("deserialize: {e}"))?;
                Ok(Some(identity))
            }
            None => Ok(None),
        }
    }

    /// Save (upsert) an identity.
    pub fn put(&self, identity: &AgentIdentity) -> Result<(), String> {
        let bytes = serde_json::to_vec(identity).map_err(|e| format!("serialize: {e}"))?;
        let txn = self.db.begin_write().map_err(|e| format!("begin_write: {e}"))?;
        {
            let mut table = txn.open_table(IDENTITY_TABLE).map_err(|e| format!("open_table: {e}"))?;
            table.insert(identity.id.as_str(), bytes.as_slice()).map_err(|e| format!("insert: {e}"))?;
        }
        txn.commit().map_err(|e| format!("commit: {e}"))?;
        Ok(())
    }

    /// List all identities, sorted by last_active (newest first).
    pub fn list(&self) -> Result<Vec<AgentIdentity>, String> {
        let txn = self.db.begin_read().map_err(|e| format!("begin_read: {e}"))?;
        let table = txn.open_table(IDENTITY_TABLE).map_err(|e| format!("open_table: {e}"))?;

        let mut identities = Vec::new();
        let iter = table.iter().map_err(|e| format!("iter: {e}"))?;
        for entry in iter {
            let (_, value) = entry.map_err(|e| format!("entry: {e}"))?;
            let identity: AgentIdentity =
                serde_json::from_slice(value.value()).map_err(|e| format!("deserialize: {e}"))?;
            identities.push(identity);
        }

        identities.sort_by_key(|i| std::cmp::Reverse(i.last_active));
        Ok(identities)
    }

    /// Find identities matching a tag.
    pub fn find_by_tag(&self, tag: &str) -> Result<Vec<AgentIdentity>, String> {
        Ok(self.list()?.into_iter().filter(|i| i.has_tag(tag)).collect())
    }

    /// Find the best agent for a capability tag, ranked by success rate.
    pub fn best_for_tag(&self, tag: &str) -> Result<Option<AgentIdentity>, String> {
        let mut candidates = self.find_by_tag(tag)?;
        candidates.sort_by(|a, b| {
            b.stats.success_rate().partial_cmp(&a.stats.success_rate()).unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(candidates.into_iter().next())
    }

    /// Delete an identity.
    pub fn delete(&self, id: &str) -> Result<bool, String> {
        let txn = self.db.begin_write().map_err(|e| format!("begin_write: {e}"))?;
        let removed = {
            let mut table = txn.open_table(IDENTITY_TABLE).map_err(|e| format!("open_table: {e}"))?;
            table.remove(id).map_err(|e| format!("remove: {e}"))?.is_some()
        };
        txn.commit().map_err(|e| format!("commit: {e}"))?;
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_creation() {
        let agent = AgentIdentity::new("builder-1", "default").with_tags(["rust", "backend"]);
        assert!(agent.id.starts_with("agent-"));
        assert_eq!(agent.name, "builder-1");
        assert_eq!(agent.agent_type, "default");
        assert!(agent.has_tag("rust"));
        assert!(agent.has_tag("RUST")); // case insensitive
        assert!(!agent.has_tag("frontend"));
    }

    #[test]
    fn test_record_work() {
        let mut agent = AgentIdentity::new("worker", "default");

        agent.record_work("fix bug #123", WorkOutcome::Success, Duration::from_secs(30), Some("sess-1"));
        agent.record_work("add tests", WorkOutcome::Success, Duration::from_secs(60), Some("sess-1"));
        agent.record_work("deploy", WorkOutcome::Failure, Duration::from_secs(10), None);

        assert_eq!(agent.stats.total_tasks, 3);
        assert_eq!(agent.stats.successes, 2);
        assert_eq!(agent.stats.failures, 1);
        assert!((agent.stats.success_rate() - 0.6667).abs() < 0.01);
        assert_eq!(agent.work_history.len(), 3);
    }

    #[test]
    fn test_work_history_capped() {
        let mut agent = AgentIdentity::new("worker", "default");
        for i in 0..150 {
            agent.record_work(&format!("task {i}"), WorkOutcome::Success, Duration::from_secs(1), None);
        }
        assert_eq!(agent.work_history.len(), 100);
        assert_eq!(agent.stats.total_tasks, 150);
    }

    #[test]
    fn test_session_tracking() {
        let mut agent = AgentIdentity::new("worker", "default");
        agent.add_session("sess-1");
        agent.add_session("sess-2");
        agent.add_session("sess-1"); // duplicate
        assert_eq!(agent.session_ids.len(), 2);
    }

    #[test]
    fn test_identity_store_roundtrip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let store = IdentityStore::open(&tmp.path().join("agents.redb")).unwrap();

        let mut agent = AgentIdentity::new("scout-1", "scout").with_tags(["rust", "grep"]);
        agent.record_work("find TODO", WorkOutcome::Success, Duration::from_secs(5), None);

        store.put(&agent).unwrap();

        let loaded = store.get(&agent.id).unwrap().unwrap();
        assert_eq!(loaded.name, "scout-1");
        assert_eq!(loaded.stats.total_tasks, 1);
        assert!(loaded.has_tag("rust"));
    }

    #[test]
    fn test_identity_store_list_and_find() {
        let tmp = tempfile::TempDir::new().unwrap();
        let store = IdentityStore::open(&tmp.path().join("agents.redb")).unwrap();

        let a1 = AgentIdentity::new("rust-dev", "builder").with_tags(["rust", "backend"]);
        let a2 = AgentIdentity::new("js-dev", "builder").with_tags(["javascript", "frontend"]);
        let a3 = AgentIdentity::new("fullstack", "builder").with_tags(["rust", "javascript"]);

        store.put(&a1).unwrap();
        store.put(&a2).unwrap();
        store.put(&a3).unwrap();

        assert_eq!(store.list().unwrap().len(), 3);
        assert_eq!(store.find_by_tag("rust").unwrap().len(), 2);
        assert_eq!(store.find_by_tag("javascript").unwrap().len(), 2);
        assert_eq!(store.find_by_tag("python").unwrap().len(), 0);
    }

    #[test]
    fn test_identity_store_best_for_tag() {
        let tmp = tempfile::TempDir::new().unwrap();
        let store = IdentityStore::open(&tmp.path().join("agents.redb")).unwrap();

        let mut good = AgentIdentity::new("good-worker", "default").with_tags(["rust"]);
        for _ in 0..10 {
            good.record_work("task", WorkOutcome::Success, Duration::from_secs(1), None);
        }

        let mut mediocre = AgentIdentity::new("mediocre-worker", "default").with_tags(["rust"]);
        for _ in 0..5 {
            mediocre.record_work("task", WorkOutcome::Success, Duration::from_secs(1), None);
        }
        for _ in 0..5 {
            mediocre.record_work("task", WorkOutcome::Failure, Duration::from_secs(1), None);
        }

        store.put(&good).unwrap();
        store.put(&mediocre).unwrap();

        let best = store.best_for_tag("rust").unwrap().unwrap();
        assert_eq!(best.name, "good-worker");
        assert!((best.stats.success_rate() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_identity_store_delete() {
        let tmp = tempfile::TempDir::new().unwrap();
        let store = IdentityStore::open(&tmp.path().join("agents.redb")).unwrap();

        let agent = AgentIdentity::new("ephemeral", "default");
        let id = agent.id.clone();
        store.put(&agent).unwrap();

        assert!(store.get(&id).unwrap().is_some());
        assert!(store.delete(&id).unwrap());
        assert!(store.get(&id).unwrap().is_none());
        assert!(!store.delete(&id).unwrap()); // already gone
    }

    #[test]
    fn test_agent_stats_defaults() {
        let stats = AgentStats::default();
        assert_eq!(stats.success_rate(), 0.0);
        assert_eq!(stats.avg_duration(), Duration::ZERO);
    }

    #[test]
    fn test_identity_serialization() {
        let agent = AgentIdentity::new("test", "default").with_tags(["a", "b"]);
        let json = serde_json::to_string(&agent).unwrap();
        let parsed: AgentIdentity = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "test");
        assert_eq!(parsed.tags, vec!["a", "b"]);
    }
}
