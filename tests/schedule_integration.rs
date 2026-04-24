//! Integration tests for the schedule tool and engine.
//!
//! Covers: tool CRUD, fire→event flow, max_fires expiry,
//! no-prompt-field handling, and persistence roundtrip.

use std::sync::Arc;
use std::time::Duration;

use clanker_scheduler::Schedule;
use clanker_scheduler::ScheduleEngine;
use clanker_scheduler::ScheduleStatus;
use clankers::agent::tool::Tool;
use clankers::agent::tool::ToolContext;
use clankers::tools::schedule::ScheduleTool;
use serde_json::json;
use tokio_util::sync::CancellationToken;

fn make_ctx() -> ToolContext {
    ToolContext::new("test-call".into(), CancellationToken::default(), None)
}

fn result_text(result: &clankers::agent::tool::ToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|c| match c {
            clankers::agent::tool::ToolResultContent::Text { text } => Some(text.clone()),
            _ => None,
        })
        .collect::<String>()
}

// ── 4.1 Tool CRUD ──────────────────────────────────────────────────

#[tokio::test]
async fn tool_create_interval() {
    let engine = Arc::new(ScheduleEngine::new());
    let tool = ScheduleTool::new(engine.clone());
    let ctx = make_ctx();

    let r = tool
        .execute(
            &ctx,
            json!({"action": "create", "name": "poll", "kind": "interval", "interval": "5m", "payload": {"prompt": "check"}}),
        )
        .await;
    assert!(!r.is_error, "create failed: {}", result_text(&r));
    assert!(result_text(&r).contains("poll"));

    // Verify it shows up in list.
    let list = tool.execute(&ctx, json!({"action": "list"})).await;
    assert!(result_text(&list).contains("poll"));
}

#[tokio::test]
async fn tool_create_once_relative() {
    let engine = Arc::new(ScheduleEngine::new());
    let tool = ScheduleTool::new(engine.clone());
    let ctx = make_ctx();

    let r = tool
        .execute(&ctx, json!({"action": "create", "name": "reminder", "kind": "once", "at": "+1h"}))
        .await;
    assert!(!r.is_error, "create once failed: {}", result_text(&r));
    assert!(result_text(&r).contains("reminder"));
}

#[tokio::test]
async fn tool_create_cron() {
    let engine = Arc::new(ScheduleEngine::new());
    let tool = ScheduleTool::new(engine.clone());
    let ctx = make_ctx();

    let r = tool
        .execute(
            &ctx,
            json!({"action": "create", "name": "standup", "kind": "cron", "cron": "0 9 1-5", "payload": {"prompt": "standup"}}),
        )
        .await;
    assert!(!r.is_error, "create cron failed: {}", result_text(&r));
    assert!(result_text(&r).contains("standup"));
}

#[tokio::test]
async fn tool_pause_resume_delete_info() {
    let engine = Arc::new(ScheduleEngine::new());
    let tool = ScheduleTool::new(engine.clone());
    let ctx = make_ctx();

    // Create.
    let r = tool
        .execute(
            &ctx,
            json!({"action": "create", "name": "test-sched", "kind": "interval", "interval": "10m", "payload": {"prompt": "x"}}),
        )
        .await;
    let text = result_text(&r);
    let id = text.split("id: ").nth(1).unwrap().trim_end_matches(')').to_string();

    // Pause.
    let r = tool.execute(&ctx, json!({"action": "pause", "id": id})).await;
    assert!(!r.is_error);
    let info = tool.execute(&ctx, json!({"action": "info", "id": id})).await;
    assert!(result_text(&info).contains("Paused"));

    // Resume.
    let r = tool.execute(&ctx, json!({"action": "resume", "id": id})).await;
    assert!(!r.is_error);
    let info = tool.execute(&ctx, json!({"action": "info", "id": id})).await;
    assert!(result_text(&info).contains("Active"));

    // Delete.
    let r = tool.execute(&ctx, json!({"action": "delete", "id": id})).await;
    assert!(!r.is_error);
    let list = tool.execute(&ctx, json!({"action": "list"})).await;
    assert!(result_text(&list).contains("No active"));

    // Info on deleted should error.
    let r = tool.execute(&ctx, json!({"action": "info", "id": id})).await;
    assert!(r.is_error);
}

#[tokio::test]
async fn tool_invalid_action() {
    let engine = Arc::new(ScheduleEngine::new());
    let tool = ScheduleTool::new(engine);
    let ctx = make_ctx();

    let r = tool.execute(&ctx, json!({"action": "invalid"})).await;
    assert!(r.is_error);
}

#[tokio::test]
async fn tool_missing_required_params() {
    let engine = Arc::new(ScheduleEngine::new());
    let tool = ScheduleTool::new(engine);
    let ctx = make_ctx();

    // "once" without "at"
    let r = tool.execute(&ctx, json!({"action": "create", "kind": "once", "name": "bad"})).await;
    assert!(r.is_error);
    assert!(result_text(&r).contains("at"));
}

// ── 4.2 Fire-to-event ──────────────────────────────────────────────

#[tokio::test]
async fn fire_produces_schedule_event() {
    let engine = ScheduleEngine::new().with_tick_interval(Duration::from_millis(10));
    let mut rx = engine.subscribe();

    let mut sched = Schedule::interval("fire-test", 0, json!({"prompt": "check status"}));
    sched.last_fired = Some(chrono::Utc::now() - chrono::Duration::seconds(100));
    engine.add(sched);

    let handle = engine.start();

    let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("timeout waiting for event")
        .expect("channel error");

    assert_eq!(event.schedule_name, "fire-test");
    assert_eq!(event.payload["prompt"], "check status");
    assert_eq!(event.fire_count, 1);

    engine.cancel_token().cancel();
    let _ = handle.await;
}

// ── 4.3 No-prompt-field ────────────────────────────────────────────

#[test]
fn no_prompt_field_is_empty_string() {
    // The drain_schedule_events consumer checks payload["prompt"].
    // Verify that a payload without "prompt" yields an empty string,
    // which the consumer skips.
    let payload = json!({"command": "ls"});
    let prompt = payload.get("prompt").and_then(|v| v.as_str()).unwrap_or_default();
    assert!(prompt.is_empty());
}

// ── 4.4 Max-fires expiry ──────────────────────────────────────────

#[test]
fn max_fires_causes_expiry() {
    let engine = ScheduleEngine::new();
    let mut rx = engine.subscribe();

    let mut sched = Schedule::interval("limited", 0, json!({"prompt": "go"}));
    sched.max_fires = Some(2);
    sched.last_fired = Some(chrono::Utc::now() - chrono::Duration::seconds(100));
    let id = engine.add(sched);

    // First tick — fires (count=1).
    engine.tick();
    let e1 = rx.try_recv().expect("first fire");
    assert_eq!(e1.fire_count, 1);

    // Second tick — fires (count=2), then expires.
    engine.tick();
    let e2 = rx.try_recv().expect("second fire");
    assert_eq!(e2.fire_count, 2);

    // Schedule should be expired and GC'd.
    assert!(engine.get(&id).is_none(), "expired schedule should be GC'd");

    // Third tick — nothing fires.
    engine.tick();
    assert!(rx.try_recv().is_err(), "should not fire after expiry");
}

// ── 4.5 Persistence roundtrip ──────────────────────────────────────

#[test]
fn persistence_roundtrip_via_engine() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("schedules.json");

    // Engine 1: create schedules.
    let engine1 = ScheduleEngine::new().with_persistence(path.clone());
    let id_a = engine1.add(Schedule::interval("alpha", 60, json!({"prompt": "a"})));
    let id_b = engine1.add(Schedule::interval("beta", 120, json!({"prompt": "b"})));
    engine1.pause(&id_b);
    drop(engine1);

    // Engine 2: load from same path.
    let loaded = ScheduleEngine::load_from(&path);
    assert_eq!(loaded.len(), 2);

    let engine2 = ScheduleEngine::new();
    engine2.add_all(loaded);

    let a = engine2.get(&id_a).unwrap();
    assert_eq!(a.name, "alpha");
    assert_eq!(a.status, ScheduleStatus::Active);

    let b = engine2.get(&id_b).unwrap();
    assert_eq!(b.name, "beta");
    assert_eq!(b.status, ScheduleStatus::Paused);
}

#[test]
fn persistence_expired_not_reloaded() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("schedules.json");

    let engine = ScheduleEngine::new().with_persistence(path.clone());
    // One-shot in the past — fires + expires on tick.
    let target = chrono::Utc::now() - chrono::Duration::seconds(1);
    engine.add(Schedule::once("ephemeral", target, json!({})));
    engine.add(Schedule::interval("survivor", 60, json!({})));
    engine.tick();
    drop(engine);

    let loaded = ScheduleEngine::load_from(&path);
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].name, "survivor");
}
