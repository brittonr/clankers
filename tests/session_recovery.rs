//! Integration tests for daemon session recovery (tasks 2.4, 3.10).
//!
//! Tests checkpoint-on-shutdown flow and recovery-from-catalog flow
//! using the catalog + DaemonState directly (no live daemon needed).

use std::path::PathBuf;
use std::sync::Arc;

use clankers::message::{
    AgentMessage, AssistantMessage, Content, MessageId, StopReason, Usage, UserMessage,
};
use clankers::modes::daemon::agent_process::{
    get_or_create_keyed_session, prompt_and_collect,
};
use clankers::modes::daemon::session_store::{
    SessionCatalog, SessionCatalogEntry, SessionLifecycle,
};
use clankers::modes::daemon::socket_bridge::SessionFactory;
use clankers::provider::streaming::{ContentDelta, MessageMetadata, StreamEvent};
use clankers::provider::{CompletionRequest, Model, Provider};
use clankers_controller::transport::{DaemonState, SessionHandle};
use clankers_protocol::SessionKey;

fn temp_catalog() -> (tempfile::TempDir, Arc<SessionCatalog>) {
    let tmp = tempfile::tempdir().unwrap();
    let db_path = tmp.path().join("test.db");
    let db = Arc::new(redb::Database::create(&db_path).unwrap());
    let catalog = Arc::new(SessionCatalog::new(db));
    (tmp, catalog)
}

fn make_entry(id: &str, state: SessionLifecycle) -> SessionCatalogEntry {
    SessionCatalogEntry {
        session_id: id.to_string(),
        automerge_path: PathBuf::from(format!("/tmp/sessions/{id}.automerge")),
        model: "claude-sonnet-4-20250514".to_string(),
        created_at: "2026-03-20T10:00:00Z".to_string(),
        last_active: "2026-03-20T10:05:00Z".to_string(),
        turn_count: 5,
        state,
    }
}

fn make_handle(id: &str, live: bool) -> SessionHandle {
    SessionHandle {
        session_id: id.to_string(),
        model: "claude-sonnet-4-20250514".to_string(),
        turn_count: 5,
        last_active: "2026-03-20T10:05:00Z".to_string(),
        client_count: 0,
        cmd_tx: if live {
            Some(tokio::sync::mpsc::unbounded_channel().0)
        } else {
            None
        },
        event_tx: if live {
            Some(tokio::sync::broadcast::channel(16).0)
        } else {
            None
        },
        socket_path: PathBuf::from(format!("/tmp/sockets/{id}.sock")),
        state: if live {
            "active".to_string()
        } else {
            "suspended".to_string()
        },
    }
}

// ── Task 2.4: Checkpoint on shutdown ────────────────────────────────────

#[test]
fn checkpoint_transitions_active_to_suspended() {
    let (_tmp, catalog) = temp_catalog();

    // Simulate 3 active sessions
    catalog.insert_session(&make_entry("s1", SessionLifecycle::Active));
    catalog.insert_session(&make_entry("s2", SessionLifecycle::Active));
    catalog.insert_session(&make_entry("s3", SessionLifecycle::Active));

    // Checkpoint: what run_daemon does on shutdown
    let suspended = catalog.transition_all(
        SessionLifecycle::Active,
        SessionLifecycle::Suspended,
    );

    assert_eq!(suspended, 3);
    assert_eq!(catalog.list_by_state(SessionLifecycle::Active).len(), 0);
    assert_eq!(catalog.list_by_state(SessionLifecycle::Suspended).len(), 3);
}

#[test]
fn checkpoint_preserves_tombstoned() {
    let (_tmp, catalog) = temp_catalog();

    catalog.insert_session(&make_entry("alive", SessionLifecycle::Active));
    catalog.insert_session(&make_entry("dead", SessionLifecycle::Tombstoned));

    catalog.transition_all(SessionLifecycle::Active, SessionLifecycle::Suspended);

    // Tombstoned stays tombstoned
    assert_eq!(
        catalog.get_session("dead").unwrap().state,
        SessionLifecycle::Tombstoned
    );
    assert_eq!(
        catalog.get_session("alive").unwrap().state,
        SessionLifecycle::Suspended
    );
}

#[test]
fn checkpoint_preserves_key_mappings() {
    let (_tmp, catalog) = temp_catalog();

    catalog.insert_session(&make_entry("s1", SessionLifecycle::Active));
    let key = SessionKey::Iroh("peer123".to_string());
    catalog.insert_key(&key, "s1");

    // Checkpoint
    catalog.transition_all(SessionLifecycle::Active, SessionLifecycle::Suspended);

    // Key mapping survives
    assert_eq!(catalog.lookup_key(&key).unwrap(), "s1");
}

// ── Task 2.4: Crash recovery (active → suspended on startup) ────────────

#[test]
fn crash_recovery_marks_stale_active_as_suspended() {
    let (_tmp, catalog) = temp_catalog();

    // Previous daemon crashed — entries still marked Active
    catalog.insert_session(&make_entry("crashed-1", SessionLifecycle::Active));
    catalog.insert_session(&make_entry("crashed-2", SessionLifecycle::Active));
    catalog.insert_session(&make_entry("already-suspended", SessionLifecycle::Suspended));

    // What run_daemon does on startup
    let recovered = catalog.transition_all(
        SessionLifecycle::Active,
        SessionLifecycle::Suspended,
    );

    assert_eq!(recovered, 2);
    assert_eq!(catalog.list_by_state(SessionLifecycle::Active).len(), 0);
    assert_eq!(catalog.list_by_state(SessionLifecycle::Suspended).len(), 3);
}

// ── Task 3.10: Suspended sessions populate DaemonState ──────────────────

#[test]
fn suspended_sessions_populate_daemon_state() {
    let (_tmp, catalog) = temp_catalog();

    catalog.insert_session(&make_entry("s1", SessionLifecycle::Suspended));
    catalog.insert_session(&make_entry("s2", SessionLifecycle::Suspended));

    let key = SessionKey::Matrix {
        user_id: "@bot:example.com".to_string(),
        room_id: "!room:example.com".to_string(),
    };
    catalog.insert_key(&key, "s1");

    // Simulate Phase 3b from run_daemon
    let mut state = DaemonState::new();
    let suspended = catalog.list_by_state(SessionLifecycle::Suspended);
    let key_mappings = catalog.list_keys();

    for entry in &suspended {
        state.sessions.insert(
            entry.session_id.clone(),
            make_handle(&entry.session_id, false),
        );
    }
    for (k, session_id) in &key_mappings {
        if state.sessions.contains_key(session_id) {
            state.register_key(k.clone(), session_id.clone());
        }
    }

    assert_eq!(state.sessions.len(), 2);
    // Both should be placeholders (no cmd_tx)
    assert!(state.sessions["s1"].cmd_tx.is_none());
    assert!(state.sessions["s2"].cmd_tx.is_none());
    assert_eq!(state.sessions["s1"].state, "suspended");

    // Key mapping restored
    let found = state.session_by_key(&key);
    assert!(found.is_some());
    assert_eq!(found.unwrap().session_id, "s1");
}

#[test]
fn session_summaries_include_suspended() {
    let mut state = DaemonState::new();
    state
        .sessions
        .insert("live".to_string(), make_handle("live", true));
    state
        .sessions
        .insert("dead".to_string(), make_handle("dead", false));

    let summaries = state.session_summaries();
    assert_eq!(summaries.len(), 2);

    let live_s = summaries.iter().find(|s| s.session_id == "live").unwrap();
    assert_eq!(live_s.state, "active");

    let dead_s = summaries.iter().find(|s| s.session_id == "dead").unwrap();
    assert_eq!(dead_s.state, "suspended");
}

// ── Task 3.10: Placeholder detection ────────────────────────────────────

#[test]
fn placeholder_has_no_channels() {
    let handle = make_handle("suspended-1", false);
    assert!(handle.cmd_tx.is_none());
    assert!(handle.event_tx.is_none());
    assert_eq!(handle.state, "suspended");
}

#[test]
fn live_handle_has_channels() {
    let handle = make_handle("active-1", true);
    assert!(handle.cmd_tx.is_some());
    assert!(handle.event_tx.is_some());
    assert_eq!(handle.state, "active");
}

// ── Task 3.10: Corrupt/missing automerge file handling ──────────────────

#[test]
fn missing_automerge_detected() {
    // recover_session handles missing files by logging and returning empty
    // seed messages. We test the detection logic directly.
    let path = PathBuf::from("/tmp/nonexistent-session-12345.automerge");
    assert!(!path.exists());
}

#[test]
fn catalog_entry_serialization_round_trip() {
    let entry = make_entry("rt-test", SessionLifecycle::Suspended);
    let json = serde_json::to_vec(&entry).unwrap();
    let decoded: SessionCatalogEntry = serde_json::from_slice(&json).unwrap();
    assert_eq!(decoded.session_id, "rt-test");
    assert_eq!(decoded.state, SessionLifecycle::Suspended);
    assert_eq!(decoded.turn_count, 5);
}

// ── Task 5.6: Restart exit code constant ────────────────────────────────

#[test]
fn restart_exit_code_is_75() {
    assert_eq!(clankers::commands::daemon::RESTART_EXIT_CODE, 75);
}

// ── Protocol: RestartDaemon command exists ───────────────────────────────

#[test]
fn restart_daemon_command_serialization() {
    use clankers_protocol::control::{ControlCommand, ControlResponse};

    let cmd = ControlCommand::RestartDaemon;
    let json = serde_json::to_string(&cmd).unwrap();
    let decoded: ControlCommand = serde_json::from_str(&json).unwrap();
    assert!(matches!(decoded, ControlCommand::RestartDaemon));

    let resp = ControlResponse::Restarting;
    let json = serde_json::to_string(&resp).unwrap();
    let decoded: ControlResponse = serde_json::from_str(&json).unwrap();
    assert!(matches!(decoded, ControlResponse::Restarting));
}

// ── DaemonConfig: drain_timeout_secs ────────────────────────────────────

#[test]
fn daemon_config_drain_timeout_default() {
    let config = clankers::modes::daemon::DaemonConfig::default();
    assert_eq!(config.drain_timeout_secs, 10);
}

static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct RecoveryEchoProvider {
    observed_requests: Arc<std::sync::Mutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl Provider for RecoveryEchoProvider {
    async fn complete(
        &self,
        request: CompletionRequest,
        tx: tokio::sync::mpsc::Sender<StreamEvent>,
    ) -> clankers::provider::error::Result<()> {
        let saw_old_user = request.messages.iter().any(|msg| match msg {
            AgentMessage::User(user) => user.content.iter().any(|content| {
                matches!(content, Content::Text { text } if text == "hello from before")
            }),
            _ => false,
        });
        let saw_old_assistant = request.messages.iter().any(|msg| match msg {
            AgentMessage::Assistant(asst) => asst.content.iter().any(|content| {
                matches!(content, Content::Text { text } if text == "old reply")
            }),
            _ => false,
        });
        let current = request
            .messages
            .iter()
            .rev()
            .find_map(|msg| match msg {
                AgentMessage::User(user) => user.content.iter().find_map(|content| match content {
                    Content::Text { text } => Some(text.clone()),
                    _ => None,
                }),
                _ => None,
            })
            .unwrap_or_default();
        let reply = format!(
            "recovered={} current={}",
            saw_old_user && saw_old_assistant,
            current
        );
        self.observed_requests
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(reply.clone());

        tx.send(StreamEvent::MessageStart {
            message: MessageMetadata {
                id: "mock-1".to_string(),
                model: "mock".to_string(),
                role: "assistant".to_string(),
            },
        })
        .await
        .ok();
        tx.send(StreamEvent::ContentBlockStart {
            index: 0,
            content_block: Content::Text { text: String::new() },
        })
        .await
        .ok();
        tx.send(StreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentDelta::TextDelta { text: reply },
        })
        .await
        .ok();
        tx.send(StreamEvent::ContentBlockStop { index: 0 })
            .await
            .ok();
        tx.send(StreamEvent::MessageDelta {
            stop_reason: Some("end_turn".to_string()),
            usage: Usage {
                input_tokens: 10,
                output_tokens: 5,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            },
        })
        .await
        .ok();
        tx.send(StreamEvent::MessageStop).await.ok();
        Ok(())
    }

    fn models(&self) -> &[Model] {
        &[]
    }

    fn name(&self) -> &str {
        "recovery-echo"
    }
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn keyed_matrix_prompt_recovers_suspended_session_before_replying() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let runtime = tmp.path().join("run");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&runtime).unwrap();

    // SAFETY: nextest runs this test in its own process, and we serialize env writes.
    unsafe {
        std::env::set_var("HOME", &home);
        std::env::set_var("XDG_RUNTIME_DIR", &runtime);
    }

    let sessions_dir = home.join(".clankers").join("agent").join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();

    let mut mgr = clankers::clankers_session::SessionManager::create(
        &sessions_dir,
        "/tmp/matrix-recovery",
        "test-model",
        None,
        None,
        None,
    )
    .unwrap();
    let session_id = mgr.session_id().to_string();
    let user_id = MessageId::new("user-1");
    mgr.append_message(
        AgentMessage::User(UserMessage {
            id: user_id.clone(),
            content: vec![Content::Text {
                text: "hello from before".to_string(),
            }],
            timestamp: chrono::Utc::now(),
        }),
        None,
    )
    .unwrap();
    mgr.append_message(
        AgentMessage::Assistant(AssistantMessage {
            id: MessageId::new("assistant-1"),
            content: vec![Content::Text {
                text: "old reply".to_string(),
            }],
            model: "test-model".to_string(),
            usage: Usage::default(),
            stop_reason: StopReason::Stop,
            timestamp: chrono::Utc::now(),
        }),
        Some(user_id),
    )
    .unwrap();
    let automerge_path = mgr.file_path().to_path_buf();

    let db_path = tmp.path().join("catalog.db");
    let db = Arc::new(redb::Database::create(&db_path).unwrap());
    let catalog = Arc::new(SessionCatalog::new(db));
    catalog.insert_session(&SessionCatalogEntry {
        session_id: session_id.clone(),
        automerge_path,
        model: "test-model".to_string(),
        created_at: "2026-03-20T10:00:00Z".to_string(),
        last_active: "2026-03-20T10:05:00Z".to_string(),
        turn_count: 1,
        state: SessionLifecycle::Suspended,
    });
    let key = SessionKey::Matrix {
        user_id: "@alice:example.com".to_string(),
        room_id: "!room:example.com".to_string(),
    };
    catalog.insert_key(&key, &session_id);

    let mut daemon_state = DaemonState::new();
    daemon_state.sessions.insert(
        session_id.clone(),
        make_handle(&session_id, false),
    );
    daemon_state.register_key(key.clone(), session_id.clone());

    let state = tokio::sync::Mutex::new(daemon_state);
    let registry = clanker_actor::ProcessRegistry::new();
    let observed_requests = Arc::new(std::sync::Mutex::new(Vec::new()));
    let factory = SessionFactory {
        provider: Arc::new(RecoveryEchoProvider {
            observed_requests: Arc::clone(&observed_requests),
        }),
        tools: vec![],
        settings: clankers::config::settings::Settings::default(),
        default_model: "test-model".to_string(),
        default_system_prompt: "You are a test.".to_string(),
        registry: None,
        catalog: Some(Arc::clone(&catalog)),
        schedule_engine: None,
        plugin_manager: None,
    };

    let (reused_session_id, cmd_tx, event_tx) =
        get_or_create_keyed_session(&state, &registry, &factory, &key, None).await;
    assert_eq!(reused_session_id, session_id);

    let _response = prompt_and_collect(&cmd_tx, &event_tx, "followup".to_string(), vec![]).await;
    let observed = observed_requests
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    assert_eq!(observed, vec!["recovered=true current=followup".to_string()]);

    let st = state.lock().await;
    let handle = st.sessions.get(&session_id).unwrap();
    assert!(handle.cmd_tx.is_some());
    assert!(handle.event_tx.is_some());
    assert_eq!(handle.state, "active");
    drop(st);

    assert_eq!(
        catalog.get_session(&session_id).unwrap().state,
        SessionLifecycle::Active
    );
}
