//! Audit tracking for tool calls — records start/end times and results to redb.
//!
//! Extracted from the main event loop runner to isolate audit concern.

use std::collections::HashMap;

/// Tracks in-flight tool calls and writes completed audit entries to the database.
pub(crate) struct AuditTracker {
    /// In-flight tool calls: call_id → (tool_name, input, start_time)
    pending: HashMap<String, (String, serde_json::Value, std::time::Instant)>,
    /// Monotonic sequence number for ordering audit entries within a session.
    seq: u32,
}

impl AuditTracker {
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
            seq: 0,
        }
    }

    /// Record a tool call start. Returns the tool name for further processing.
    pub fn start_call(&mut self, call_id: &str, tool_name: &str, input: &serde_json::Value) {
        self.pending.insert(
            call_id.to_string(),
            (tool_name.to_string(), input.clone(), std::time::Instant::now()),
        );
    }

    /// Record a completed tool call. Writes the audit entry to the database
    /// in a background write task.
    pub fn end_call(
        &mut self,
        call_id: &str,
        result: &crate::tools::ToolResult,
        is_error: bool,
        session_id: &str,
        db: &crate::db::Db,
    ) {
        let (tool_name, input, started_at) = self
            .pending
            .remove(call_id)
            .unwrap_or_else(|| ("unknown".into(), serde_json::json!({}), std::time::Instant::now()));

        let duration_ms = started_at.elapsed().as_millis() as u64;

        let result_preview: String = result
            .content
            .iter()
            .filter_map(|c| match c {
                crate::tools::ToolResultContent::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
            .chars()
            .take(500)
            .collect();

        let sandbox_blocked = if is_error {
            result_preview.strip_prefix("🔒 ").map(|s| s.to_string())
        } else {
            None
        };

        let session_id = session_id.to_string();
        let call_id = call_id.to_string();
        let seq = self.seq;
        self.seq += 1;

        db.spawn_write(move |db| {
            let entry = crate::db::audit::AuditEntry {
                session_id,
                seq,
                tool: tool_name,
                call_id,
                input,
                is_error,
                result_preview,
                duration_ms,
                timestamp: chrono::Utc::now(),
                sandbox_blocked,
            };
            if let Err(e) = db.audit().record(&entry) {
                tracing::warn!("Failed to record audit entry: {}", e);
            }
        });
    }
}
