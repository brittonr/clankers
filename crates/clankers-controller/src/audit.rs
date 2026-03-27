//! Audit tracking for tool call timing and leak detection.

use std::collections::HashMap;
use std::time::Instant;

use clankers_agent::events::AgentEvent;
use tracing::warn;

/// Maximum number of pending (unfinished) tool calls before warning.
const MAX_PENDING_CALLS: usize = 1024;

/// Tracks tool call starts and ends for timing and leak detection.
pub struct AuditTracker {
    /// call_id → (tool_name, start_time)
    pending: HashMap<String, (String, Instant)>,
    /// Total tool calls completed.
    completed: u64,
    /// Total tool call duration in milliseconds.
    total_duration_ms: u128,
}

impl AuditTracker {
    pub fn new() -> Self {
        Self {
            pending: HashMap::new(),
            completed: 0,
            total_duration_ms: 0,
        }
    }

    /// Process an agent event, tracking tool call starts and ends.
    pub fn process_event(&mut self, event: &AgentEvent) {
        match event {
            AgentEvent::ToolExecutionStart { call_id, tool_name } => {
                self.pending.insert(call_id.clone(), (tool_name.clone(), Instant::now()));
                if self.pending.len() > MAX_PENDING_CALLS {
                    warn!("audit: {} pending tool calls (possible leak)", self.pending.len());
                }
            }
            AgentEvent::ToolExecutionEnd { call_id, .. } => {
                if let Some((_, start)) = self.pending.remove(call_id) {
                    let elapsed = start.elapsed();
                    self.completed += 1;
                    self.total_duration_ms = self.total_duration_ms.saturating_add(elapsed.as_millis());
                }
            }
            _ => {}
        }
    }

    /// Number of tool calls currently in progress.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Total completed tool calls.
    pub fn completed_count(&self) -> u64 {
        self.completed
    }

    /// Average tool call duration in milliseconds (0 if no calls completed).
    #[cfg_attr(dylint_lib = "tigerstyle", allow(unchecked_division, reason = "divisor guarded by is_empty/non-zero check or TUI layout constraint"))]
    pub fn avg_duration_ms(&self) -> u64 {
        if self.completed == 0 {
            0
        } else {
            u64::try_from((self.total_duration_ms / self.completed as u128).min(u128::from(u64::MAX))).unwrap_or(u64::MAX)
        }
    }
}

impl Default for AuditTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use clankers_agent::ToolResult;

    use super::*;

    #[test]
    fn test_audit_basic() {
        let mut audit = AuditTracker::new();

        audit.process_event(&AgentEvent::ToolExecutionStart {
            call_id: "c1".to_string(),
            tool_name: "bash".to_string(),
        });

        assert_eq!(audit.pending_count(), 1);

        audit.process_event(&AgentEvent::ToolExecutionEnd {
            call_id: "c1".to_string(),
            result: ToolResult::text("ok"),
            is_error: false,
        });

        assert_eq!(audit.pending_count(), 0);
        assert_eq!(audit.completed_count(), 1);
    }

    #[test]
    fn test_audit_unknown_end() {
        let mut audit = AuditTracker::new();

        // Ending an unknown call_id should not panic
        audit.process_event(&AgentEvent::ToolExecutionEnd {
            call_id: "unknown".to_string(),
            result: ToolResult::text("ok"),
            is_error: false,
        });

        assert_eq!(audit.pending_count(), 0);
        assert_eq!(audit.completed_count(), 0);
    }
}
