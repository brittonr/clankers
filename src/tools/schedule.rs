//! Schedule tool — create, list, pause, resume, and delete scheduled tasks.
//!
//! Wraps `clankers_scheduler::ScheduleEngine` as an agent tool. The LLM can
//! create schedules that fire prompts or commands at specified times.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Duration;
use chrono::Utc;
use clankers_scheduler::ScheduleEngine;
use clankers_scheduler::ScheduleId;
use clankers_scheduler::schedule::Schedule;
use serde_json::Value;
use serde_json::json;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;

pub struct ScheduleTool {
    definition: ToolDefinition,
    engine: Arc<ScheduleEngine>,
}

impl ScheduleTool {
    pub fn new(engine: Arc<ScheduleEngine>) -> Self {
        Self {
            definition: ToolDefinition {
                name: "schedule".to_string(),
                description: concat!(
                    "Create, list, pause, resume, or delete scheduled tasks. ",
                    "Schedules fire at specified times and produce events that ",
                    "the agent processes (run a prompt, execute a command, etc.).\n\n",
                    "Actions:\n",
                    "  create  — Create a new schedule (once, interval, or cron)\n",
                    "  list    — List all active schedules\n",
                    "  pause   — Pause a schedule by ID\n",
                    "  resume  — Resume a paused schedule\n",
                    "  delete  — Remove a schedule by ID\n",
                    "  info    — Get details about a specific schedule",
                )
                .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["create", "list", "pause", "resume", "delete", "info"],
                            "description": "Action to perform"
                        },
                        "name": {
                            "type": "string",
                            "description": "Schedule name (for create)"
                        },
                        "kind": {
                            "type": "string",
                            "enum": ["once", "interval", "cron"],
                            "description": "Schedule type (for create)"
                        },
                        "at": {
                            "type": "string",
                            "description": "ISO datetime or relative ('+30m', '+2h') for once schedules"
                        },
                        "interval": {
                            "type": "string",
                            "description": "Interval like '5m', '1h', '30s' for interval schedules"
                        },
                        "cron": {
                            "type": "string",
                            "description": "Cron pattern 'minute hour day_of_week' (e.g., '0 9 1-5')"
                        },
                        "payload": {
                            "type": "object",
                            "description": "Payload for the schedule (e.g., {\"prompt\": \"check status\"})"
                        },
                        "max_fires": {
                            "type": "integer",
                            "description": "Maximum times to fire before auto-expiring (optional)"
                        },
                        "id": {
                            "type": "string",
                            "description": "Schedule ID (for pause/resume/delete/info)"
                        }
                    },
                    "required": ["action"]
                }),
            },
            engine,
        }
    }

    fn handle_create(&self, params: &Value) -> ToolResult {
        let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("unnamed");
        let kind = params.get("kind").and_then(|v| v.as_str()).unwrap_or("interval");
        let payload = params.get("payload").cloned().unwrap_or_else(|| json!({}));

        let mut schedule = match kind {
            "once" => {
                let at_str = match params.get("at").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => return ToolResult::error("'once' schedule requires 'at' parameter"),
                };
                let at = match parse_datetime(at_str) {
                    Ok(dt) => dt,
                    Err(e) => return ToolResult::error(format!("invalid 'at': {e}")),
                };
                Schedule::once(name, at, payload)
            }
            "interval" => {
                let interval_str = match params.get("interval").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => return ToolResult::error("'interval' schedule requires 'interval' parameter"),
                };
                let secs = match parse_duration_secs(interval_str) {
                    Ok(s) => s,
                    Err(e) => return ToolResult::error(format!("invalid interval: {e}")),
                };
                Schedule::interval(name, secs, payload)
            }
            "cron" => {
                let pattern_str = match params.get("cron").and_then(|v| v.as_str()) {
                    Some(s) => s,
                    None => return ToolResult::error("'cron' schedule requires 'cron' parameter"),
                };
                let pattern = match clankers_scheduler::cron::CronPattern::parse(pattern_str) {
                    Ok(p) => p,
                    Err(e) => return ToolResult::error(format!("invalid cron pattern: {e}")),
                };
                Schedule::cron(name, pattern, payload)
            }
            other => return ToolResult::error(format!("unknown schedule kind: {other}")),
        };

        if let Some(max) = params.get("max_fires").and_then(|v| v.as_u64()) {
            schedule.max_fires = Some(max);
        }

        let id = self.engine.add(schedule);
        ToolResult::text(format!("Schedule created: {name} (id: {id})"))
    }

    fn handle_list(&self) -> ToolResult {
        let schedules = self.engine.list();
        if schedules.is_empty() {
            return ToolResult::text("No active schedules.");
        }

        let mut lines = Vec::new();
        for s in &schedules {
            let kind_str = match &s.kind {
                clankers_scheduler::ScheduleKind::Once { at } => {
                    format!("once at {}", at.format("%Y-%m-%d %H:%M:%S UTC"))
                }
                clankers_scheduler::ScheduleKind::Interval { interval_secs } => {
                    format!("every {}", format_duration(*interval_secs))
                }
                clankers_scheduler::ScheduleKind::Cron { .. } => "cron".to_string(),
            };
            lines.push(format!(
                "  {} | {} | {} | fired {} time(s) | {:?}",
                s.id, s.name, kind_str, s.fire_count, s.status,
            ));
        }
        ToolResult::text(format!("{} schedule(s):\n{}", schedules.len(), lines.join("\n")))
    }

    fn handle_action_by_id(&self, params: &Value, action: &str) -> ToolResult {
        let id_str = match params.get("id").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => return ToolResult::error(format!("'{action}' requires 'id' parameter")),
        };
        let id = ScheduleId(id_str.to_string());

        match action {
            "pause" => {
                if self.engine.pause(&id) {
                    ToolResult::text(format!("Schedule {id} paused."))
                } else {
                    ToolResult::error(format!("Schedule {id} not found or not active."))
                }
            }
            "resume" => {
                if self.engine.resume(&id) {
                    ToolResult::text(format!("Schedule {id} resumed."))
                } else {
                    ToolResult::error(format!("Schedule {id} not found or not paused."))
                }
            }
            "delete" => {
                if let Some(s) = self.engine.remove(&id) {
                    ToolResult::text(format!("Schedule '{}' ({id}) deleted.", s.name))
                } else {
                    ToolResult::error(format!("Schedule {id} not found."))
                }
            }
            "info" => {
                if let Some(s) = self.engine.get(&id) {
                    let json = serde_json::to_string_pretty(&s).unwrap_or_else(|_| "?".into());
                    ToolResult::text(json)
                } else {
                    ToolResult::error(format!("Schedule {id} not found."))
                }
            }
            _ => ToolResult::error(format!("unknown action: {action}")),
        }
    }
}

#[async_trait]
impl Tool for ScheduleTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, _ctx: &ToolContext, params: Value) -> ToolResult {
        let action = match params.get("action").and_then(|v| v.as_str()) {
            Some(a) => a.to_string(),
            None => return ToolResult::error("Missing 'action' parameter"),
        };

        match action.as_str() {
            "create" => self.handle_create(&params),
            "list" => self.handle_list(),
            "pause" | "resume" | "delete" | "info" => self.handle_action_by_id(&params, &action),
            other => ToolResult::error(format!("Unknown action: {other}")),
        }
    }
}

/// Parse a relative or ISO datetime string.
///
/// # Tiger Style
///
/// Uses `i64::try_from` instead of `as i64` to catch u64→i64 overflow.
/// The `parse_duration_secs` bounds check (365 days) guarantees this
/// won't overflow in practice, but the explicit check is defense-in-depth.
fn parse_datetime(s: &str) -> Result<chrono::DateTime<Utc>, String> {
    // Relative: "+30m", "+2h", "+1d"
    if let Some(rel) = s.strip_prefix('+') {
        let secs = parse_duration_secs(rel)?;
        let secs_i64 = i64::try_from(secs).map_err(|_| format!("duration too large for datetime: {secs}s"))?;
        return Ok(Utc::now() + Duration::seconds(secs_i64));
    }
    // Try ISO 8601
    s.parse::<chrono::DateTime<Utc>>().map_err(|e| format!("cannot parse datetime '{s}': {e}"))
}

/// Parse a duration string like "30s", "5m", "2h", "1d".
///
/// # Tiger Style
///
/// Uses `checked_mul` to prevent overflow on large numeric inputs.
/// Rejects durations exceeding 365 days (reasonable upper bound for schedules).
fn parse_duration_secs(s: &str) -> Result<u64, String> {
    /// Tiger Style: maximum schedule duration (365 days in seconds).
    const MAX_DURATION_SECS: u64 = 365 * 86_400;

    let s = s.trim();
    if s.is_empty() {
        return Err("empty duration".into());
    }
    let (num_part, unit) = if let Some(stripped) = s.strip_suffix('s') {
        (stripped, 1u64)
    } else if let Some(stripped) = s.strip_suffix('m') {
        (stripped, 60)
    } else if let Some(stripped) = s.strip_suffix('h') {
        (stripped, 3600)
    } else if let Some(stripped) = s.strip_suffix('d') {
        (stripped, 86400)
    } else {
        // Bare number = seconds
        (s, 1)
    };

    let n: u64 = num_part.parse().map_err(|_| format!("invalid number in duration: {s}"))?;

    // Tiger Style: checked arithmetic to prevent overflow.
    let total = n.checked_mul(unit).ok_or_else(|| format!("duration overflow: {s}"))?;

    if total > MAX_DURATION_SECS {
        return Err(format!("duration too large: {s} ({total}s > {MAX_DURATION_SECS}s max)"));
    }

    Ok(total)
}

/// Format seconds as a human-readable duration.
fn format_duration(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}d", secs / 86400)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_duration_units() {
        assert_eq!(parse_duration_secs("30s").unwrap(), 30);
        assert_eq!(parse_duration_secs("5m").unwrap(), 300);
        assert_eq!(parse_duration_secs("2h").unwrap(), 7200);
        assert_eq!(parse_duration_secs("1d").unwrap(), 86400);
        assert_eq!(parse_duration_secs("120").unwrap(), 120);
    }

    #[test]
    fn parse_duration_errors() {
        assert!(parse_duration_secs("").is_err());
        assert!(parse_duration_secs("abc").is_err());
    }

    // Tiger Style: overflow and bounds tests
    #[test]
    fn parse_duration_overflow_rejected() {
        // u64::MAX seconds would overflow when multiplied by 60
        assert!(parse_duration_secs("99999999999999999999m").is_err());
    }

    #[test]
    fn parse_duration_too_large_rejected() {
        // 366 days exceeds MAX_DURATION_SECS (365 days)
        assert!(parse_duration_secs("366d").is_err());
    }

    #[test]
    fn parse_duration_boundary_accepted() {
        // 365 days is exactly at the limit
        assert_eq!(parse_duration_secs("365d").unwrap(), 365 * 86_400);
    }

    #[test]
    fn parse_relative_datetime() {
        let dt = parse_datetime("+1h").unwrap();
        let diff = (dt - Utc::now()).num_seconds();
        assert!(diff >= 3590 && diff <= 3610);
    }

    #[test]
    fn format_duration_display() {
        assert_eq!(format_duration(30), "30s");
        assert_eq!(format_duration(300), "5m");
        assert_eq!(format_duration(7200), "2h");
        assert_eq!(format_duration(86400), "1d");
    }

    #[tokio::test]
    async fn create_and_list() {
        let engine = Arc::new(ScheduleEngine::new());
        let tool = ScheduleTool::new(engine);
        let ctx = ToolContext::new("test".into(), Default::default(), None);

        let result = tool
            .execute(
                &ctx,
                json!({
                    "action": "create",
                    "name": "test-schedule",
                    "kind": "interval",
                    "interval": "5m",
                    "payload": {"prompt": "check status"}
                }),
            )
            .await;
        assert!(!result.is_error);
        let text = match &result.content[0] {
            super::super::ToolResultContent::Text { text } => text,
            _ => panic!("expected text"),
        };
        assert!(text.contains("test-schedule"));

        let list_result = tool.execute(&ctx, json!({"action": "list"})).await;
        assert!(!list_result.is_error);
    }
}
