//! Resource registry — tracks metadata for skills, prompts, and agent definitions.
//!
//! Content stays on disk as markdown files. The registry stores usage stats,
//! enabled/disabled state, and error history. Keyed by `"{kind}:{name}"`:
//!
//! - `skill:napkin`
//! - `prompt:commit`
//! - `agent:worker`
//!
//! # Example
//!
//! ```ignore
//! let db = Db::open(path)?;
//! let reg = db.registry();
//!
//! // Record that the napkin skill was used
//! reg.record_use("skill", "napkin")?;
//!
//! // Disable a skill
//! reg.set_enabled("skill", "context7-cli", false)?;
//!
//! // Get stats for all skills
//! let skills = reg.list_kind("skill")?;
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

/// Table: `"{kind}:{name}"` → serialized `RegistryEntry`
pub(crate) const TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("resource_registry");

/// What kind of resource this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceKind {
    Skill,
    Prompt,
    Agent,
}

impl std::str::FromStr for ResourceKind {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "skill" => Ok(Self::Skill),
            "prompt" => Ok(Self::Prompt),
            "agent" => Ok(Self::Agent),
            _ => Err(format!("Unknown resource kind: {}", s)),
        }
    }
}

impl ResourceKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Skill => "skill",
            Self::Prompt => "prompt",
            Self::Agent => "agent",
        }
    }

    /// Parse a resource kind from a string.
    pub fn parse(s: &str) -> Option<Self> {
        s.parse().ok()
    }
}

impl std::fmt::Display for ResourceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Metadata about a resource. Content lives on disk — this is just stats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    /// Resource kind (skill, prompt, agent).
    pub kind: ResourceKind,
    /// Resource name (e.g. "napkin", "commit", "worker").
    pub name: String,
    /// File path on disk (last known location).
    pub path: String,
    /// Whether this resource is enabled.
    pub enabled: bool,
    /// Total times this resource was used.
    pub use_count: u64,
    /// When it was last used (None if never).
    pub last_used: Option<DateTime<Utc>>,
    /// When this entry was first created.
    pub created_at: DateTime<Utc>,
    /// When this entry was last modified.
    pub updated_at: DateTime<Utc>,
    /// Last error encountered loading/using this resource (None if clean).
    pub last_error: Option<String>,
    /// Total tokens consumed (agents only, 0 for skills/prompts).
    pub total_tokens: u64,
    /// Total cost in USD (agents only, 0.0 for skills/prompts).
    pub total_cost: f64,
}

impl RegistryEntry {
    /// Create a new registry entry for a resource.
    pub fn new(kind: ResourceKind, name: impl Into<String>, path: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            kind,
            name: name.into(),
            path: path.into(),
            enabled: true,
            use_count: 0,
            last_used: None,
            created_at: now,
            updated_at: now,
            last_error: None,
            total_tokens: 0,
            total_cost: 0.0,
        }
    }
}

/// Build the table key from kind and name.
fn make_key(kind: &str, name: &str) -> String {
    format!("{kind}:{name}")
}

/// Accessor for the resource registry table.
pub struct Registry<'db> {
    db: &'db Db,
}

impl<'db> Registry<'db> {
    pub(crate) fn new(db: &'db Db) -> Self {
        Self { db }
    }

    /// Upsert a resource entry. If it exists, updates path and updated_at
    /// but preserves use stats. If new, inserts with defaults.
    pub fn upsert(&self, kind: ResourceKind, name: &str, path: &str) -> Result<()> {
        let key = make_key(kind.as_str(), name);

        let entry = match self.get_by_key(&key)? {
            Some(mut existing) => {
                existing.path = path.to_string();
                existing.updated_at = Utc::now();
                existing
            }
            None => RegistryEntry::new(kind, name, path),
        };

        self.put(&key, &entry)
    }

    /// Record a use of the resource (bumps count and last_used).
    pub fn record_use(&self, kind: &str, name: &str) -> Result<()> {
        let key = make_key(kind, name);

        let Some(mut entry) = self.get_by_key(&key)? else {
            return Ok(()); // Resource not registered — no-op
        };

        entry.use_count += 1;
        entry.last_used = Some(Utc::now());
        entry.updated_at = Utc::now();

        self.put(&key, &entry)
    }

    /// Record token usage and cost (for agent entries).
    pub fn record_tokens(&self, name: &str, tokens: u64, cost: f64) -> Result<()> {
        let key = make_key("agent", name);

        let Some(mut entry) = self.get_by_key(&key)? else {
            return Ok(());
        };

        entry.total_tokens += tokens;
        entry.total_cost += cost;
        entry.updated_at = Utc::now();

        self.put(&key, &entry)
    }

    /// Set enabled/disabled state.
    pub fn set_enabled(&self, kind: &str, name: &str, enabled: bool) -> Result<bool> {
        let key = make_key(kind, name);

        let Some(mut entry) = self.get_by_key(&key)? else {
            return Ok(false);
        };

        entry.enabled = enabled;
        entry.updated_at = Utc::now();

        self.put(&key, &entry)?;
        Ok(true)
    }

    /// Record an error for a resource.
    pub fn record_error(&self, kind: &str, name: &str, error: &str) -> Result<()> {
        let key = make_key(kind, name);

        let Some(mut entry) = self.get_by_key(&key)? else {
            return Ok(());
        };

        entry.last_error = Some(error.to_string());
        entry.updated_at = Utc::now();

        self.put(&key, &entry)
    }

    /// Clear the last error for a resource.
    pub fn clear_error(&self, kind: &str, name: &str) -> Result<()> {
        let key = make_key(kind, name);

        let Some(mut entry) = self.get_by_key(&key)? else {
            return Ok(());
        };

        entry.last_error = None;
        entry.updated_at = Utc::now();

        self.put(&key, &entry)
    }

    /// Get a single entry by kind and name.
    pub fn get(&self, kind: &str, name: &str) -> Result<Option<RegistryEntry>> {
        self.get_by_key(&make_key(kind, name))
    }

    /// List all entries of a given kind.
    pub fn list_kind(&self, kind: &str) -> Result<Vec<RegistryEntry>> {
        let prefix = format!("{kind}:");
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;
        let mut entries = Vec::new();

        for item in table.iter().map_err(db_err)? {
            let (key, value) = item.map_err(db_err)?;
            if key.value().starts_with(&prefix)
                && let Ok(entry) = serde_json::from_slice::<RegistryEntry>(value.value())
            {
                entries.push(entry);
            }
        }

        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    /// List all entries across all kinds.
    pub fn list_all(&self) -> Result<Vec<RegistryEntry>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;
        let mut entries = Vec::new();

        for item in table.iter().map_err(db_err)? {
            let (_key, value) = item.map_err(db_err)?;
            if let Ok(entry) = serde_json::from_slice::<RegistryEntry>(value.value()) {
                entries.push(entry);
            }
        }

        entries.sort_by(|a, b| a.kind.as_str().cmp(b.kind.as_str()).then_with(|| a.name.cmp(&b.name)));
        Ok(entries)
    }

    /// List enabled resources of a given kind.
    pub fn list_enabled(&self, kind: &str) -> Result<Vec<RegistryEntry>> {
        Ok(self.list_kind(kind)?.into_iter().filter(|e| e.enabled).collect())
    }

    /// List disabled resources of a given kind.
    pub fn list_disabled(&self, kind: &str) -> Result<Vec<RegistryEntry>> {
        Ok(self.list_kind(kind)?.into_iter().filter(|e| !e.enabled).collect())
    }

    /// Check if a resource is enabled. Returns true if not registered (default enabled).
    pub fn is_enabled(&self, kind: &str, name: &str) -> Result<bool> {
        match self.get(kind, name)? {
            Some(entry) => Ok(entry.enabled),
            None => Ok(true),
        }
    }

    /// Remove a single entry.
    pub fn remove(&self, kind: &str, name: &str) -> Result<bool> {
        let key = make_key(kind, name);
        let tx = self.db.begin_write()?;
        let was_removed = {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;
            table.remove(key.as_str()).map_err(db_err)?.is_some()
        };
        tx.commit().map_err(db_err)?;
        Ok(was_removed)
    }

    /// Remove all entries of a given kind.
    pub fn clear_kind(&self, kind: &str) -> Result<u64> {
        let prefix = format!("{kind}:");
        let tx = self.db.begin_write()?;
        let mut count = 0u64;
        {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;
            // Collect keys to remove (can't mutate while iterating)
            let keys: Vec<String> = table
                .iter()
                .map_err(db_err)?
                .filter_map(|item| {
                    let (key, _) = item.ok()?;
                    let k = key.value().to_string();
                    k.starts_with(&prefix).then_some(k)
                })
                .collect();

            for key in &keys {
                if table.remove(key.as_str()).map_err(db_err)?.is_some() {
                    count += 1;
                }
            }
        }
        tx.commit().map_err(db_err)?;
        Ok(count)
    }

    /// Total count of all registered resources.
    pub fn count(&self) -> Result<u64> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;
        table.len().map_err(db_err)
    }

    /// Sync the registry against files on disk. Registers new resources,
    /// removes entries whose files no longer exist.
    pub fn sync_from_disk(&self, kind: ResourceKind, resources: &[(&str, &str)]) -> Result<SyncReport> {
        let mut registered = 0u64;
        let mut was_removed = 0u64;

        // Upsert all discovered resources
        for &(name, path) in resources {
            let key = make_key(kind.as_str(), name);
            if self.get_by_key(&key)?.is_none() {
                registered += 1;
            }
            self.upsert(kind, name, path)?;
        }

        // Remove entries that are no longer on disk
        let disk_names: std::collections::HashSet<&str> = resources.iter().map(|(name, _)| *name).collect();
        let existing = self.list_kind(kind.as_str())?;
        for entry in &existing {
            if !disk_names.contains(entry.name.as_str()) {
                self.remove(kind.as_str(), &entry.name)?;
                was_removed += 1;
            }
        }

        Ok(SyncReport {
            kind,
            registered,
            removed: was_removed,
            total: resources.len() as u64,
        })
    }

    // ── Internal helpers ────────────────────────────────────────

    fn get_by_key(&self, key: &str) -> Result<Option<RegistryEntry>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;
        match table.get(key).map_err(db_err)? {
            Some(value) => {
                let entry = serde_json::from_slice(value.value()).map_err(|e| crate::error::DbError {
                    message: format!("failed to deserialize registry entry: {e}"),
                })?;
                Ok(Some(entry))
            }
            None => Ok(None),
        }
    }

    fn put(&self, key: &str, entry: &RegistryEntry) -> Result<()> {
        let bytes = serde_json::to_vec(entry).map_err(|e| crate::error::DbError {
            message: format!("failed to serialize registry entry: {e}"),
        })?;
        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;
            table.insert(key, bytes.as_slice()).map_err(db_err)?;
        }
        tx.commit().map_err(db_err)?;
        Ok(())
    }
}

/// Result of syncing registry against disk.
#[derive(Debug)]
pub struct SyncReport {
    pub kind: ResourceKind,
    pub registered: u64,
    pub removed: u64,
    pub total: u64,
}

impl std::fmt::Display for SyncReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}s: {} total, {} new, {} removed", self.kind, self.total, self.registered, self.removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Result<Db> {
        Db::in_memory()
    }

    #[test]
    fn upsert_and_get() -> Result<()> {
        let db = test_db()?;
        let reg = db.registry();

        reg.upsert(ResourceKind::Skill, "napkin", "/home/user/.clankers/agent/skills/napkin")?;

        let entry = reg.get("skill", "napkin")?.expect("should exist");
        assert_eq!(entry.name, "napkin");
        assert_eq!(entry.kind, ResourceKind::Skill);
        assert!(entry.enabled);
        assert_eq!(entry.use_count, 0);
        assert!(entry.last_used.is_none());
        Ok(())
    }

    #[test]
    fn upsert_preserves_stats() -> Result<()> {
        let db = test_db()?;
        let reg = db.registry();

        reg.upsert(ResourceKind::Skill, "napkin", "/old/path")?;
        reg.record_use("skill", "napkin")?;
        reg.record_use("skill", "napkin")?;

        // Re-upsert with new path
        reg.upsert(ResourceKind::Skill, "napkin", "/new/path")?;

        let entry = reg.get("skill", "napkin")?.expect("should exist");
        assert_eq!(entry.path, "/new/path");
        assert_eq!(entry.use_count, 2); // preserved
        assert!(entry.last_used.is_some()); // preserved
        Ok(())
    }

    #[test]
    fn record_use() -> Result<()> {
        let db = test_db()?;
        let reg = db.registry();

        reg.upsert(ResourceKind::Prompt, "commit", "/path/commit.md")?;
        reg.record_use("prompt", "commit")?;
        reg.record_use("prompt", "commit")?;
        reg.record_use("prompt", "commit")?;

        let entry = reg.get("prompt", "commit")?.expect("should exist");
        assert_eq!(entry.use_count, 3);
        assert!(entry.last_used.is_some());
        Ok(())
    }

    #[test]
    fn record_use_unregistered_is_noop() -> Result<()> {
        let db = test_db()?;
        let reg = db.registry();
        // Should not error on unknown resource
        reg.record_use("skill", "nonexistent")?;
        Ok(())
    }

    #[test]
    fn record_tokens() -> Result<()> {
        let db = test_db()?;
        let reg = db.registry();

        reg.upsert(ResourceKind::Agent, "worker", "/path/worker.md")?;
        reg.record_tokens("worker", 1000, 0.05)?;
        reg.record_tokens("worker", 2000, 0.10)?;

        let entry = reg.get("agent", "worker")?.expect("should exist");
        assert_eq!(entry.total_tokens, 3000);
        assert!((entry.total_cost - 0.15).abs() < f64::EPSILON);
        Ok(())
    }

    #[test]
    fn enable_disable() -> Result<()> {
        let db = test_db()?;
        let reg = db.registry();

        reg.upsert(ResourceKind::Skill, "browser-cli", "/path")?;
        assert!(reg.is_enabled("skill", "browser-cli")?);

        reg.set_enabled("skill", "browser-cli", false)?;
        assert!(!reg.is_enabled("skill", "browser-cli")?);

        reg.set_enabled("skill", "browser-cli", true)?;
        assert!(reg.is_enabled("skill", "browser-cli")?);
        Ok(())
    }

    #[test]
    fn is_enabled_defaults_true_for_unknown() -> Result<()> {
        let db = test_db()?;
        assert!(db.registry().is_enabled("skill", "unknown")?);
        Ok(())
    }

    #[test]
    fn set_enabled_returns_false_for_unknown() -> Result<()> {
        let db = test_db()?;
        assert!(!db.registry().set_enabled("skill", "ghost", false)?);
        Ok(())
    }

    #[test]
    fn error_tracking() -> Result<()> {
        let db = test_db()?;
        let reg = db.registry();

        reg.upsert(ResourceKind::Skill, "broken", "/path")?;

        reg.record_error("skill", "broken", "SKILL.md not found")?;
        let entry = reg.get("skill", "broken")?.expect("should exist");
        assert_eq!(entry.last_error.as_deref(), Some("SKILL.md not found"));

        reg.clear_error("skill", "broken")?;
        let entry = reg.get("skill", "broken")?.expect("should exist");
        assert!(entry.last_error.is_none());
        Ok(())
    }

    #[test]
    fn list_kind() -> Result<()> {
        let db = test_db()?;
        let reg = db.registry();

        reg.upsert(ResourceKind::Skill, "napkin", "/s/napkin")?;
        reg.upsert(ResourceKind::Skill, "nix", "/s/nix")?;
        reg.upsert(ResourceKind::Agent, "worker", "/a/worker")?;
        reg.upsert(ResourceKind::Prompt, "commit", "/p/commit")?;

        let skills = reg.list_kind("skill")?;
        assert_eq!(skills.len(), 2);
        assert_eq!(skills[0].name, "napkin"); // sorted
        assert_eq!(skills[1].name, "nix");

        let agents = reg.list_kind("agent")?;
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].name, "worker");
        Ok(())
    }

    #[test]
    fn list_all() -> Result<()> {
        let db = test_db()?;
        let reg = db.registry();

        reg.upsert(ResourceKind::Skill, "napkin", "/s/napkin")?;
        reg.upsert(ResourceKind::Agent, "worker", "/a/worker")?;
        reg.upsert(ResourceKind::Prompt, "commit", "/p/commit")?;

        let all = reg.list_all()?;
        assert_eq!(all.len(), 3);
        // Sorted by kind then name: agent:worker, prompt:commit, skill:napkin
        assert_eq!(all[0].kind, ResourceKind::Agent);
        assert_eq!(all[1].kind, ResourceKind::Prompt);
        assert_eq!(all[2].kind, ResourceKind::Skill);
        Ok(())
    }

    #[test]
    fn list_enabled_disabled() -> Result<()> {
        let db = test_db()?;
        let reg = db.registry();

        reg.upsert(ResourceKind::Skill, "a", "/a")?;
        reg.upsert(ResourceKind::Skill, "b", "/b")?;
        reg.upsert(ResourceKind::Skill, "c", "/c")?;
        reg.set_enabled("skill", "b", false)?;

        assert_eq!(reg.list_enabled("skill")?.len(), 2);
        assert_eq!(reg.list_disabled("skill")?.len(), 1);
        assert_eq!(reg.list_disabled("skill")?[0].name, "b");
        Ok(())
    }

    #[test]
    fn remove() -> Result<()> {
        let db = test_db()?;
        let reg = db.registry();

        reg.upsert(ResourceKind::Skill, "doomed", "/path")?;
        assert!(reg.remove("skill", "doomed")?);
        assert!(reg.get("skill", "doomed")?.is_none());
        assert!(!reg.remove("skill", "doomed")?); // already gone
        Ok(())
    }

    #[test]
    fn clear_kind() -> Result<()> {
        let db = test_db()?;
        let reg = db.registry();

        reg.upsert(ResourceKind::Skill, "a", "/a")?;
        reg.upsert(ResourceKind::Skill, "b", "/b")?;
        reg.upsert(ResourceKind::Agent, "worker", "/w")?;

        let cleared = reg.clear_kind("skill")?;
        assert_eq!(cleared, 2);
        assert!(reg.list_kind("skill")?.is_empty());
        assert_eq!(reg.list_kind("agent")?.len(), 1); // untouched
        Ok(())
    }

    #[test]
    fn count() -> Result<()> {
        let db = test_db()?;
        let reg = db.registry();

        assert_eq!(reg.count()?, 0);
        reg.upsert(ResourceKind::Skill, "a", "/a")?;
        reg.upsert(ResourceKind::Agent, "b", "/b")?;
        assert_eq!(reg.count()?, 2);
        Ok(())
    }

    #[test]
    fn sync_from_disk() -> Result<()> {
        let db = test_db()?;
        let reg = db.registry();

        // Initial sync: register 3 skills
        let resources = vec![
            ("napkin", "/s/napkin"),
            ("nix", "/s/nix"),
            ("web-fetch", "/s/web-fetch"),
        ];
        let report = reg.sync_from_disk(ResourceKind::Skill, &resources)?;
        assert_eq!(report.total, 3);
        assert_eq!(report.registered, 3);
        assert_eq!(report.removed, 0);

        // Record some usage
        reg.record_use("skill", "napkin")?;

        // Re-sync with one removed and one added
        let resources = vec![
            ("napkin", "/s/napkin"),
            ("nix", "/s/nix"),
            ("tigerstyle", "/s/tigerstyle"),
        ];
        let report = reg.sync_from_disk(ResourceKind::Skill, &resources)?;
        assert_eq!(report.total, 3);
        assert_eq!(report.registered, 1); // tigerstyle is new
        assert_eq!(report.removed, 1); // web-fetch gone

        // napkin stats preserved
        let napkin = reg.get("skill", "napkin")?.expect("should exist");
        assert_eq!(napkin.use_count, 1);

        // web-fetch removed
        assert!(reg.get("skill", "web-fetch")?.is_none());
        Ok(())
    }

    #[test]
    fn sync_from_disk_preserves_enabled_state() -> Result<()> {
        let db = test_db()?;
        let reg = db.registry();

        let resources = vec![("napkin", "/s/napkin")];
        reg.sync_from_disk(ResourceKind::Skill, &resources)?;

        reg.set_enabled("skill", "napkin", false)?;

        // Re-sync — should preserve disabled state
        reg.sync_from_disk(ResourceKind::Skill, &resources)?;
        assert!(!reg.is_enabled("skill", "napkin")?);
        Ok(())
    }

    #[test]
    fn different_kinds_dont_collide() -> Result<()> {
        let db = test_db()?;
        let reg = db.registry();

        // Same name, different kinds
        reg.upsert(ResourceKind::Skill, "review", "/skills/review")?;
        reg.upsert(ResourceKind::Prompt, "review", "/prompts/review")?;
        reg.upsert(ResourceKind::Agent, "review", "/agents/review")?;

        assert_eq!(reg.count()?, 3);

        let skill = reg.get("skill", "review")?.expect("should exist");
        assert_eq!(skill.path, "/skills/review");

        let prompt = reg.get("prompt", "review")?.expect("should exist");
        assert_eq!(prompt.path, "/prompts/review");
        Ok(())
    }

    #[test]
    fn resource_kind_display() {
        assert_eq!(ResourceKind::Skill.to_string(), "skill");
        assert_eq!(ResourceKind::Prompt.to_string(), "prompt");
        assert_eq!(ResourceKind::Agent.to_string(), "agent");
    }

    #[test]
    fn resource_kind_parse() {
        assert_eq!(ResourceKind::parse("skill"), Some(ResourceKind::Skill));
        assert_eq!(ResourceKind::parse("prompt"), Some(ResourceKind::Prompt));
        assert_eq!(ResourceKind::parse("agent"), Some(ResourceKind::Agent));
        assert_eq!(ResourceKind::parse("bogus"), None);
    }

    #[test]
    fn sync_report_display() {
        let report = SyncReport {
            kind: ResourceKind::Skill,
            registered: 2,
            removed: 1,
            total: 5,
        };
        assert_eq!(report.to_string(), "skills: 5 total, 2 new, 1 removed");
    }
}
