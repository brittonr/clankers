//! Work item types

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

/// Short hash-based ID (e.g. "ck-a3f8") to avoid merge conflicts.
pub fn generate_work_id() -> String {
    let raw = crate::util::id::generate_id();
    let short: String = raw.chars().take(5).collect();
    format!("ck-{short}")
}

/// Work item — a trackable unit of work.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkItem {
    /// Unique ID (e.g. "ck-a3f8")
    pub id: String,
    /// Title / description
    pub title: String,
    /// Detailed description or acceptance criteria
    pub description: Option<String>,
    /// Priority level
    pub priority: Priority,
    /// Current status
    pub status: Status,
    /// Kind of work item
    pub kind: WorkKind,
    /// Parent item ID (for epic → task hierarchy)
    pub parent_id: Option<String>,
    /// IDs of items that must be completed before this one
    pub blocked_by: Vec<String>,
    /// Agent identity ID that owns this item (None = unassigned)
    pub assignee: Option<String>,
    /// Free-form tags
    pub tags: Vec<String>,
    /// When created
    pub created_at: DateTime<Utc>,
    /// When last updated
    pub updated_at: DateTime<Utc>,
    /// When work started (status → InProgress)
    pub started_at: Option<DateTime<Utc>>,
    /// When work completed (status → Done/Failed)
    pub completed_at: Option<DateTime<Utc>>,
    /// Session ID where this item was created
    pub created_in_session: Option<String>,
    /// Outcome notes (filled on completion)
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    /// Critical — blocks everything
    P0 = 0,
    /// High — important, do soon
    P1 = 1,
    /// Medium — normal priority
    P2 = 2,
    /// Low — nice to have
    P3 = 3,
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Priority::P0 => write!(f, "P0"),
            Priority::P1 => write!(f, "P1"),
            Priority::P2 => write!(f, "P2"),
            Priority::P3 => write!(f, "P3"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Not started
    Open,
    /// Actively being worked on
    InProgress,
    /// Completed successfully
    Done,
    /// Failed
    Failed,
    /// Cancelled / won't do
    Cancelled,
}

impl Status {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Status::Done | Status::Failed | Status::Cancelled)
    }

    pub fn is_open(&self) -> bool {
        matches!(self, Status::Open | Status::InProgress)
    }
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Status::Open => write!(f, "open"),
            Status::InProgress => write!(f, "in_progress"),
            Status::Done => write!(f, "done"),
            Status::Failed => write!(f, "failed"),
            Status::Cancelled => write!(f, "cancelled"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkKind {
    /// Large body of work with subtasks
    Epic,
    /// Normal task
    Task,
    /// Bug fix
    Bug,
    /// Research / investigation
    Spike,
}

impl Default for WorkKind {
    fn default() -> Self {
        WorkKind::Task
    }
}

impl std::fmt::Display for WorkKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkKind::Epic => write!(f, "epic"),
            WorkKind::Task => write!(f, "task"),
            WorkKind::Bug => write!(f, "bug"),
            WorkKind::Spike => write!(f, "spike"),
        }
    }
}

impl WorkItem {
    /// Create a new work item.
    pub fn new(title: impl Into<String>, priority: Priority) -> Self {
        let now = Utc::now();
        Self {
            id: generate_work_id(),
            title: title.into(),
            description: None,
            priority,
            status: Status::Open,
            kind: WorkKind::Task,
            parent_id: None,
            blocked_by: Vec::new(),
            assignee: None,
            tags: Vec::new(),
            created_at: now,
            updated_at: now,
            started_at: None,
            completed_at: None,
            created_in_session: None,
            notes: None,
        }
    }

    pub fn with_kind(mut self, kind: WorkKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn with_parent(mut self, parent_id: &str) -> Self {
        self.parent_id = Some(parent_id.to_string());
        self
    }

    pub fn blocked_by(mut self, ids: &[&str]) -> Self {
        self.blocked_by = ids.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn with_tags(mut self, tags: &[&str]) -> Self {
        self.tags = tags.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn with_session(mut self, session_id: &str) -> Self {
        self.created_in_session = Some(session_id.to_string());
        self
    }

    /// Claim this item (atomically assign + set in_progress).
    pub fn claim(&mut self, agent_id: &str) {
        self.assignee = Some(agent_id.to_string());
        self.status = Status::InProgress;
        self.started_at = Some(Utc::now());
        self.updated_at = Utc::now();
    }

    /// Mark as done.
    pub fn complete(&mut self, notes: Option<&str>) {
        self.status = Status::Done;
        self.completed_at = Some(Utc::now());
        self.updated_at = Utc::now();
        if let Some(n) = notes {
            self.notes = Some(n.to_string());
        }
    }

    /// Mark as failed.
    pub fn fail(&mut self, reason: &str) {
        self.status = Status::Failed;
        self.completed_at = Some(Utc::now());
        self.updated_at = Utc::now();
        self.notes = Some(reason.to_string());
    }

    /// Mark as cancelled.
    pub fn cancel(&mut self, reason: Option<&str>) {
        self.status = Status::Cancelled;
        self.completed_at = Some(Utc::now());
        self.updated_at = Utc::now();
        if let Some(r) = reason {
            self.notes = Some(r.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_work_id_format() {
        let id = generate_work_id();
        assert!(id.starts_with("ck-"));
        assert!(id.len() >= 5);
    }

    #[test]
    fn test_work_item_lifecycle() {
        let mut item = WorkItem::new("fix bug", Priority::P1)
            .with_kind(WorkKind::Bug)
            .with_description("segfault on startup")
            .with_tags(&["backend", "urgent"]);

        assert_eq!(item.status, Status::Open);
        assert!(item.assignee.is_none());

        item.claim("agent-1");
        assert_eq!(item.status, Status::InProgress);
        assert_eq!(item.assignee.as_deref(), Some("agent-1"));
        assert!(item.started_at.is_some());

        item.complete(Some("fixed the null pointer"));
        assert_eq!(item.status, Status::Done);
        assert!(item.completed_at.is_some());
        assert_eq!(item.notes.as_deref(), Some("fixed the null pointer"));
    }

    #[test]
    fn test_work_item_failure() {
        let mut item = WorkItem::new("deploy", Priority::P0);
        item.claim("agent-2");
        item.fail("connection refused");
        assert_eq!(item.status, Status::Failed);
        assert!(item.status.is_terminal());
        assert!(!item.status.is_open());
    }

    #[test]
    fn test_priority_ordering() {
        assert!(Priority::P0 < Priority::P1);
        assert!(Priority::P1 < Priority::P2);
        assert!(Priority::P2 < Priority::P3);
    }

    #[test]
    fn test_builder_pattern() {
        let item = WorkItem::new("epic task", Priority::P1)
            .with_kind(WorkKind::Epic)
            .with_parent("ck-parent")
            .blocked_by(&["ck-dep1", "ck-dep2"])
            .with_session("sess-123");

        assert_eq!(item.kind, WorkKind::Epic);
        assert_eq!(item.parent_id.as_deref(), Some("ck-parent"));
        assert_eq!(item.blocked_by.len(), 2);
        assert_eq!(item.created_in_session.as_deref(), Some("sess-123"));
    }
}
