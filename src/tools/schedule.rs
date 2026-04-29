//! Schedule tool — create, list, pause, resume, and delete scheduled tasks.
//!
//! Wraps `clanker_scheduler::ScheduleEngine` as an agent tool. The LLM can
//! create schedules that fire prompts or commands at specified times.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::Duration;
use chrono::Utc;
use clanker_scheduler::ScheduleEngine;
use clanker_scheduler::ScheduleId;
use clanker_scheduler::schedule::Schedule;
use serde_json::Map;
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
                    "Create, list, update, pause, resume, run, or delete scheduled tasks. ",
                    "Schedules fire at specified times and produce events that ",
                    "the agent processes (run a prompt, execute a command, etc.). Also accepts ",
                    "Hermes cronjob-style prompts via `prompt`, `schedule`, `repeat`, `skills`, ",
                    "`model`, `script`, and `enabled_toolsets`.\n\n",
                    "Actions:\n",
                    "  create  — Create a new schedule (once, interval, or cron)\n",
                    "  list    — List all active schedules\n",
                    "  update  — Replace an existing schedule with a new definition (new id)\n",
                    "  pause   — Pause a schedule by ID\n",
                    "  resume  — Resume a paused schedule\n",
                    "  delete/remove — Remove a schedule by ID\n",
                    "  run     — Queue an existing schedule to run once as soon as the scheduler ticks\n",
                    "  info    — Get details about a specific schedule",
                )
                .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["create", "list", "update", "pause", "resume", "delete", "remove", "run", "info"],
                            "description": "Action to perform"
                        },
                        "schedule": {
                            "type": "string",
                            "description": "Hermes-style schedule string: '30m', 'every 2h', ISO datetime, or cron pattern"
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
                        "prompt": {
                            "type": "string",
                            "description": "Self-contained prompt to run when the schedule fires"
                        },
                        "repeat": {
                            "type": "integer",
                            "description": "Hermes alias for max_fires"
                        },
                        "skills": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Skill names to load/use when the scheduled prompt runs (stored in payload metadata)"
                        },
                        "model": {
                            "type": "object",
                            "description": "Model override metadata stored in payload"
                        },
                        "script": {
                            "type": "string",
                            "description": "Pre-run script path metadata stored in payload"
                        },
                        "enabled_toolsets": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Toolset restriction metadata stored in payload"
                        },
                        "max_fires": {
                            "type": "integer",
                            "description": "Maximum times to fire before auto-expiring (optional)"
                        },
                        "id": {
                            "type": "string",
                            "description": "Schedule ID (for pause/resume/delete/remove/run/update/info)"
                        },
                        "job_id": {
                            "type": "string",
                            "description": "Hermes cronjob-compatible alias for id"
                        }
                    },
                    "required": ["action"]
                }),
            },
            engine,
        }
    }

    fn handle_create(&self, params: &Value, session_id: &str) -> ToolResult {
        let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("unnamed");
        let payload = build_payload(params, session_id);

        let mut schedule = match build_schedule_from_params(name, params, payload) {
            Ok(schedule) => schedule,
            Err(err) => return ToolResult::error(err),
        };

        if let Some(max) = max_fires_from_params(params) {
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
                clanker_scheduler::ScheduleKind::Once { at } => {
                    format!("once at {}", at.format("%Y-%m-%d %H:%M:%S UTC"))
                }
                clanker_scheduler::ScheduleKind::Interval { interval_secs } => {
                    format!("every {}", format_duration(*interval_secs))
                }
                clanker_scheduler::ScheduleKind::Cron { .. } => "cron".to_string(),
            };
            lines.push(format!(
                "  {} | {} | {} | fired {} time(s) | {:?}",
                s.id, s.name, kind_str, s.fire_count, s.status,
            ));
        }
        ToolResult::text(format!("{} schedule(s):\n{}", schedules.len(), lines.join("\n")))
    }

    fn handle_action_by_id(&self, params: &Value, action: &str, session_id: &str) -> ToolResult {
        let id_str =
            match params.get("id").and_then(|v| v.as_str()).or_else(|| params.get("job_id").and_then(|v| v.as_str())) {
                Some(s) => s,
                None => return ToolResult::error(format!("'{action}' requires 'id' or 'job_id' parameter")),
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
            "delete" | "remove" => {
                if let Some(s) = self.engine.remove(&id) {
                    ToolResult::text(format!("Schedule '{}' ({id}) deleted.", s.name))
                } else {
                    ToolResult::error(format!("Schedule {id} not found."))
                }
            }
            "run" => {
                let Some(existing) = self.engine.get(&id) else {
                    return ToolResult::error(format!("Schedule {id} not found."));
                };
                let mut schedule =
                    Schedule::once(format!("{} (manual run)", existing.name), Utc::now(), existing.payload.clone());
                schedule.max_fires = Some(1);
                let new_id = self.engine.add(schedule);
                ToolResult::text(format!("Queued schedule '{}' to run once (id: {new_id}).", existing.name))
            }
            "update" => {
                let Some(old) = self.engine.remove(&id) else {
                    return ToolResult::error(format!("Schedule {id} not found."));
                };
                let name = params.get("name").and_then(|v| v.as_str()).unwrap_or(&old.name);
                let payload = build_payload_with_base(params, session_id, Some(old.payload.clone()));
                let mut schedule = match build_schedule_from_params(name, params, payload) {
                    Ok(schedule) => schedule,
                    Err(err) => {
                        self.engine.add(old);
                        return ToolResult::error(err);
                    }
                };
                if let Some(max) = max_fires_from_params(params) {
                    schedule.max_fires = Some(max);
                }
                let new_id = self.engine.add(schedule);
                ToolResult::text(format!("Schedule {id} updated as new id {new_id}."))
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

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        let action = match params.get("action").and_then(|v| v.as_str()) {
            Some(a) => a.to_string(),
            None => return ToolResult::error("Missing 'action' parameter"),
        };

        match action.as_str() {
            "create" => self.handle_create(&params, ctx.session_id()),
            "list" => self.handle_list(),
            "pause" | "resume" | "delete" | "remove" | "run" | "update" | "info" => {
                self.handle_action_by_id(&params, &action, ctx.session_id())
            }
            other => ToolResult::error(format!("Unknown action: {other}")),
        }
    }
}

fn build_payload(params: &Value, session_id: &str) -> Value {
    build_payload_with_base(params, session_id, None)
}

fn build_payload_with_base(params: &Value, session_id: &str, base: Option<Value>) -> Value {
    let mut payload = params.get("payload").cloned().or(base).unwrap_or_else(|| json!({}));

    if !payload.is_object() {
        payload = json!({ "payload": payload });
    }

    let Some(obj) = payload.as_object_mut() else {
        return payload;
    };

    insert_if_present(obj, params, "prompt");
    insert_if_present(obj, params, "skills");
    insert_if_present(obj, params, "model");
    insert_if_present(obj, params, "script");
    insert_if_present(obj, params, "enabled_toolsets");

    if !session_id.is_empty() {
        obj.insert("_session_id".to_string(), json!(session_id));
    }

    payload
}

fn insert_if_present(obj: &mut Map<String, Value>, params: &Value, key: &str) {
    if let Some(value) = params.get(key) {
        obj.insert(key.to_string(), value.clone());
    }
}

fn build_schedule_from_params(name: &str, params: &Value, payload: Value) -> Result<Schedule, String> {
    if let Some(schedule) = params.get("schedule").and_then(|v| v.as_str()) {
        return build_schedule_from_string(name, schedule, payload);
    }

    let kind = params.get("kind").and_then(|v| v.as_str()).unwrap_or("interval");
    match kind {
        "once" => {
            let at_str = params
                .get("at")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "'once' schedule requires 'at' parameter".to_string())?;
            let at = parse_datetime(at_str).map_err(|e| format!("invalid 'at': {e}"))?;
            Ok(Schedule::once(name, at, payload))
        }
        "interval" => {
            let interval_str = params
                .get("interval")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "'interval' schedule requires 'interval' parameter".to_string())?;
            let secs = parse_duration_secs(interval_str).map_err(|e| format!("invalid interval: {e}"))?;
            Ok(Schedule::interval(name, secs, payload))
        }
        "cron" => {
            let pattern_str = params
                .get("cron")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "'cron' schedule requires 'cron' parameter".to_string())?;
            let pattern = clanker_scheduler::cron::CronPattern::parse(pattern_str)
                .map_err(|e| format!("invalid cron pattern: {e}"))?;
            Ok(Schedule::cron(name, pattern, payload))
        }
        other => Err(format!("unknown schedule kind: {other}")),
    }
}

fn build_schedule_from_string(name: &str, schedule: &str, payload: Value) -> Result<Schedule, String> {
    let schedule = schedule.trim();
    if schedule.is_empty() {
        return Err("empty schedule".to_string());
    }

    if let Some(interval) = schedule.strip_prefix("every ") {
        let secs = parse_duration_secs(interval.trim()).map_err(|e| format!("invalid schedule interval: {e}"))?;
        return Ok(Schedule::interval(name, secs, payload));
    }

    if looks_like_duration(schedule) {
        let secs = parse_duration_secs(schedule).map_err(|e| format!("invalid schedule interval: {e}"))?;
        return Ok(Schedule::interval(name, secs, payload));
    }

    if schedule.contains(char::is_whitespace) {
        let pattern =
            clanker_scheduler::cron::CronPattern::parse(schedule).map_err(|e| format!("invalid cron schedule: {e}"))?;
        return Ok(Schedule::cron(name, pattern, payload));
    }

    let at = parse_datetime(schedule).map_err(|e| format!("invalid schedule datetime: {e}"))?;
    Ok(Schedule::once(name, at, payload))
}

fn looks_like_duration(s: &str) -> bool {
    let s = s.trim();
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit() || matches!(c, 's' | 'm' | 'h' | 'd'))
}

fn max_fires_from_params(params: &Value) -> Option<u64> {
    params
        .get("max_fires")
        .and_then(|v| v.as_u64())
        .or_else(|| params.get("repeat").and_then(|v| v.as_u64()))
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
    use tokio_util::sync::CancellationToken;

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
        assert!((3590..=3610).contains(&diff));
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
        let ctx = ToolContext::new("test".into(), CancellationToken::default(), None);

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

    #[tokio::test]
    async fn create_accepts_hermes_schedule_prompt_and_metadata() {
        let engine = Arc::new(ScheduleEngine::new());
        let tool = ScheduleTool::new(engine.clone());

        let result = tool.handle_create(
            &json!({
                "action": "create",
                "name": "cronjob-style",
                "schedule": "every 30m",
                "prompt": "summarize repo status",
                "repeat": 3,
                "skills": ["repo-state"],
                "model": {"provider": "anthropic", "model": "claude-sonnet"},
                "script": "scripts/preflight.rs",
                "enabled_toolsets": ["terminal", "file"]
            }),
            "session-123",
        );

        assert!(!result.is_error);
        let schedules = engine.list();
        assert_eq!(schedules.len(), 1);
        let schedule = &schedules[0];
        assert_eq!(schedule.name, "cronjob-style");
        assert_eq!(schedule.max_fires, Some(3));
        assert_eq!(schedule.payload["prompt"], "summarize repo status");
        assert_eq!(schedule.payload["_session_id"], "session-123");
        assert_eq!(schedule.payload["skills"], json!(["repo-state"]));
        assert_eq!(schedule.payload["enabled_toolsets"], json!(["terminal", "file"]));

        match &schedule.kind {
            clanker_scheduler::ScheduleKind::Interval { interval_secs } => assert_eq!(*interval_secs, 1800),
            other => panic!("expected interval schedule, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn job_id_alias_remove_and_run_work() {
        let engine = Arc::new(ScheduleEngine::new());
        let tool = ScheduleTool::new(engine.clone());
        let ctx = ToolContext::new("test".into(), CancellationToken::default(), None);

        let create = tool
            .execute(
                &ctx,
                json!({
                    "action": "create",
                    "name": "manual",
                    "schedule": "15m",
                    "prompt": "ping"
                }),
            )
            .await;
        assert!(!create.is_error);
        let original_id = engine.list()[0].id.clone();

        let run = tool.execute(&ctx, json!({"action": "run", "job_id": original_id.0})).await;
        assert!(!run.is_error);
        assert_eq!(engine.list().len(), 2);

        let remove = tool.execute(&ctx, json!({"action": "remove", "job_id": original_id.0})).await;
        assert!(!remove.is_error);
        assert_eq!(engine.list().len(), 1);
    }

    #[tokio::test]
    async fn update_preserves_existing_payload_when_prompt_omitted() {
        let engine = Arc::new(ScheduleEngine::new());
        let tool = ScheduleTool::new(engine.clone());
        let ctx = ToolContext::new("test".into(), CancellationToken::default(), None);

        let create = tool
            .execute(
                &ctx,
                json!({
                    "action": "create",
                    "name": "old",
                    "schedule": "10m",
                    "prompt": "keep me"
                }),
            )
            .await;
        assert!(!create.is_error);
        let old_id = engine.list()[0].id.clone();

        let update = tool
            .execute(
                &ctx,
                json!({
                    "action": "update",
                    "job_id": old_id.0,
                    "name": "new",
                    "schedule": "20m"
                }),
            )
            .await;
        assert!(!update.is_error);

        let schedules = engine.list();
        assert_eq!(schedules.len(), 1);
        assert_eq!(schedules[0].name, "new");
        assert_eq!(schedules[0].payload["prompt"], "keep me");
        match &schedules[0].kind {
            clanker_scheduler::ScheduleKind::Interval { interval_secs } => assert_eq!(*interval_secs, 1200),
            other => panic!("expected interval schedule, got {other:?}"),
        }
    }
}
