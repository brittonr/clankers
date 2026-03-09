use super::*;

#[test]
fn test_create_and_open_session() {
    let tmp = tempfile::TempDir::new().unwrap();
    let sessions_dir = tmp.path();
    let cwd = "/tmp/test";

    let mgr = SessionManager::create(sessions_dir, cwd, "claude-sonnet", None, None, None).unwrap();
    assert!(!mgr.session_id().is_empty());
    assert!(mgr.file_path().exists());

    // Should be able to open the session
    let mgr2 = SessionManager::open(mgr.file_path().to_path_buf()).unwrap();
    assert_eq!(mgr2.session_id(), mgr.session_id());
    assert_eq!(mgr2.cwd(), cwd);
}

#[test]
fn test_list_and_find_sessions() {
    let tmp = tempfile::TempDir::new().unwrap();
    let sessions_dir = tmp.path();
    let cwd = "/tmp/test";

    // Create two sessions
    let mgr1 = SessionManager::create(sessions_dir, cwd, "model-a", None, None, None).unwrap();
    let mgr2 = SessionManager::create(sessions_dir, cwd, "model-b", None, None, None).unwrap();

    let files = store::list_sessions(sessions_dir, cwd);
    assert_eq!(files.len(), 2);

    // Both session IDs should appear in the file list
    let all_names: String =
        files.iter().filter_map(|f| f.file_name().and_then(|n| n.to_str())).collect::<Vec<_>>().join(" ");
    assert!(all_names.contains(mgr1.session_id()));
    assert!(all_names.contains(mgr2.session_id()));
}

#[test]
fn test_duplicate_append_is_idempotent() {
    let tmp = tempfile::TempDir::new().unwrap();
    let sessions_dir = tmp.path();

    let mut mgr = SessionManager::create(sessions_dir, "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    let user_id = MessageId::generate();
    let user_msg = AgentMessage::User(UserMessage {
        id: user_id.clone(),
        content: vec![Content::Text {
            text: "Hello".to_string(),
        }],
        timestamp: Utc::now(),
    });

    // Append the same message twice
    mgr.append_message(user_msg.clone(), None).unwrap();
    mgr.append_message(user_msg, None).unwrap();

    // Should only have 1 message in the file
    let context = mgr.build_context().unwrap();
    assert_eq!(context.len(), 1);
}

#[test]
fn test_is_persisted() {
    let tmp = tempfile::TempDir::new().unwrap();
    let sessions_dir = tmp.path();

    let mut mgr = SessionManager::create(sessions_dir, "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    let user_id = MessageId::generate();
    assert!(!mgr.is_persisted(&user_id));

    let user_msg = AgentMessage::User(UserMessage {
        id: user_id.clone(),
        content: vec![Content::Text {
            text: "Hello".to_string(),
        }],
        timestamp: Utc::now(),
    });
    mgr.append_message(user_msg, None).unwrap();
    assert!(mgr.is_persisted(&user_id));
}

#[test]
fn test_open_tracks_existing_persisted_ids() {
    let tmp = tempfile::TempDir::new().unwrap();
    let sessions_dir = tmp.path();

    let mut mgr = SessionManager::create(sessions_dir, "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    let user_id = MessageId::generate();
    let user_msg = AgentMessage::User(UserMessage {
        id: user_id.clone(),
        content: vec![Content::Text {
            text: "Hello".to_string(),
        }],
        timestamp: Utc::now(),
    });
    mgr.append_message(user_msg, None).unwrap();

    // Re-open the session — it should know about the existing message
    let mgr2 = SessionManager::open(mgr.file_path().to_path_buf()).unwrap();
    assert!(mgr2.is_persisted(&user_id));
    assert_eq!(mgr2.message_count(), 1);
}

#[test]
fn test_model_accessor() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-opus", None, None, None).unwrap();
    assert_eq!(mgr.model(), "claude-opus");
}
