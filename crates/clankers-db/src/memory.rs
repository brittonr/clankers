//! Cross-session memory store.
//!
//! Stores learned facts, user preferences, and project knowledge that
//! persists across sessions. Entries are scoped (global or per-project)
//! and tagged for selective retrieval.
//!
//! # Example
//!
//! ```ignore
//! let db = Db::open(path)?;
//! let mem = db.memory();
//!
//! // Agent learns something
//! mem.save(MemoryEntry::new("User prefers snake_case", MemoryScope::Global))?;
//!
//! // Load project-relevant memories into system prompt
//! let entries = mem.list(Some(&MemoryScope::Project("/home/user/myproject".into())))?;
//! ```

use chrono::DateTime;
use chrono::Utc;
use redb::ReadableTable;
use redb::ReadableTableMetadata;
use redb::TableDefinition;
use serde::Deserialize;
use serde::Serialize;

use super::Db;
use crate::error::Result;
use crate::error::db_err;

/// Table: memory_id (u64 millis) → serialized MemoryEntry
pub(crate) const TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new("memories");

/// Where a memory applies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum MemoryScope {
    /// Applies everywhere (user preferences, conventions).
    Global,
    /// Applies to a specific project directory.
    Project { path: String },
}

impl std::fmt::Display for MemoryScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Global => write!(f, "global"),
            Self::Project { path } => write!(f, "project:{path}"),
        }
    }
}

/// How the memory was created.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemorySource {
    /// Explicitly saved by the user (e.g. "remember this").
    User,
    /// Saved by the agent during conversation.
    Agent,
    /// Extracted automatically at session end.
    SessionSummary,
}

/// A single memory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// Unique ID (millisecond timestamp, monotonic).
    pub id: u64,
    /// The fact / preference / knowledge.
    pub text: String,
    /// Where this memory applies.
    pub scope: MemoryScope,
    /// How it was created.
    pub source: MemorySource,
    /// Optional tags for filtering.
    pub tags: Vec<String>,
    /// When it was created.
    pub created_at: DateTime<Utc>,
    /// Session that produced this memory (if any).
    pub session_id: Option<String>,
}

impl MemoryEntry {
    /// Create a new memory entry with a generated ID.
    pub fn new(text: impl Into<String>, scope: MemoryScope) -> Self {
        Self {
            id: generate_id(),
            text: text.into(),
            scope,
            source: MemorySource::Agent,
            tags: Vec::new(),
            created_at: Utc::now(),
            session_id: None,
        }
    }

    /// Builder: set source.
    pub fn with_source(mut self, source: MemorySource) -> Self {
        self.source = source;
        self
    }

    /// Builder: set tags.
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Builder: set session ID.
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }
}

/// Generate a monotonic ID from current time in microseconds.
pub(crate) fn generate_id() -> u64 {
    use std::sync::atomic::AtomicU64;
    use std::sync::atomic::Ordering;

    static LAST: AtomicU64 = AtomicU64::new(0);

    let now = Utc::now().timestamp_micros() as u64;
    let mut last = LAST.load(Ordering::Relaxed);
    loop {
        let next = now.max(last + 1);
        match LAST.compare_exchange_weak(last, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return next,
            Err(actual) => last = actual,
        }
    }
}

/// Accessor for the memory table.
pub struct MemoryStore<'db> {
    db: &'db Db,
}

impl<'db> MemoryStore<'db> {
    pub(crate) fn new(db: &'db Db) -> Self {
        Self { db }
    }

    /// Save a new memory entry.
    pub fn save(&self, entry: &MemoryEntry) -> Result<()> {
        let bytes = serde_json::to_vec(entry).map_err(|e| crate::error::DbError {
            message: format!("failed to serialize memory: {e}"),
        })?;

        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;
            table.insert(entry.id, bytes.as_slice()).map_err(db_err)?;
        }
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// Remove a memory entry by ID.
    pub fn remove(&self, id: u64) -> Result<bool> {
        let tx = self.db.begin_write()?;
        let removed = {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;
            table.remove(id).map_err(db_err)?.is_some()
        };
        tx.commit().map_err(db_err)?;
        Ok(removed)
    }

    /// List memories, optionally filtered by scope.
    pub fn list(&self, scope: Option<&MemoryScope>) -> Result<Vec<MemoryEntry>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;
        let mut entries = Vec::new();

        for item in table.iter().map_err(db_err)? {
            let (_key, value) = item.map_err(db_err)?;
            if let Ok(entry) = serde_json::from_slice::<MemoryEntry>(value.value()) {
                if let Some(filter_scope) = scope
                    && !scope_matches(&entry.scope, filter_scope)
                {
                    continue;
                }
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    /// Update an existing memory entry (replaces the stored value).
    ///
    /// Returns `true` if the entry existed and was updated, `false` if not found.
    pub fn update(&self, entry: &MemoryEntry) -> Result<bool> {
        // Check existence first
        if self.get(entry.id)?.is_none() {
            return Ok(false);
        }

        let bytes = serde_json::to_vec(entry).map_err(|e| crate::error::DbError {
            message: format!("failed to serialize memory: {e}"),
        })?;
        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;
            table.insert(entry.id, bytes.as_slice()).map_err(db_err)?;
        }
        tx.commit().map_err(db_err)?;
        Ok(true)
    }

    /// Get a single memory by ID.
    pub fn get(&self, id: u64) -> Result<Option<MemoryEntry>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;
        match table.get(id).map_err(db_err)? {
            Some(value) => {
                let entry = serde_json::from_slice(value.value()).map_err(|e| crate::error::DbError {
                    message: format!("failed to deserialize memory: {e}"),
                })?;
                Ok(Some(entry))
            }
            None => Ok(None),
        }
    }

    /// Search memories by substring (case-insensitive).
    pub fn search(&self, query: &str) -> Result<Vec<MemoryEntry>> {
        let lower = query.to_lowercase();
        let all = self.list(None)?;
        Ok(all
            .into_iter()
            .filter(|e| {
                e.text.to_lowercase().contains(&lower) || e.tags.iter().any(|t| t.to_lowercase().contains(&lower))
            })
            .collect())
    }

    /// Load all memories relevant to a context (global + matching project).
    /// Returns them formatted as text for injection into the system prompt.
    pub fn context_for(&self, project_path: Option<&str>) -> Result<String> {
        self.context_for_with_limits(project_path, None, None)
    }

    /// Like [`context_for`] but includes capacity headers when limits are provided.
    ///
    /// The header shows usage percentage and char counts so the agent knows
    /// how much room is left before it needs to consolidate.
    pub fn context_for_with_limits(
        &self,
        project_path: Option<&str>,
        global_limit: Option<usize>,
        project_limit: Option<usize>,
    ) -> Result<String> {
        let entries = self.list(None)?;
        if entries.is_empty() {
            return Ok(String::new());
        }

        let mut global = Vec::new();
        let mut project = Vec::new();

        for entry in &entries {
            match &entry.scope {
                MemoryScope::Global => global.push(entry),
                MemoryScope::Project { path } => {
                    if let Some(pp) = project_path
                        && (pp.starts_with(path.as_str()) || path.starts_with(pp))
                    {
                        project.push(entry);
                    }
                }
            }
        }

        if global.is_empty() && project.is_empty() {
            return Ok(String::new());
        }

        let mut out = String::new();

        if !global.is_empty() {
            let chars: usize = global.iter().map(|e| e.text.len()).sum();
            let header = match global_limit {
                Some(limit) if limit > 0 => {
                    let pct = (chars * 100) / limit;
                    format!("MEMORY (general) [{pct}% — {chars}/{limit} chars]")
                }
                _ => "MEMORY (general)".to_string(),
            };
            out.push_str(&format!("# {header}\n"));
            for e in &global {
                out.push_str(&format!("- {}\n", e.text));
            }
        }

        if !project.is_empty() {
            let chars: usize = project.iter().map(|e| e.text.len()).sum();
            let header = match project_limit {
                Some(limit) if limit > 0 => {
                    let pct = (chars * 100) / limit;
                    format!("MEMORY (this project) [{pct}% — {chars}/{limit} chars]")
                }
                _ => "MEMORY (this project)".to_string(),
            };
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&format!("# {header}\n"));
            for e in &project {
                out.push_str(&format!("- {}\n", e.text));
            }
        }

        Ok(out)
    }

    /// Total characters across all memory entries matching the optional scope filter.
    ///
    /// Used for capacity enforcement — the memory tool checks this against
    /// the configured char limit before saving new entries.
    pub fn total_chars(&self, scope: Option<&MemoryScope>) -> Result<usize> {
        let entries = self.list(scope)?;
        Ok(entries.iter().map(|e| e.text.len()).sum())
    }

    /// Count total memories.
    pub fn count(&self) -> Result<u64> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;
        table.len().map_err(db_err)
    }

    /// Remove all memories (for testing / reset).
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

/// Check if an entry's scope matches the filter.
fn scope_matches(entry_scope: &MemoryScope, filter: &MemoryScope) -> bool {
    match (entry_scope, filter) {
        (MemoryScope::Global, MemoryScope::Global) => true,
        (MemoryScope::Project { path: a }, MemoryScope::Project { path: b }) => {
            a.starts_with(b.as_str()) || b.starts_with(a.as_str())
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Result<Db> {
        Db::in_memory()
    }

    #[test]
    fn test_save_and_get() -> Result<()> {
        let db = test_db()?;
        let mem = db.memory();

        let entry = MemoryEntry::new("User prefers tabs", MemoryScope::Global);
        let id = entry.id;
        mem.save(&entry)?;

        let got = mem.get(id)?.expect("entry should exist");
        assert_eq!(got.text, "User prefers tabs");
        assert_eq!(got.scope, MemoryScope::Global);
        Ok(())
    }

    #[test]
    fn test_save_and_list() -> Result<()> {
        let db = test_db()?;
        let mem = db.memory();

        mem.save(&MemoryEntry::new("fact one", MemoryScope::Global))?;
        mem.save(&MemoryEntry::new("fact two", MemoryScope::Global))?;
        mem.save(&MemoryEntry::new("project fact", MemoryScope::Project {
            path: "/home/user/proj".into(),
        }))?;

        assert_eq!(mem.list(None)?.len(), 3);
        assert_eq!(mem.list(Some(&MemoryScope::Global))?.len(), 2);
        assert_eq!(
            mem.list(Some(&MemoryScope::Project {
                path: "/home/user/proj".into()
            }))?
            .len(),
            1
        );
        Ok(())
    }

    #[test]
    fn test_remove() -> Result<()> {
        let db = test_db()?;
        let mem = db.memory();

        let entry = MemoryEntry::new("to delete", MemoryScope::Global);
        let id = entry.id;
        mem.save(&entry)?;
        assert_eq!(mem.count()?, 1);

        assert!(mem.remove(id)?);
        assert_eq!(mem.count()?, 0);
        assert!(mem.get(id)?.is_none());
        Ok(())
    }

    #[test]
    fn test_remove_nonexistent() -> Result<()> {
        let db = test_db()?;
        assert!(!db.memory().remove(999)?);
        Ok(())
    }

    #[test]
    fn test_update() -> Result<()> {
        let db = test_db()?;
        let mem = db.memory();

        let mut entry = MemoryEntry::new("original text", MemoryScope::Global);
        let id = entry.id;
        mem.save(&entry)?;

        entry.text = "updated text".into();
        assert!(mem.update(&entry)?);

        let got = mem.get(id)?.expect("entry should exist");
        assert_eq!(got.text, "updated text");
        Ok(())
    }

    #[test]
    fn test_update_nonexistent() -> Result<()> {
        let db = test_db()?;
        let entry = MemoryEntry::new("ghost", MemoryScope::Global);
        assert!(!db.memory().update(&entry)?);
        Ok(())
    }

    #[test]
    fn test_update_preserves_other_fields() -> Result<()> {
        let db = test_db()?;
        let mem = db.memory();

        let mut entry = MemoryEntry::new("fact", MemoryScope::Global)
            .with_source(MemorySource::User)
            .with_tags(vec!["rust".into()])
            .with_session("sess-1");
        let id = entry.id;
        mem.save(&entry)?;

        entry.text = "updated fact".into();
        mem.update(&entry)?;

        let got = mem.get(id)?.expect("entry should exist");
        assert_eq!(got.text, "updated fact");
        assert_eq!(got.source, MemorySource::User);
        assert_eq!(got.tags, vec!["rust".to_string()]);
        assert_eq!(got.session_id.as_deref(), Some("sess-1"));
        Ok(())
    }

    #[test]
    fn test_search() -> Result<()> {
        let db = test_db()?;
        let mem = db.memory();

        mem.save(&MemoryEntry::new("prefers snake_case", MemoryScope::Global))?;
        mem.save(&MemoryEntry::new("uses pnpm not npm", MemoryScope::Global))?;
        mem.save(&MemoryEntry::new("API uses JWT", MemoryScope::Global))?;

        let results = mem.search("snake")?;
        assert_eq!(results.len(), 1);
        assert!(results[0].text.contains("snake_case"));

        let results = mem.search("npm")?;
        assert_eq!(results.len(), 1);
        Ok(())
    }

    #[test]
    fn test_search_by_tag() -> Result<()> {
        let db = test_db()?;
        let mem = db.memory();

        let entry = MemoryEntry::new("some fact", MemoryScope::Global).with_tags(vec!["style".into(), "rust".into()]);
        mem.save(&entry)?;

        let results = mem.search("style")?;
        assert_eq!(results.len(), 1);
        Ok(())
    }

    #[test]
    fn test_search_case_insensitive() -> Result<()> {
        let db = test_db()?;
        let mem = db.memory();

        mem.save(&MemoryEntry::new("Uses PostgreSQL", MemoryScope::Global))?;

        assert_eq!(mem.search("postgresql")?.len(), 1);
        assert_eq!(mem.search("POSTGRESQL")?.len(), 1);
        Ok(())
    }

    #[test]
    fn test_context_for_empty() -> Result<()> {
        let db = test_db()?;
        let ctx = db.memory().context_for(Some("/any/path"))?;
        assert!(ctx.is_empty());
        Ok(())
    }

    #[test]
    fn test_context_for_mixed_scopes() -> Result<()> {
        let db = test_db()?;
        let mem = db.memory();

        mem.save(&MemoryEntry::new("global pref", MemoryScope::Global))?;
        mem.save(&MemoryEntry::new("project fact", MemoryScope::Project {
            path: "/home/user/proj".into(),
        }))?;
        mem.save(&MemoryEntry::new("other project", MemoryScope::Project {
            path: "/home/user/other".into(),
        }))?;

        let ctx = mem.context_for(Some("/home/user/proj"))?;
        assert!(ctx.contains("global pref"));
        assert!(ctx.contains("project fact"));
        assert!(!ctx.contains("other project"));
        Ok(())
    }

    #[test]
    fn test_clear() -> Result<()> {
        let db = test_db()?;
        let mem = db.memory();

        mem.save(&MemoryEntry::new("one", MemoryScope::Global))?;
        mem.save(&MemoryEntry::new("two", MemoryScope::Global))?;
        assert_eq!(mem.count()?, 2);

        let cleared = mem.clear()?;
        assert_eq!(cleared, 2);
        assert_eq!(mem.count()?, 0);
        Ok(())
    }

    #[test]
    fn test_builder_methods() {
        let entry = MemoryEntry::new("fact", MemoryScope::Global)
            .with_source(MemorySource::User)
            .with_tags(vec!["tag1".into()])
            .with_session("sess-123");

        assert_eq!(entry.source, MemorySource::User);
        assert_eq!(entry.tags, vec!["tag1"]);
        assert_eq!(entry.session_id.as_deref(), Some("sess-123"));
    }

    #[test]
    fn test_ids_are_monotonic() {
        let id1 = generate_id();
        let id2 = generate_id();
        let id3 = generate_id();
        assert!(id1 < id2);
        assert!(id2 < id3);
    }

    #[test]
    fn test_total_chars_empty() -> Result<()> {
        let db = test_db()?;
        assert_eq!(db.memory().total_chars(None)?, 0);
        Ok(())
    }

    #[test]
    fn test_total_chars_global() -> Result<()> {
        let db = test_db()?;
        let mem = db.memory();
        mem.save(&MemoryEntry::new("hello", MemoryScope::Global))?; // 5 chars
        mem.save(&MemoryEntry::new("world!", MemoryScope::Global))?; // 6 chars
        mem.save(&MemoryEntry::new("proj", MemoryScope::Project {
            path: "/p".into(),
        }))?; // 4 chars, different scope

        assert_eq!(mem.total_chars(None)?, 15); // all scopes
        assert_eq!(mem.total_chars(Some(&MemoryScope::Global))?, 11); // global only
        Ok(())
    }

    #[test]
    fn test_scope_display() {
        assert_eq!(MemoryScope::Global.to_string(), "global");
        assert_eq!(MemoryScope::Project { path: "/foo".into() }.to_string(), "project:/foo");
    }
}
