//! Metrics redb tables and persistence.

use redb::ReadableTable;
use redb::TableDefinition;

use super::types::DailyMetricsRollup;
use super::types::MetricEventRecord;
use super::types::SessionMetricsSummary;
use crate::Db;
use crate::error::Result;
use crate::error::db_err;

pub(crate) const SESSION_SUMMARY_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("metrics_session_summary");

pub(crate) const DAILY_ROLLUP_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("metrics_daily_rollup");

pub(crate) const RECENT_EVENTS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("metrics_recent_events");

const MAX_RECENT_EVENTS_PER_SESSION: usize = 500;
const EVICTION_BATCH_FRACTION: usize = 10;

pub struct MetricsStore<'db> {
    db: &'db Db,
}

impl<'db> MetricsStore<'db> {
    pub(crate) fn new(db: &'db Db) -> Self {
        Self { db }
    }

    // ── Session summaries ───────────────────────────────────────────

    pub fn save_session_summary(&self, summary: &SessionMetricsSummary) -> Result<()> {
        let bytes = serde_json::to_vec(summary).map_err(|e| crate::error::DbError {
            message: format!("failed to serialize session summary: {e}"),
        })?;
        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(SESSION_SUMMARY_TABLE).map_err(db_err)?;
            table.insert(summary.session_id.as_str(), bytes.as_slice()).map_err(db_err)?;
        }
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    pub fn get_session_summary(&self, session_id: &str) -> Result<Option<SessionMetricsSummary>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(SESSION_SUMMARY_TABLE).map_err(db_err)?;
        match table.get(session_id).map_err(db_err)? {
            Some(value) => {
                let summary = serde_json::from_slice(value.value()).map_err(|e| crate::error::DbError {
                    message: format!("failed to deserialize session summary: {e}"),
                })?;
                Ok(Some(summary))
            }
            None => Ok(None),
        }
    }

    pub fn list_session_summaries(&self, limit: usize) -> Result<Vec<SessionMetricsSummary>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(SESSION_SUMMARY_TABLE).map_err(db_err)?;
        let mut out = Vec::new();
        for item in table.iter().map_err(db_err)?.rev() {
            if out.len() >= limit {
                break;
            }
            let (_key, value) = item.map_err(db_err)?;
            if let Ok(s) = serde_json::from_slice::<SessionMetricsSummary>(value.value()) {
                out.push(s);
            }
        }
        Ok(out)
    }

    // ── Daily rollups ───────────────────────────────────────────────

    pub fn save_daily_rollup(&self, rollup: &DailyMetricsRollup) -> Result<()> {
        let bytes = serde_json::to_vec(rollup).map_err(|e| crate::error::DbError {
            message: format!("failed to serialize daily rollup: {e}"),
        })?;
        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(DAILY_ROLLUP_TABLE).map_err(db_err)?;
            table.insert(rollup.date.as_str(), bytes.as_slice()).map_err(db_err)?;
        }
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    pub fn get_daily_rollup(&self, date: &str) -> Result<Option<DailyMetricsRollup>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(DAILY_ROLLUP_TABLE).map_err(db_err)?;
        match table.get(date).map_err(db_err)? {
            Some(value) => {
                let rollup = serde_json::from_slice(value.value()).map_err(|e| crate::error::DbError {
                    message: format!("failed to deserialize daily rollup: {e}"),
                })?;
                Ok(Some(rollup))
            }
            None => Ok(None),
        }
    }

    pub fn recent_daily_rollups(&self, n: usize) -> Result<Vec<DailyMetricsRollup>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(DAILY_ROLLUP_TABLE).map_err(db_err)?;
        let mut out = Vec::new();
        for item in table.iter().map_err(db_err)?.rev() {
            if out.len() >= n {
                break;
            }
            let (_key, value) = item.map_err(db_err)?;
            if let Ok(r) = serde_json::from_slice::<DailyMetricsRollup>(value.value()) {
                out.push(r);
            }
        }
        Ok(out)
    }

    // ── Recent events ───────────────────────────────────────────────

    pub fn append_recent_event(&self, event: &MetricEventRecord) -> Result<()> {
        let key = format!("{}:{:06}", event.session_id, event.seq);
        let bytes = serde_json::to_vec(event).map_err(|e| crate::error::DbError {
            message: format!("failed to serialize metric event: {e}"),
        })?;

        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(RECENT_EVENTS_TABLE).map_err(db_err)?;
            table.insert(key.as_str(), bytes.as_slice()).map_err(db_err)?;

            self.maybe_evict_in_tx(&mut table, &event.session_id)?;
        }
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    fn maybe_evict_in_tx(&self, table: &mut redb::Table<&str, &[u8]>, session_id: &str) -> Result<()> {
        let prefix = format!("{session_id}:");
        let count = table
            .range(prefix.as_str()..)
            .map_err(db_err)?
            .take_while(|r| r.as_ref().map(|(k, _)| k.value().starts_with(&prefix)).unwrap_or(false))
            .count();

        if count <= MAX_RECENT_EVENTS_PER_SESSION {
            return Ok(());
        }

        let to_evict = count / EVICTION_BATCH_FRACTION;
        let keys_to_remove: Vec<String> = table
            .range(prefix.as_str()..)
            .map_err(db_err)?
            .take_while(|r| r.as_ref().map(|(k, _)| k.value().starts_with(&prefix)).unwrap_or(false))
            .take(to_evict)
            .filter_map(|r| r.ok().map(|(k, _)| k.value().to_string()))
            .collect();

        for key in &keys_to_remove {
            table.remove(key.as_str()).map_err(db_err)?;
        }

        Ok(())
    }

    pub fn recent_events_for_session(&self, session_id: &str, limit: usize) -> Result<Vec<MetricEventRecord>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(RECENT_EVENTS_TABLE).map_err(db_err)?;
        let prefix = format!("{session_id}:");
        let mut out = Vec::new();
        for item in table.range(prefix.as_str()..).map_err(db_err)? {
            let (key, value) = item.map_err(db_err)?;
            if !key.value().starts_with(&prefix) {
                break;
            }
            if out.len() >= limit {
                break;
            }
            if let Ok(e) = serde_json::from_slice::<MetricEventRecord>(value.value()) {
                out.push(e);
            }
        }
        Ok(out)
    }

    pub fn clear_all(&self) -> Result<()> {
        let tx = self.db.begin_write()?;
        {
            let mut t1 = tx.open_table(SESSION_SUMMARY_TABLE).map_err(db_err)?;
            t1.retain(|_, _| false).map_err(db_err)?;
            let mut t2 = tx.open_table(DAILY_ROLLUP_TABLE).map_err(db_err)?;
            t2.retain(|_, _| false).map_err(db_err)?;
            let mut t3 = tx.open_table(RECENT_EVENTS_TABLE).map_err(db_err)?;
            t3.retain(|_, _| false).map_err(db_err)?;
        }
        tx.commit().map_err(db_err)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::metrics::types::MetricEventKind;

    fn test_db() -> Result<Db> {
        Db::in_memory()
    }

    #[test]
    fn session_summary_roundtrip() -> Result<()> {
        let db = test_db()?;
        let store = db.metrics();
        let mut summary = SessionMetricsSummary::new("sess-1".into());
        summary.turns_total = 5;
        summary.input_tokens = 1000;
        store.save_session_summary(&summary)?;
        let loaded = store.get_session_summary("sess-1")?.unwrap();
        assert_eq!(loaded.turns_total, 5);
        assert_eq!(loaded.input_tokens, 1000);
        Ok(())
    }

    #[test]
    fn session_summary_not_found() -> Result<()> {
        let db = test_db()?;
        assert!(db.metrics().get_session_summary("nope")?.is_none());
        Ok(())
    }

    #[test]
    fn list_session_summaries_ordered() -> Result<()> {
        let db = test_db()?;
        let store = db.metrics();
        for i in 0..5 {
            let mut s = SessionMetricsSummary::new(format!("s{i:02}"));
            s.turns_total = i as u32;
            store.save_session_summary(&s)?;
        }
        let listed = store.list_session_summaries(3)?;
        assert_eq!(listed.len(), 3);
        assert_eq!(listed[0].session_id, "s04");
        Ok(())
    }

    #[test]
    fn daily_rollup_roundtrip() -> Result<()> {
        let db = test_db()?;
        let store = db.metrics();
        let mut rollup = DailyMetricsRollup::new("2026-04-24".into());
        rollup.sessions = 3;
        rollup.input_tokens = 5000;
        store.save_daily_rollup(&rollup)?;
        let loaded = store.get_daily_rollup("2026-04-24")?.unwrap();
        assert_eq!(loaded.sessions, 3);
        assert_eq!(loaded.input_tokens, 5000);
        Ok(())
    }

    #[test]
    fn recent_events_append_and_read() -> Result<()> {
        let db = test_db()?;
        let store = db.metrics();
        for i in 0..3 {
            store.append_recent_event(&MetricEventRecord {
                session_id: "s1".into(),
                seq: i,
                timestamp: Utc::now(),
                kind: MetricEventKind::TurnStart { index: i },
            })?;
        }
        let events = store.recent_events_for_session("s1", 10)?;
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].seq, 0);
        Ok(())
    }

    #[test]
    fn recent_events_session_isolation() -> Result<()> {
        let db = test_db()?;
        let store = db.metrics();
        store.append_recent_event(&MetricEventRecord {
            session_id: "s1".into(),
            seq: 0,
            timestamp: Utc::now(),
            kind: MetricEventKind::SessionStart,
        })?;
        store.append_recent_event(&MetricEventRecord {
            session_id: "s2".into(),
            seq: 0,
            timestamp: Utc::now(),
            kind: MetricEventKind::SessionStart,
        })?;
        assert_eq!(store.recent_events_for_session("s1", 10)?.len(), 1);
        assert_eq!(store.recent_events_for_session("s2", 10)?.len(), 1);
        Ok(())
    }

    #[test]
    fn clear_all_empties_tables() -> Result<()> {
        let db = test_db()?;
        let store = db.metrics();
        store.save_session_summary(&SessionMetricsSummary::new("s1".into()))?;
        store.save_daily_rollup(&DailyMetricsRollup::new("2026-04-24".into()))?;
        store.append_recent_event(&MetricEventRecord {
            session_id: "s1".into(),
            seq: 0,
            timestamp: Utc::now(),
            kind: MetricEventKind::SessionStart,
        })?;
        store.clear_all()?;
        assert!(store.get_session_summary("s1")?.is_none());
        assert!(store.get_daily_rollup("2026-04-24")?.is_none());
        assert!(store.recent_events_for_session("s1", 10)?.is_empty());
        Ok(())
    }
}
