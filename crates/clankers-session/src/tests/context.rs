use super::*;

#[test]
fn test_append_and_build_context() {
    let tmp = tempfile::TempDir::new().unwrap();
    let sessions_dir = tmp.path();

    let mut mgr = SessionManager::create(sessions_dir, "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    // Append a user message
    let user_id = MessageId::generate();
    let user_msg = AgentMessage::User(UserMessage {
        id: user_id.clone(),
        content: vec![Content::Text {
            text: "Hello".to_string(),
        }],
        timestamp: Utc::now(),
    });
    mgr.append_message(user_msg, None).unwrap();

    // Append an assistant message
    let asst_id = MessageId::generate();
    let asst_msg = AgentMessage::Assistant(AssistantMessage {
        id: asst_id.clone(),
        content: vec![Content::Text {
            text: "Hi there!".to_string(),
        }],
        model: "claude-sonnet".to_string(),
        usage: Usage::default(),
        stop_reason: StopReason::Stop,
        timestamp: Utc::now(),
    });
    mgr.append_message(asst_msg, Some(user_id.clone())).unwrap();

    // Build context should return both messages
    let context = mgr.build_context().unwrap();
    assert_eq!(context.len(), 2);
    assert!(context[0].is_user());
    assert!(context[1].is_assistant());
}

#[test]
fn test_session_resume_with_resume_entry() {
    let tmp = tempfile::TempDir::new().unwrap();
    let sessions_dir = tmp.path();

    let mut mgr = SessionManager::create(sessions_dir, "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    // Add a message
    let user_id = MessageId::generate();
    let user_msg = AgentMessage::User(UserMessage {
        id: user_id.clone(),
        content: vec![Content::Text {
            text: "First session message".to_string(),
        }],
        timestamp: Utc::now(),
    });
    mgr.append_message(user_msg, None).unwrap();

    // Record a resume via the new API
    mgr.record_resume(MessageId::new("resume")).unwrap();

    // Re-open and verify context still works
    let mgr2 = SessionManager::open(mgr.file_path().to_path_buf()).unwrap();
    let context = mgr2.build_context().unwrap();
    assert_eq!(context.len(), 1);
}

#[test]
fn test_session_persists_latest_compaction_summary() {
    let tmp = tempfile::TempDir::new().unwrap();
    let sessions_dir = tmp.path();

    let mut mgr = SessionManager::create(sessions_dir, "/tmp/test", "claude-sonnet", None, None, None).unwrap();
    assert!(mgr.latest_compaction_summary().is_none());

    mgr.record_compaction_summary("## Active Task\n- continue".to_string()).unwrap();
    assert_eq!(mgr.latest_compaction_summary(), Some("## Active Task\n- continue"));

    let reopened = SessionManager::open(mgr.file_path().to_path_buf()).unwrap();
    assert_eq!(reopened.latest_compaction_summary(), Some("## Active Task\n- continue"));
}
