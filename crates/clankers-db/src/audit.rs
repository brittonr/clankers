//! Append-only audit log for tool invocations.
//!
//! Every tool call the agent makes is recorded with its parameters,
//! result status, and timing. This enables post-session review of
//! what the agent actually did, security auditing, and debugging.
//!
//! ## Storage
//!
//! Key: `"{session_id}:{sequence}"` (lexicographic order within a session).
//! Value: JSON-serialized `AuditEntry`.
//!
//! The sequence number is zero-padded to 6 digits so entries sort correctly
//! within a session (supports up to 999,999 tool calls per session).

use chrono::DateTime;
use chrono::Utc;
use redb::ReadableTable;
use redb::ReadableTableMetadata;
use redb::TableDefinition;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::fmt::Write;

use super::Db;
use crate::error::Result;
use crate::error::db_err;

/// Table: `"{session_id}:{seq:06}"` → serialized AuditEntry
pub(crate) const TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("audit_log");

/// A single tool invocation record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Session that triggered this call.
    pub session_id: String,
    /// Monotonic sequence within the session (0-based).
    pub seq: u32,
    /// Tool name (`bash`, `read`, `write`, `edit`, etc.).
    pub tool: String,
    /// Tool call ID from the LLM.
    pub call_id: String,
    /// Tool input parameters (the raw JSON the LLM sent).
    pub input: Value,
    /// Whether the tool returned an error.
    pub is_error: bool,
    /// First 500 chars of the result text (enough for review without bloat).
    pub result_preview: String,
    /// Wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// Timestamp of the call.
    pub timestamp: DateTime<Utc>,
    /// If the call was blocked by the sandbox, the reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox_blocked: Option<String>,
}

/// Accessor for the audit log.
pub struct AuditLog<'db> {
    db: &'db Db,
}

impl<'db> AuditLog<'db> {
    pub(crate) fn new(db: &'db Db) -> Self {
        Self { db }
    }

    /// Append an entry to the log.
    pub fn record(&self, entry: &AuditEntry) -> Result<()> {
        let key = format!("{}:{:06}", entry.session_id, entry.seq);
        let bytes = serde_json::to_vec(entry).map_err(|e| crate::error::DbError {
            message: format!("failed to serialize audit entry: {e}"),
        })?;

        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(TABLE).map_err(db_err)?;
            table.insert(key.as_str(), bytes.as_slice()).map_err(db_err)?;
        }
        tx.commit().map_err(db_err)?;
        Ok(())
    }

    /// Retrieve all entries for a session, in order.
    pub fn for_session(&self, session_id: &str) -> Result<Vec<AuditEntry>> {
        let prefix = format!("{session_id}:");
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;

        let mut entries = Vec::new();
        // Range scan: all keys starting with "{session_id}:"
        let start = prefix.as_str();
        // End range: increment the last char of the prefix to get an exclusive upper bound.
        // "session_id:" → "session_id;" (';' is ':' + 1 in ASCII)
        let end = format!("{session_id};");

        for item in table.range(start..end.as_str()).map_err(db_err)? {
            let (_key, value) = item.map_err(db_err)?;
            if let Ok(entry) = serde_json::from_slice::<AuditEntry>(value.value()) {
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    /// Get the next sequence number for a session.
    pub fn next_seq(&self, session_id: &str) -> Result<u32> {
        let entries = self.for_session(session_id)?;
        Ok(entries.last().map(|e| e.seq + 1).unwrap_or(0))
    }

    /// Count total entries across all sessions.
    pub fn count(&self) -> Result<u64> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;
        table.len().map_err(db_err)
    }

    /// Get the most recent N entries across all sessions (newest first).
    pub fn recent(&self, n: usize) -> Result<Vec<AuditEntry>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(TABLE).map_err(db_err)?;

        let mut entries = Vec::new();
        for item in table.iter().map_err(db_err)?.rev() {
            if entries.len() >= n {
                break;
            }
            let (_key, value) = item.map_err(db_err)?;
            if let Ok(entry) = serde_json::from_slice::<AuditEntry>(value.value()) {
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    /// Format a session's audit log as human-readable text.
    pub fn format_session(&self, session_id: &str) -> Result<String> {
        let entries = self.for_session(session_id)?;
        if entries.is_empty() {
            return Ok(format!("No audit entries for session {session_id}"));
        }

        let mut out = format!("## Audit log — {} ({} entries)\n\n", session_id, entries.len());

        for e in &entries {
            let status = if e.sandbox_blocked.is_some() {
                "🔒 BLOCKED"
            } else if e.is_error {
                "❌ ERROR"
            } else {
                "✅ OK"
            };

            let _ = writeln!(out, "{}. **{}** {} ({}ms)", e.seq + 1, e.tool, status, e.duration_ms);

            // Show key parameters
            match e.tool.as_str() {
                "bash" => {
                    if let Some(cmd) = e.input.get("command").and_then(|v| v.as_str()) {
                        let preview: String = cmd.chars().take(120).collect();
                        let _ = writeln!(out, "   `{preview}`");
                    }
                }
                "read" | "write" | "edit" => {
                    if let Some(path) = e.input.get("path").and_then(|v| v.as_str()) {
                        let _ = writeln!(out, "   `{path}`");
                    }
                }
                "grep" => {
                    if let Some(pattern) = e.input.get("pattern").and_then(|v| v.as_str()) {
                        let path = e.input.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                        let _ = writeln!(out, "   `{pattern}` in `{path}`");
                    }
                }
                _ => {}
            }

            if let Some(ref reason) = e.sandbox_blocked {
                let _ = writeln!(out, "   Blocked: {reason}");
            }

            out.push('\n');
        }

        // Summary
        let total = entries.len();
        let errors = entries.iter().filter(|e| e.is_error).count();
        let blocked = entries.iter().filter(|e| e.sandbox_blocked.is_some()).count();
        let total_ms: u64 = entries.iter().map(|e| e.duration_ms).sum();

        out.push_str("---\n");
        let _ = writeln!(out, "{total} calls, {errors} errors, {blocked} blocked, {total_ms}ms total");

        Ok(out)
    }

    /// Remove all audit entries (for testing).
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Result<Db> {
        Db::in_memory()
    }

    fn make_entry(session_id: &str, seq: u32, tool: &str) -> AuditEntry {
        AuditEntry {
            session_id: session_id.to_string(),
            seq,
            tool: tool.to_string(),
            call_id: format!("call_{seq}"),
            input: serde_json::json!({"path": "/tmp/test.rs"}),
            is_error: false,
            result_preview: "ok".to_string(),
            duration_ms: 42,
            timestamp: Utc::now(),
            sandbox_blocked: None,
        }
    }

    fn make_bash_entry(session_id: &str, seq: u32, command: &str) -> AuditEntry {
        AuditEntry {
            session_id: session_id.to_string(),
            seq,
            tool: "bash".to_string(),
            call_id: format!("call_{seq}"),
            input: serde_json::json!({"command": command}),
            is_error: false,
            result_preview: "".to_string(),
            duration_ms: 100,
            timestamp: Utc::now(),
            sandbox_blocked: None,
        }
    }

    // ── Basic CRUD ──────────────────────────────────────────────────

    #[test]
    fn record_and_retrieve() -> Result<()> {
        let db = test_db()?;
        let log = db.audit();

        let entry = make_entry("sess-1", 0, "read");
        log.record(&entry)?;

        let entries = log.for_session("sess-1")?;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].tool, "read");
        assert_eq!(entries[0].seq, 0);
        assert_eq!(entries[0].session_id, "sess-1");
        Ok(())
    }

    #[test]
    fn multiple_entries_ordered() -> Result<()> {
        let db = test_db()?;
        let log = db.audit();

        log.record(&make_entry("sess-1", 0, "read"))?;
        log.record(&make_entry("sess-1", 1, "edit"))?;
        log.record(&make_entry("sess-1", 2, "bash"))?;

        let entries = log.for_session("sess-1")?;
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].tool, "read");
        assert_eq!(entries[1].tool, "edit");
        assert_eq!(entries[2].tool, "bash");
        Ok(())
    }

    #[test]
    fn sessions_isolated() -> Result<()> {
        let db = test_db()?;
        let log = db.audit();

        log.record(&make_entry("sess-a", 0, "read"))?;
        log.record(&make_entry("sess-a", 1, "write"))?;
        log.record(&make_entry("sess-b", 0, "bash"))?;

        assert_eq!(log.for_session("sess-a")?.len(), 2);
        assert_eq!(log.for_session("sess-b")?.len(), 1);
        assert_eq!(log.for_session("sess-c")?.len(), 0);
        Ok(())
    }

    // ── Sequence tracking ───────────────────────────────────────────

    #[test]
    fn next_seq_empty() -> Result<()> {
        let db = test_db()?;
        assert_eq!(db.audit().next_seq("sess-1")?, 0);
        Ok(())
    }

    #[test]
    fn next_seq_after_entries() -> Result<()> {
        let db = test_db()?;
        let log = db.audit();

        log.record(&make_entry("sess-1", 0, "read"))?;
        log.record(&make_entry("sess-1", 1, "write"))?;

        assert_eq!(log.next_seq("sess-1")?, 2);
        Ok(())
    }

    // ── Count and recent ────────────────────────────────────────────

    #[test]
    fn count_all() -> Result<()> {
        let db = test_db()?;
        let log = db.audit();

        assert_eq!(log.count()?, 0);

        log.record(&make_entry("s1", 0, "read"))?;
        log.record(&make_entry("s2", 0, "write"))?;

        assert_eq!(log.count()?, 2);
        Ok(())
    }

    #[test]
    fn recent_entries() -> Result<()> {
        let db = test_db()?;
        let log = db.audit();

        log.record(&make_entry("s1", 0, "read"))?;
        log.record(&make_entry("s1", 1, "write"))?;
        log.record(&make_entry("s1", 2, "bash"))?;

        let recent = log.recent(2)?;
        assert_eq!(recent.len(), 2);
        // Newest first
        assert_eq!(recent[0].seq, 2);
        assert_eq!(recent[1].seq, 1);
        Ok(())
    }

    // ── Error and sandbox entries ───────────────────────────────────

    #[test]
    fn error_entry() -> Result<()> {
        let db = test_db()?;
        let log = db.audit();

        let mut entry = make_entry("s1", 0, "bash");
        entry.is_error = true;
        entry.result_preview = "command not found".to_string();
        log.record(&entry)?;

        let entries = log.for_session("s1")?;
        assert!(entries[0].is_error);
        Ok(())
    }

    #[test]
    fn sandbox_blocked_entry() -> Result<()> {
        let db = test_db()?;
        let log = db.audit();

        let mut entry = make_entry("s1", 0, "read");
        entry.sandbox_blocked = Some("blocked: inside ~/.ssh".to_string());
        entry.is_error = true;
        log.record(&entry)?;

        let entries = log.for_session("s1")?;
        assert!(entries[0].sandbox_blocked.is_some());
        assert!(entries[0].sandbox_blocked.as_ref().expect("sandbox_blocked should be present").contains(".ssh"));
        Ok(())
    }

    // ── Formatting ──────────────────────────────────────────────────

    #[test]
    fn format_empty_session() -> Result<()> {
        let db = test_db()?;
        let out = db.audit().format_session("nonexistent")?;
        assert!(out.contains("No audit entries"));
        Ok(())
    }

    #[test]
    fn format_session_with_entries() -> Result<()> {
        let db = test_db()?;
        let log = db.audit();

        log.record(&make_entry("s1", 0, "read"))?;
        log.record(&make_bash_entry("s1", 1, "cargo build"))?;

        let mut blocked = make_entry("s1", 2, "read");
        blocked.input = serde_json::json!({"path": "~/.ssh/id_rsa"});
        blocked.sandbox_blocked = Some("inside sensitive path".to_string());
        blocked.is_error = true;
        log.record(&blocked)?;

        let out = log.format_session("s1")?;
        assert!(out.contains("**read**"));
        assert!(out.contains("**bash**"));
        assert!(out.contains("cargo build"));
        assert!(out.contains("🔒 BLOCKED"));
        assert!(out.contains("3 calls"));
        assert!(out.contains("1 errors"));
        assert!(out.contains("1 blocked"));
        Ok(())
    }

    // ── Clear ───────────────────────────────────────────────────────

    #[test]
    fn clear_removes_all() -> Result<()> {
        let db = test_db()?;
        let log = db.audit();

        log.record(&make_entry("s1", 0, "read"))?;
        log.record(&make_entry("s2", 0, "write"))?;

        let cleared = log.clear()?;
        assert_eq!(cleared, 2);
        assert_eq!(log.count()?, 0);
        Ok(())
    }

    // ── Serialization roundtrip ─────────────────────────────────────

    #[test]
    fn entry_serialization_roundtrip() -> Result<()> {
        let entry = AuditEntry {
            session_id: "test".into(),
            seq: 42,
            tool: "bash".into(),
            call_id: "call_42".into(),
            input: serde_json::json!({"command": "ls -la"}),
            is_error: false,
            result_preview: "total 42\ndrwxr-xr-x ...".into(),
            duration_ms: 123,
            timestamp: Utc::now(),
            sandbox_blocked: None,
        };

        let json = serde_json::to_string(&entry).map_err(|e| crate::error::DbError {
            message: format!("serialization failed: {e}"),
        })?;
        let parsed: AuditEntry = serde_json::from_str(&json).map_err(|e| crate::error::DbError {
            message: format!("deserialization failed: {e}"),
        })?;

        assert_eq!(parsed.session_id, "test");
        assert_eq!(parsed.seq, 42);
        assert_eq!(parsed.tool, "bash");
        assert_eq!(parsed.duration_ms, 123);
        assert!(parsed.sandbox_blocked.is_none());
        Ok(())
    }

    #[test]
    fn entry_with_sandbox_blocked_serializes() -> Result<()> {
        let entry = AuditEntry {
            session_id: "test".into(),
            seq: 0,
            tool: "read".into(),
            call_id: "call_0".into(),
            input: serde_json::json!({"path": "~/.ssh/id_rsa"}),
            is_error: true,
            result_preview: "".into(),
            duration_ms: 0,
            timestamp: Utc::now(),
            sandbox_blocked: Some("sensitive path".into()),
        };

        let json = serde_json::to_string(&entry).map_err(|e| crate::error::DbError {
            message: format!("serialization failed: {e}"),
        })?;
        assert!(json.contains("sandbox_blocked"));
        assert!(json.contains("sensitive path"));
        Ok(())
    }

    #[test]
    fn entry_without_sandbox_blocked_omits_field() -> Result<()> {
        let entry = make_entry("s1", 0, "read");
        let json = serde_json::to_string(&entry).map_err(|e| crate::error::DbError {
            message: format!("serialization failed: {e}"),
        })?;
        assert!(!json.contains("sandbox_blocked"));
        Ok(())
    }

    // ── High sequence numbers ───────────────────────────────────────

    #[test]
    fn high_sequence_numbers_sort_correctly() -> Result<()> {
        let db = test_db()?;
        let log = db.audit();

        // Insert entries with gaps to test zero-padding sort
        for seq in [0, 9, 10, 99, 100, 999, 1000] {
            log.record(&make_entry("s1", seq, "bash"))?;
        }

        let entries = log.for_session("s1")?;
        assert_eq!(entries.len(), 7);
        // Verify monotonically increasing
        for pair in entries.windows(2) {
            assert!(pair[0].seq < pair[1].seq, "{} should be < {}", pair[0].seq, pair[1].seq);
        }
        Ok(())
    }

    // ── Recent spans multiple sessions ──────────────────────────────

    #[test]
    fn recent_spans_sessions() -> Result<()> {
        let db = test_db()?;
        let log = db.audit();

        log.record(&make_entry("s1", 0, "read"))?;
        log.record(&make_entry("s2", 0, "write"))?;
        log.record(&make_entry("s3", 0, "bash"))?;

        let recent = log.recent(10)?;
        assert_eq!(recent.len(), 3);
        // All three sessions present
        let sessions: Vec<&str> = recent.iter().map(|e| e.session_id.as_str()).collect();
        assert!(sessions.contains(&"s1"));
        assert!(sessions.contains(&"s2"));
        assert!(sessions.contains(&"s3"));
        Ok(())
    }

    #[test]
    fn recent_zero_returns_empty() -> Result<()> {
        let db = test_db()?;
        let log = db.audit();

        log.record(&make_entry("s1", 0, "read"))?;
        assert_eq!(log.recent(0)?.len(), 0);
        Ok(())
    }

    // ── Clear then re-use ───────────────────────────────────────────

    #[test]
    fn clear_then_reuse() -> Result<()> {
        let db = test_db()?;
        let log = db.audit();

        log.record(&make_entry("s1", 0, "read"))?;
        log.clear()?;

        // Can still write after clear
        log.record(&make_entry("s1", 0, "write"))?;
        let entries = log.for_session("s1")?;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].tool, "write");
        Ok(())
    }

    // ── next_seq independent per session ────────────────────────────

    #[test]
    fn next_seq_per_session() -> Result<()> {
        let db = test_db()?;
        let log = db.audit();

        log.record(&make_entry("s1", 0, "read"))?;
        log.record(&make_entry("s1", 1, "write"))?;
        log.record(&make_entry("s2", 0, "bash"))?;

        assert_eq!(log.next_seq("s1")?, 2);
        assert_eq!(log.next_seq("s2")?, 1);
        assert_eq!(log.next_seq("s3")?, 0);
        Ok(())
    }

    // ── Overwrite same key ──────────────────────────────────────────

    #[test]
    fn overwrite_same_seq() -> Result<()> {
        let db = test_db()?;
        let log = db.audit();

        log.record(&make_entry("s1", 0, "read"))?;
        log.record(&make_entry("s1", 0, "write"))?; // same seq

        let entries = log.for_session("s1")?;
        // redb INSERT replaces, so should have 1 entry with the latest tool
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].tool, "write");
        Ok(())
    }

    // ── Format: grep tool rendering ─────────────────────────────────

    #[test]
    fn format_grep_entry() -> Result<()> {
        let db = test_db()?;
        let log = db.audit();

        let mut entry = make_entry("s1", 0, "grep");
        entry.input = serde_json::json!({"pattern": "TODO", "path": "src/"});
        log.record(&entry)?;

        let out = log.format_session("s1")?;
        assert!(out.contains("TODO"));
        assert!(out.contains("src/"));
        Ok(())
    }

    // ── Format: duration totals ─────────────────────────────────────

    #[test]
    fn format_duration_totals() -> Result<()> {
        let db = test_db()?;
        let log = db.audit();

        let mut e1 = make_entry("s1", 0, "read");
        e1.duration_ms = 100;
        let mut e2 = make_entry("s1", 1, "write");
        e2.duration_ms = 200;
        log.record(&e1)?;
        log.record(&e2)?;

        let out = log.format_session("s1")?;
        assert!(out.contains("300ms total"));
        Ok(())
    }

    // ── Input with special characters ───────────────────────────────

    #[test]
    fn unicode_in_fields() -> Result<()> {
        let db = test_db()?;
        let log = db.audit();

        let mut entry = make_entry("sess-日本語", 0, "bash");
        entry.input = serde_json::json!({"command": "echo '🚀 héllo'"});
        entry.result_preview = "🚀 héllo".to_string();
        log.record(&entry)?;

        let entries = log.for_session("sess-日本語")?;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].result_preview, "🚀 héllo");
        Ok(())
    }

    // ── Empty result preview ────────────────────────────────────────

    #[test]
    fn empty_result_preview() -> Result<()> {
        let db = test_db()?;
        let log = db.audit();

        let mut entry = make_entry("s1", 0, "write");
        entry.result_preview = String::new();
        log.record(&entry)?;

        let entries = log.for_session("s1")?;
        assert_eq!(entries[0].result_preview, "");
        Ok(())
    }

    // ── Large input JSON ────────────────────────────────────────────

    #[test]
    fn large_input_json() -> Result<()> {
        let db = test_db()?;
        let log = db.audit();

        let big_content: String = "x".repeat(10_000);
        let mut entry = make_entry("s1", 0, "write");
        entry.input = serde_json::json!({"path": "/tmp/big.txt", "content": big_content});
        log.record(&entry)?;

        let entries = log.for_session("s1")?;
        assert_eq!(entries.len(), 1);
        let content = entries[0].input["content"].as_str().expect("content field should be a string");
        assert_eq!(content.len(), 10_000);
        Ok(())
    }
}
