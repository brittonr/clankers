//! Durable process/job metadata store.
//!
//! This module stores bounded, safe metadata only. Large stdout/stderr bytes
//! belong in append-only log files or backend-native log references.

use std::collections::BTreeMap;
use std::path::PathBuf;

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

pub(crate) const TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("process_jobs");

pub const PROCESS_JOB_RECORD_SCHEMA_VERSION: u32 = 1;
const MAX_COMMAND_PREVIEW_LEN: usize = 200;
const REDACTED: &str = "[REDACTED]";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoredProcessJobBackendKind {
    Native,
    Pueue,
    Systemd,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum StoredProcessJobStatus {
    Pending,
    Running,
    Waiting,
    Succeeded { exit_code: Option<i32> },
    Failed { exit_code: Option<i32>, reason: String },
    Killed,
    Cancelled,
    LostAfterRestart,
    ReattachedLogIncomplete,
    BackendUnavailable { reason: String },
    Unknown { raw: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum StoredProcessJobOwnerScope {
    Session(String),
    Workspace(String),
    User(String),
    DaemonGlobal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "path")]
pub enum StoredProcessJobCwd {
    Inherited,
    Explicit(PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoredProcessJobStream {
    Stdout,
    Stderr,
    Combined,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredProcessJobLogRef {
    pub stream: StoredProcessJobStream,
    pub reference: String,
    pub retained_until: Option<DateTime<Utc>>,
    pub max_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredProcessJobResourcePolicy {
    pub timeout_seconds: Option<u64>,
    pub memory_max_bytes: Option<u64>,
    pub cpu_quota_percent: Option<u32>,
    pub max_log_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredProcessJobCapabilitySummary {
    pub can_observe: bool,
    pub can_read_logs: bool,
    pub can_start: bool,
    pub can_kill: bool,
    pub can_restart: bool,
    pub can_write_stdin: bool,
    pub can_select_backend: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredProcessJobRecord {
    pub schema_version: u32,
    pub id: String,
    pub backend: StoredProcessJobBackendKind,
    pub backend_ref: Option<String>,
    pub command_preview: String,
    pub cwd: StoredProcessJobCwd,
    pub owner: StoredProcessJobOwnerScope,
    pub status: StoredProcessJobStatus,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub os_pid: Option<u32>,
    pub process_group: Option<i32>,
    pub log_refs: Vec<StoredProcessJobLogRef>,
    pub resource_policy: StoredProcessJobResourcePolicy,
    pub capability_summary: StoredProcessJobCapabilitySummary,
    pub safe_metadata: BTreeMap<String, String>,
}

impl StoredProcessJobRecord {
    #[must_use]
    pub fn new_native(
        id: impl Into<String>,
        command_preview: impl Into<String>,
        owner: StoredProcessJobOwnerScope,
    ) -> Self {
        let now = Utc::now();
        Self {
            schema_version: PROCESS_JOB_RECORD_SCHEMA_VERSION,
            id: id.into(),
            backend: StoredProcessJobBackendKind::Native,
            backend_ref: None,
            command_preview: redact_command_preview(&command_preview.into()),
            cwd: StoredProcessJobCwd::Inherited,
            owner,
            status: StoredProcessJobStatus::Pending,
            started_at: now,
            updated_at: now,
            completed_at: None,
            os_pid: None,
            process_group: None,
            log_refs: Vec::new(),
            resource_policy: StoredProcessJobResourcePolicy {
                timeout_seconds: None,
                memory_max_bytes: None,
                cpu_quota_percent: None,
                max_log_bytes: None,
            },
            capability_summary: StoredProcessJobCapabilitySummary {
                can_observe: true,
                can_read_logs: true,
                can_start: true,
                can_kill: true,
                can_restart: false,
                can_write_stdin: true,
                can_select_backend: false,
            },
            safe_metadata: BTreeMap::new(),
        }
    }
}

pub struct ProcessJobStore<'db> {
    db: &'db Db,
}

#[derive(Clone)]
pub struct AsyncProcessJobStore {
    db: Db,
}

impl<'db> ProcessJobStore<'db> {
    pub(crate) fn new(db: &'db Db) -> Self {
        Self { db }
    }

    pub fn upsert(&self, record: &StoredProcessJobRecord) -> Result<()> {
        let mut safe = record.clone();
        safe.schema_version = PROCESS_JOB_RECORD_SCHEMA_VERSION;
        safe.command_preview = redact_command_preview(&safe.command_preview);
        safe.safe_metadata = redact_metadata(&safe.safe_metadata);

        let bytes = serde_json::to_vec(&safe).map_err(|e| crate::error::DbError {
            message: format!("failed to serialize process job record: {e}"),
        })?;
        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;
            table.insert(safe.id.as_str(), bytes.as_slice()).map_err(db_err)?;
        }
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<Option<StoredProcessJobRecord>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;
        match table.get(id).map_err(db_err)? {
            Some(value) => decode_supported_record(value.value()),
            None => Ok(None),
        }
    }

    pub fn list(&self) -> Result<Vec<StoredProcessJobRecord>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;
        let mut records = Vec::new();
        for item in table.iter().map_err(db_err)? {
            let (_key, value) = item.map_err(db_err)?;
            if let Some(record) = decode_supported_record(value.value())? {
                records.push(record);
            }
        }
        Ok(records)
    }

    pub fn count(&self) -> Result<u64> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;
        table.len().map_err(db_err)
    }
}

impl AsyncProcessJobStore {
    #[must_use]
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub async fn upsert(&self, record: StoredProcessJobRecord) -> Result<()> {
        self.db.blocking(move |db| db.process_jobs().upsert(&record)).await
    }

    pub async fn get(&self, id: impl Into<String>) -> Result<Option<StoredProcessJobRecord>> {
        let id = id.into();
        self.db.blocking(move |db| db.process_jobs().get(&id)).await
    }

    pub async fn list(&self) -> Result<Vec<StoredProcessJobRecord>> {
        self.db.blocking(|db| db.process_jobs().list()).await
    }

    pub async fn count(&self) -> Result<u64> {
        self.db.blocking(|db| db.process_jobs().count()).await
    }
}

fn decode_supported_record(bytes: &[u8]) -> Result<Option<StoredProcessJobRecord>> {
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| crate::error::DbError {
        message: format!("failed to deserialize process job record envelope: {e}"),
    })?;
    let Some(version) = value.get("schema_version").and_then(|v| v.as_u64()) else {
        return Ok(None);
    };
    if version > u64::from(PROCESS_JOB_RECORD_SCHEMA_VERSION) {
        return Ok(None);
    }
    let record = serde_json::from_value(value).map_err(|e| crate::error::DbError {
        message: format!("failed to deserialize process job record: {e}"),
    })?;
    Ok(Some(record))
}

#[must_use]
pub fn redact_command_preview(command: &str) -> String {
    let mut redacted = Vec::new();
    for token in command.split_whitespace() {
        let lower = token.to_ascii_lowercase();
        let should_redact = lower.contains("token=")
            || lower.contains("password=")
            || lower.contains("secret=")
            || lower.contains("authorization=")
            || lower.contains("api_key=")
            || lower.contains("apikey=")
            || lower.starts_with("bearer");
        if should_redact {
            redacted.push(REDACTED.to_string());
        } else if token.len() > MAX_COMMAND_PREVIEW_LEN {
            redacted.push(format!("{}…", &token[..MAX_COMMAND_PREVIEW_LEN]));
        } else {
            redacted.push(token.to_string());
        }
    }
    let joined = redacted.join(" ");
    if joined.len() > MAX_COMMAND_PREVIEW_LEN {
        format!("{}…", &joined[..MAX_COMMAND_PREVIEW_LEN])
    } else {
        joined
    }
}

#[must_use]
pub fn redact_metadata(metadata: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    metadata
        .iter()
        .map(|(key, value)| {
            let lower = key.to_ascii_lowercase();
            let value = if lower.contains("token")
                || lower.contains("secret")
                || lower.contains("password")
                || lower.contains("credential")
                || lower.contains("authorization")
            {
                REDACTED.to_string()
            } else {
                value.clone()
            };
            (key.clone(), value)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Result<Db> {
        Db::in_memory()
    }

    #[test]
    fn process_job_record_roundtrips_safe_metadata() -> Result<()> {
        let db = test_db()?;
        let store = db.process_jobs();
        let mut record = StoredProcessJobRecord::new_native(
            "proc_1",
            "curl -H authorization=secret token=abc https://example.invalid",
            StoredProcessJobOwnerScope::Session("sess".to_string()),
        );
        record.safe_metadata.insert("api_token".to_string(), "secret-value".to_string());
        record.safe_metadata.insert("profile".to_string(), "build".to_string());
        store.upsert(&record)?;

        let stored = store.get("proc_1")?.expect("record exists");
        assert_eq!(stored.id, "proc_1");
        assert_eq!(stored.safe_metadata.get("api_token").map(String::as_str), Some(REDACTED));
        assert_eq!(stored.safe_metadata.get("profile").map(String::as_str), Some("build"));
        assert!(!stored.command_preview.contains("secret"));
        assert!(!stored.command_preview.contains("token=abc"));
        Ok(())
    }

    #[test]
    fn future_record_versions_are_skipped() -> Result<()> {
        let db = test_db()?;
        let store = db.process_jobs();
        let mut record =
            StoredProcessJobRecord::new_native("proc_future", "sleep 1", StoredProcessJobOwnerScope::DaemonGlobal);
        record.schema_version = PROCESS_JOB_RECORD_SCHEMA_VERSION + 1;
        let bytes = serde_json::to_vec(&record).expect("record serializes");
        let tx = db.begin_write()?;
        {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;
            table.insert(record.id.as_str(), bytes.as_slice()).map_err(db_err)?;
        }
        tx.commit().map_err(db_err)?;

        assert!(store.get("proc_future")?.is_none());
        assert!(store.list()?.is_empty());
        assert_eq!(store.count()?, 1);
        Ok(())
    }

    #[tokio::test]
    async fn async_facade_uses_blocking_store_roundtrip() -> Result<()> {
        let db = test_db()?;
        let store = db.async_process_jobs();
        let mut record = StoredProcessJobRecord::new_native(
            "proc_async",
            "sleep 1",
            StoredProcessJobOwnerScope::Session("sess".to_string()),
        );
        record.backend_ref = Some("pid:123".to_string());
        store.upsert(record.clone()).await?;

        assert_eq!(store.count().await?, 1);
        assert_eq!(store.get("proc_async").await?, Some(record.clone()));
        assert_eq!(store.list().await?, vec![record]);
        Ok(())
    }
}
