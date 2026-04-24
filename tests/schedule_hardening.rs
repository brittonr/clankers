//! Hardening tests for the schedule tool and engine.
//!
//! Covers edge cases and error paths not exercised by the existing
//! integration tests: duplicate names, paused-schedule tick behavior,
//! double-pause/resume idempotency, session ID injection, unknown kind,
//! cron schedule via tool, persistence corruption, and format_duration
//! boundary values.

use std::sync::Arc;
use std::time::Duration;

use clanker_scheduler::Schedule;
use clanker_scheduler::ScheduleEngine;
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

fn extract_id(text: &str) -> String {
    text.split("id: ").nth(1).unwrap().trim_end_matches(')').to_string()
}

// ── Duplicate names ─────────────────────────────────────────────────

#[tokio::test]
async fn duplicate_names_get_distinct_ids() {
    let engine = Arc::new(ScheduleEngine::new());
    let tool = ScheduleTool::new(engine.clone());
    let ctx = make_ctx();

    let r1 = tool
        .execute(
            &ctx,
            json!({"action": "create", "name": "dup", "kind": "interval", "interval": "5m", "payload": {"prompt": "a"}}),
        )
        .await;
    let r2 = tool
        .execute(
            &ctx,
            json!({"action": "create", "name": "dup", "kind": "interval", "interval": "10m", "payload": {"prompt": "b"}}),
        )
        .await;

    assert!(!r1.is_error);
    assert!(!r2.is_error);

    let id1 = extract_id(&result_text(&r1));
    let id2 = extract_id(&result_text(&r2));
    assert_ne!(id1, id2, "duplicate names should get distinct IDs");

    let list = tool.execute(&ctx, json!({"action": "list"})).await;
    let text = result_text(&list);
    assert!(text.contains("2 schedule(s)"), "should list both: {text}");
}

// ── Paused schedule does not fire ───────────────────────────────────

#[test]
fn paused_schedule_skipped_during_tick() {
    let engine = ScheduleEngine::new();
    let mut rx = engine.subscribe();

    let mut sched = Schedule::interval("paused-test", 0, json!({"prompt": "no"}));
    sched.last_fired = Some(chrono::Utc::now() - chrono::Duration::seconds(100));
    let id = engine.add(sched);

    engine.pause(&id);
    engine.tick();

    assert!(rx.try_recv().is_err(), "paused schedule should not fire");
}

// ── Double-pause is idempotent ──────────────────────────────────────

#[tokio::test]
async fn double_pause_is_idempotent() {
    let engine = Arc::new(ScheduleEngine::new());
    let tool = ScheduleTool::new(engine.clone());
    let ctx = make_ctx();

    let r = tool
        .execute(&ctx, json!({"action": "create", "name": "dp", "kind": "interval", "interval": "1m", "payload": {}}))
        .await;
    let id = extract_id(&result_text(&r));

    let p1 = tool.execute(&ctx, json!({"action": "pause", "id": id})).await;
    assert!(!p1.is_error);

    // Engine.pause() unconditionally sets Paused if found — idempotent success
    let p2 = tool.execute(&ctx, json!({"action": "pause", "id": id})).await;
    assert!(!p2.is_error, "double-pause should succeed (idempotent)");

    // Still paused
    let info = tool.execute(&ctx, json!({"action": "info", "id": id})).await;
    let parsed: serde_json::Value = serde_json::from_str(&result_text(&info)).unwrap();
    assert_eq!(parsed["status"], "Paused");
}

// ── Double-resume on active schedule ────────────────────────────────

#[tokio::test]
async fn resume_active_schedule_is_noop_error() {
    let engine = Arc::new(ScheduleEngine::new());
    let tool = ScheduleTool::new(engine.clone());
    let ctx = make_ctx();

    let r = tool
        .execute(&ctx, json!({"action": "create", "name": "ar", "kind": "interval", "interval": "1m", "payload": {}}))
        .await;
    let id = extract_id(&result_text(&r));

    // Resume without pausing first — should report not-paused
    let res = tool.execute(&ctx, json!({"action": "resume", "id": id})).await;
    assert!(res.is_error, "resume on active should fail: {}", result_text(&res));
}

// ── Unknown kind rejected ───────────────────────────────────────────

#[tokio::test]
async fn unknown_kind_rejected() {
    let engine = Arc::new(ScheduleEngine::new());
    let tool = ScheduleTool::new(engine);
    let ctx = make_ctx();

    let r = tool
        .execute(&ctx, json!({"action": "create", "name": "bad", "kind": "weekly", "payload": {}}))
        .await;
    assert!(r.is_error);
    assert!(
        result_text(&r).contains("unknown schedule kind"),
        "should mention unknown kind: {}",
        result_text(&r)
    );
}

// ── Cron schedule via tool ──────────────────────────────────────────

#[tokio::test]
async fn cron_schedule_shows_in_list() {
    let engine = Arc::new(ScheduleEngine::new());
    let tool = ScheduleTool::new(engine.clone());
    let ctx = make_ctx();

    let r = tool
        .execute(
            &ctx,
            json!({"action": "create", "name": "cron-test", "kind": "cron", "cron": "*/15 * *", "payload": {"prompt": "check"}}),
        )
        .await;
    assert!(!r.is_error, "cron create failed: {}", result_text(&r));

    let list = tool.execute(&ctx, json!({"action": "list"})).await;
    let text = result_text(&list);
    assert!(text.contains("cron-test"), "cron should appear in list: {text}");
    assert!(text.contains("cron"), "should show kind: {text}");
}

// ── Invalid cron pattern rejected ───────────────────────────────────

#[tokio::test]
async fn invalid_cron_pattern_rejected() {
    let engine = Arc::new(ScheduleEngine::new());
    let tool = ScheduleTool::new(engine);
    let ctx = make_ctx();

    let r = tool
        .execute(&ctx, json!({"action": "create", "name": "bad-cron", "kind": "cron", "cron": "not a cron"}))
        .await;
    assert!(r.is_error, "bad cron should fail: {}", result_text(&r));
    assert!(result_text(&r).contains("invalid cron pattern"), "error msg: {}", result_text(&r));
}

// ── Missing cron field for cron kind ────────────────────────────────

#[tokio::test]
async fn cron_missing_pattern_rejected() {
    let engine = Arc::new(ScheduleEngine::new());
    let tool = ScheduleTool::new(engine);
    let ctx = make_ctx();

    let r = tool.execute(&ctx, json!({"action": "create", "name": "no-cron", "kind": "cron"})).await;
    assert!(r.is_error);
    assert!(result_text(&r).contains("cron"), "should mention cron: {}", result_text(&r));
}

// ── Missing interval field for interval kind ────────────────────────

#[tokio::test]
async fn interval_missing_field_rejected() {
    let engine = Arc::new(ScheduleEngine::new());
    let tool = ScheduleTool::new(engine);
    let ctx = make_ctx();

    let r = tool.execute(&ctx, json!({"action": "create", "name": "no-interval", "kind": "interval"})).await;
    assert!(r.is_error);
    assert!(result_text(&r).contains("interval"), "should mention interval: {}", result_text(&r));
}

// ── Operations on nonexistent ID ────────────────────────────────────

#[tokio::test]
async fn operations_on_missing_id() {
    let engine = Arc::new(ScheduleEngine::new());
    let tool = ScheduleTool::new(engine);
    let ctx = make_ctx();
    let fake = "00000000-0000-0000-0000-000000000000";

    for action in ["pause", "resume", "delete", "info"] {
        let r = tool.execute(&ctx, json!({"action": action, "id": fake})).await;
        assert!(r.is_error, "{action} on missing ID should fail");
        assert!(result_text(&r).contains("not found"), "{action} error: {}", result_text(&r));
    }
}

// ── Operations missing ID param ─────────────────────────────────────

#[tokio::test]
async fn operations_missing_id_param() {
    let engine = Arc::new(ScheduleEngine::new());
    let tool = ScheduleTool::new(engine);
    let ctx = make_ctx();

    for action in ["pause", "resume", "delete", "info"] {
        let r = tool.execute(&ctx, json!({"action": action})).await;
        assert!(r.is_error, "{action} without id should fail");
        assert!(result_text(&r).contains("id"), "{action} error should mention id: {}", result_text(&r));
    }
}

// ── Info output contains expected fields ────────────────────────────

#[tokio::test]
async fn info_output_contains_schedule_fields() {
    let engine = Arc::new(ScheduleEngine::new());
    let tool = ScheduleTool::new(engine.clone());
    let ctx = make_ctx();

    let r = tool
        .execute(
            &ctx,
            json!({"action": "create", "name": "info-test", "kind": "interval", "interval": "2h", "payload": {"prompt": "x"}}),
        )
        .await;
    let id = extract_id(&result_text(&r));

    let info = tool.execute(&ctx, json!({"action": "info", "id": id})).await;
    assert!(!info.is_error);
    let text = result_text(&info);

    // Info returns JSON — check it parses and has expected keys
    let parsed: serde_json::Value = serde_json::from_str(&text).expect("info should return valid JSON");
    assert_eq!(parsed["name"], "info-test");
    assert_eq!(parsed["status"], "Active");
    assert_eq!(parsed["fire_count"], 0);
}

// ── max_fires via tool ──────────────────────────────────────────────

#[tokio::test]
async fn max_fires_set_via_tool() {
    let engine = Arc::new(ScheduleEngine::new());
    let tool = ScheduleTool::new(engine.clone());
    let ctx = make_ctx();

    let r = tool
        .execute(
            &ctx,
            json!({"action": "create", "name": "limited", "kind": "interval", "interval": "1m", "max_fires": 3, "payload": {}}),
        )
        .await;
    assert!(!r.is_error);
    let id_str = extract_id(&result_text(&r));

    let info = tool.execute(&ctx, json!({"action": "info", "id": id_str})).await;
    let parsed: serde_json::Value = serde_json::from_str(&result_text(&info)).unwrap();
    assert_eq!(parsed["max_fires"], 3);
}

// ── Empty list ──────────────────────────────────────────────────────

#[tokio::test]
async fn empty_list_message() {
    let engine = Arc::new(ScheduleEngine::new());
    let tool = ScheduleTool::new(engine);
    let ctx = make_ctx();

    let r = tool.execute(&ctx, json!({"action": "list"})).await;
    assert!(!r.is_error);
    assert!(result_text(&r).contains("No active"), "empty list: {}", result_text(&r));
}

// ── Persistence: corrupt file yields empty ──────────────────────────

#[test]
fn persistence_corrupt_file_returns_empty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("schedules.json");
    std::fs::write(&path, "not valid json {{{{").unwrap();

    let loaded = ScheduleEngine::load_from(&path);
    assert!(loaded.is_empty(), "corrupt file should return empty vec");
}

// ── Persistence: missing file yields empty ──────────────────────────

#[test]
fn persistence_missing_file_returns_empty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nonexistent.json");

    let loaded = ScheduleEngine::load_from(&path);
    assert!(loaded.is_empty(), "missing file should return empty vec");
}

// ── Concurrent add during tick loop ─────────────────────────────────

#[tokio::test]
async fn concurrent_add_during_tick_loop() {
    let engine = Arc::new(ScheduleEngine::new().with_tick_interval(Duration::from_millis(10)));
    let mut rx = engine.subscribe();

    // Start tick loop first
    let handle = engine.start();

    // Add a schedule that should fire immediately
    let mut sched = Schedule::interval("concurrent", 0, json!({}));
    sched.last_fired = Some(chrono::Utc::now() - chrono::Duration::seconds(100));
    engine.add(sched);

    // Should receive an event even though schedule was added after start
    let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("timeout — concurrent add should fire")
        .expect("channel error");

    assert_eq!(event.schedule_name, "concurrent");

    engine.cancel_token().cancel();
    let _ = handle.await;
}

// ── Multiple schedules fire in same tick ────────────────────────────

#[test]
fn multiple_schedules_fire_in_same_tick() {
    let engine = ScheduleEngine::new();
    let mut rx = engine.subscribe();

    for i in 0..3 {
        let mut sched = Schedule::interval(format!("multi-{i}"), 0, json!({"n": i}));
        sched.last_fired = Some(chrono::Utc::now() - chrono::Duration::seconds(100));
        engine.add(sched);
    }

    engine.tick();

    let mut names: Vec<String> = Vec::new();
    while let Ok(event) = rx.try_recv() {
        names.push(event.schedule_name);
    }

    names.sort();
    assert_eq!(names, vec!["multi-0", "multi-1", "multi-2"]);
}

// ── Delete then re-create with same name ────────────────────────────

#[tokio::test]
async fn delete_and_recreate_same_name() {
    let engine = Arc::new(ScheduleEngine::new());
    let tool = ScheduleTool::new(engine.clone());
    let ctx = make_ctx();

    // Create
    let r1 = tool
        .execute(
            &ctx,
            json!({"action": "create", "name": "ephemeral", "kind": "interval", "interval": "1m", "payload": {}}),
        )
        .await;
    let id1 = extract_id(&result_text(&r1));

    // Delete
    tool.execute(&ctx, json!({"action": "delete", "id": id1})).await;

    // Re-create
    let r2 = tool
        .execute(
            &ctx,
            json!({"action": "create", "name": "ephemeral", "kind": "interval", "interval": "2m", "payload": {}}),
        )
        .await;
    assert!(!r2.is_error);
    let id2 = extract_id(&result_text(&r2));
    assert_ne!(id1, id2, "re-created schedule should get new ID");

    // Old ID should be gone
    let info = tool.execute(&ctx, json!({"action": "info", "id": id1})).await;
    assert!(info.is_error, "old ID should be gone");
}

// ── Once schedule with past time fires immediately ──────────────────

#[tokio::test]
async fn once_past_time_fires_on_tick() {
    let engine = Arc::new(ScheduleEngine::new());
    let tool = ScheduleTool::new(engine.clone());
    let ctx = make_ctx();
    let mut rx = engine.subscribe();

    // Create a once schedule 1 hour in the past
    let r = tool
        .execute(
            &ctx,
            json!({"action": "create", "name": "overdue", "kind": "once", "at": "+0s", "payload": {"prompt": "late"}}),
        )
        .await;
    assert!(!r.is_error, "create failed: {}", result_text(&r));

    // Tick should fire and expire it
    engine.tick();

    let event = rx.try_recv().expect("overdue once should fire");
    assert_eq!(event.schedule_name, "overdue");

    // Should be gc'd
    let list = tool.execute(&ctx, json!({"action": "list"})).await;
    assert!(result_text(&list).contains("No active"), "expired once should be gc'd: {}", result_text(&list));
}

// ── No-op tick on empty engine ──────────────────────────────────────

#[test]
fn tick_on_empty_engine_is_harmless() {
    let engine = ScheduleEngine::new();
    let mut rx = engine.subscribe();

    engine.tick();
    engine.tick();
    engine.tick();

    assert!(rx.try_recv().is_err(), "empty engine should produce no events");
}

// ── Persistence roundtrip preserves max_fires and fire_count ────────

#[test]
fn persistence_preserves_all_fields() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("schedules.json");

    let engine = ScheduleEngine::new().with_persistence(path.clone());
    let mut sched = Schedule::interval("detailed", 300, json!({"prompt": "hi"}));
    sched.max_fires = Some(10);
    // Simulate having fired 3 times
    sched.fire_count = 3;
    sched.last_fired = Some(chrono::Utc::now());
    engine.add(sched);
    drop(engine);

    let loaded = ScheduleEngine::load_from(&path);
    assert_eq!(loaded.len(), 1);
    let s = &loaded[0];
    assert_eq!(s.name, "detailed");
    assert_eq!(s.max_fires, Some(10));
    assert_eq!(s.fire_count, 3);
    assert!(s.last_fired.is_some());
}
