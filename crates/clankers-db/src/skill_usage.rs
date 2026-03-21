//! Skill usage tracking — records when skills are loaded and how they perform.
//!
//! Each entry represents one skill load event in a session. The outcome
//! field is updated after the session to indicate whether the skill helped
//! (success), was corrected by the user, or failed.
//!
//! The agent queries this data via `SkillManageTool::stats` to identify
//! skills that need revision.

use std::cmp::Reverse;

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

/// Table: id (u64 microseconds) → serialized SkillUsageEntry
pub(crate) const TABLE: TableDefinition<u64, &[u8]> = TableDefinition::new("skill_usage");

/// Outcome of a skill usage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkillOutcome {
    /// Skill was loaded but session hasn't ended yet.
    Pending,
    /// Task succeeded while using this skill.
    Success,
    /// User corrected the agent's approach during/after skill use.
    Correction,
    /// Task failed or skill didn't help.
    Failure,
}

impl std::fmt::Display for SkillOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Success => write!(f, "success"),
            Self::Correction => write!(f, "correction"),
            Self::Failure => write!(f, "failure"),
        }
    }
}

/// A single skill usage event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillUsageEntry {
    /// Unique ID (microsecond timestamp).
    pub id: u64,
    /// Skill name.
    pub skill_name: String,
    /// Session that loaded the skill.
    pub session_id: String,
    /// When the skill was loaded.
    pub loaded_at: DateTime<Utc>,
    /// Outcome of the skill usage.
    pub outcome: SkillOutcome,
    /// Optional note about what happened (e.g., "user said to use X instead").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Aggregated stats for a single skill.
#[derive(Debug, Clone)]
pub struct SkillStats {
    pub skill_name: String,
    pub total_loads: usize,
    pub successes: usize,
    pub corrections: usize,
    pub failures: usize,
    pub pending: usize,
    pub last_used: Option<DateTime<Utc>>,
}

impl SkillStats {
    /// Correction rate as a percentage (0-100). Returns 0 if no resolved outcomes.
    pub fn correction_rate(&self) -> f64 {
        let resolved = self.successes + self.corrections + self.failures;
        if resolved == 0 {
            return 0.0;
        }
        (self.corrections as f64 / resolved as f64) * 100.0
    }

    /// Success rate as a percentage (0-100). Returns 0 if no resolved outcomes.
    pub fn success_rate(&self) -> f64 {
        let resolved = self.successes + self.corrections + self.failures;
        if resolved == 0 {
            return 0.0;
        }
        (self.successes as f64 / resolved as f64) * 100.0
    }
}

/// Accessor for the skill usage table.
pub struct SkillUsageStore<'db> {
    db: &'db Db,
}

impl<'db> SkillUsageStore<'db> {
    pub(crate) fn new(db: &'db Db) -> Self {
        Self { db }
    }

    /// Record a skill load event.
    pub fn record(&self, entry: &SkillUsageEntry) -> Result<()> {
        let bytes = serde_json::to_vec(entry).map_err(|e| crate::error::DbError {
            message: format!("failed to serialize skill usage: {e}"),
        })?;

        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;
            table.insert(entry.id, bytes.as_slice()).map_err(db_err)?;
        }
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// Record a new skill load (convenience wrapper).
    pub fn record_load(&self, skill_name: &str, session_id: &str) -> Result<u64> {
        let id = crate::memory::generate_id();
        let entry = SkillUsageEntry {
            id,
            skill_name: skill_name.to_string(),
            session_id: session_id.to_string(),
            loaded_at: Utc::now(),
            outcome: SkillOutcome::Pending,
            note: None,
        };
        self.record(&entry)?;
        Ok(id)
    }

    /// Update the outcome for a usage entry.
    pub fn set_outcome(&self, id: u64, outcome: SkillOutcome, note: Option<String>) -> Result<bool> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;
        let entry_data = match table.get(id).map_err(db_err)? {
            Some(v) => v.value().to_vec(),
            None => return Ok(false),
        };
        drop(table);
        drop(tx);

        let mut entry: SkillUsageEntry = serde_json::from_slice(&entry_data).map_err(|e| crate::error::DbError {
            message: format!("failed to deserialize skill usage: {e}"),
        })?;
        entry.outcome = outcome;
        entry.note = note;

        let bytes = serde_json::to_vec(&entry).map_err(|e| crate::error::DbError {
            message: format!("failed to serialize skill usage: {e}"),
        })?;

        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;
            table.insert(id, bytes.as_slice()).map_err(db_err)?;
        }
        tx.commit().map_err(db_err)?;
        Ok(true)
    }

    /// Get stats for a specific skill.
    pub fn stats_for(&self, skill_name: &str) -> Result<SkillStats> {
        let entries = self.entries_for(skill_name)?;
        Ok(compute_stats(skill_name, &entries))
    }

    /// Get stats for all skills that have usage data.
    pub fn all_stats(&self) -> Result<Vec<SkillStats>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;

        let mut by_skill: std::collections::HashMap<String, Vec<SkillUsageEntry>> = std::collections::HashMap::new();

        for item in table.iter().map_err(db_err)? {
            let (_key, value) = item.map_err(db_err)?;
            if let Ok(entry) = serde_json::from_slice::<SkillUsageEntry>(value.value()) {
                by_skill.entry(entry.skill_name.clone()).or_default().push(entry);
            }
        }

        let mut stats: Vec<SkillStats> = by_skill
            .iter()
            .map(|(name, entries)| compute_stats(name, entries))
            .collect();

        // Sort by total loads descending
        stats.sort_by_key(|s| Reverse(s.total_loads));
        Ok(stats)
    }

    /// Get all entries for a specific skill, newest first.
    pub fn entries_for(&self, skill_name: &str) -> Result<Vec<SkillUsageEntry>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;

        let mut entries = Vec::new();
        for item in table.iter().map_err(db_err)? {
            let (_key, value) = item.map_err(db_err)?;
            if let Ok(entry) = serde_json::from_slice::<SkillUsageEntry>(value.value())
                && entry.skill_name == skill_name
            {
                entries.push(entry);
            }
        }

        entries.sort_by_key(|e| Reverse(e.loaded_at));
        Ok(entries)
    }

    /// Count total usage entries.
    pub fn count(&self) -> Result<u64> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;
        table.len().map_err(db_err)
    }

    /// Clear all entries (for testing).
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

fn compute_stats(skill_name: &str, entries: &[SkillUsageEntry]) -> SkillStats {
    let mut stats = SkillStats {
        skill_name: skill_name.to_string(),
        total_loads: entries.len(),
        successes: 0,
        corrections: 0,
        failures: 0,
        pending: 0,
        last_used: None,
    };

    for e in entries {
        match e.outcome {
            SkillOutcome::Success => stats.successes += 1,
            SkillOutcome::Correction => stats.corrections += 1,
            SkillOutcome::Failure => stats.failures += 1,
            SkillOutcome::Pending => stats.pending += 1,
        }
        if stats.last_used.is_none() || Some(e.loaded_at) > stats.last_used {
            stats.last_used = Some(e.loaded_at);
        }
    }
    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Result<Db> {
        Db::in_memory()
    }

    #[test]
    fn test_record_and_query() -> Result<()> {
        let db = test_db()?;
        let store = db.skill_usage();

        let id = store.record_load("deploy-k8s", "sess-1")?;
        assert!(id > 0);

        let entries = store.entries_for("deploy-k8s")?;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].skill_name, "deploy-k8s");
        assert_eq!(entries[0].session_id, "sess-1");
        assert_eq!(entries[0].outcome, SkillOutcome::Pending);
        Ok(())
    }

    #[test]
    fn test_set_outcome() -> Result<()> {
        let db = test_db()?;
        let store = db.skill_usage();

        let id = store.record_load("debug-rust", "sess-1")?;
        store.set_outcome(id, SkillOutcome::Success, None)?;

        let entries = store.entries_for("debug-rust")?;
        assert_eq!(entries[0].outcome, SkillOutcome::Success);
        Ok(())
    }

    #[test]
    fn test_set_outcome_with_note() -> Result<()> {
        let db = test_db()?;
        let store = db.skill_usage();

        let id = store.record_load("deploy-k8s", "sess-1")?;
        store.set_outcome(id, SkillOutcome::Correction, Some("user said use helm".into()))?;

        let entries = store.entries_for("deploy-k8s")?;
        assert_eq!(entries[0].outcome, SkillOutcome::Correction);
        assert_eq!(entries[0].note.as_deref(), Some("user said use helm"));
        Ok(())
    }

    #[test]
    fn test_set_outcome_nonexistent() -> Result<()> {
        let db = test_db()?;
        assert!(!db.skill_usage().set_outcome(999, SkillOutcome::Success, None)?);
        Ok(())
    }

    #[test]
    fn test_stats_for() -> Result<()> {
        let db = test_db()?;
        let store = db.skill_usage();

        let id1 = store.record_load("my-skill", "s1")?;
        let id2 = store.record_load("my-skill", "s2")?;
        let id3 = store.record_load("my-skill", "s3")?;
        let _id4 = store.record_load("my-skill", "s4")?;

        store.set_outcome(id1, SkillOutcome::Success, None)?;
        store.set_outcome(id2, SkillOutcome::Success, None)?;
        store.set_outcome(id3, SkillOutcome::Correction, None)?;
        // id4 stays Pending

        let stats = store.stats_for("my-skill")?;
        assert_eq!(stats.total_loads, 4);
        assert_eq!(stats.successes, 2);
        assert_eq!(stats.corrections, 1);
        assert_eq!(stats.pending, 1);
        assert!((stats.success_rate() - 66.6).abs() < 1.0);
        assert!((stats.correction_rate() - 33.3).abs() < 1.0);
        Ok(())
    }

    #[test]
    fn test_stats_empty() -> Result<()> {
        let db = test_db()?;
        let stats = db.skill_usage().stats_for("nonexistent")?;
        assert_eq!(stats.total_loads, 0);
        assert_eq!(stats.success_rate(), 0.0);
        Ok(())
    }

    #[test]
    fn test_all_stats() -> Result<()> {
        let db = test_db()?;
        let store = db.skill_usage();

        store.record_load("skill-a", "s1")?;
        store.record_load("skill-a", "s2")?;
        store.record_load("skill-b", "s1")?;

        let all = store.all_stats()?;
        assert_eq!(all.len(), 2);
        // Sorted by total loads descending
        assert_eq!(all[0].skill_name, "skill-a");
        assert_eq!(all[0].total_loads, 2);
        assert_eq!(all[1].skill_name, "skill-b");
        assert_eq!(all[1].total_loads, 1);
        Ok(())
    }

    #[test]
    fn test_count_and_clear() -> Result<()> {
        let db = test_db()?;
        let store = db.skill_usage();

        store.record_load("a", "s1")?;
        store.record_load("b", "s2")?;
        assert_eq!(store.count()?, 2);

        store.clear()?;
        assert_eq!(store.count()?, 0);
        Ok(())
    }
}
