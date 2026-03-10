use super::*;

#[test]
fn test_record_label() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    let id = MessageId::generate();
    let msg = AgentMessage::User(UserMessage {
        id: id.clone(),
        content: vec![Content::Text {
            text: "Message".to_string(),
        }],
        timestamp: Utc::now(),
    });
    mgr.append_message(msg, None).unwrap();

    // Record a label
    mgr.record_label("test-label").unwrap();

    // Re-open and verify the label was persisted
    let mgr2 = SessionManager::open(mgr.file_path().to_path_buf()).unwrap();
    let entries = store::read_entries(mgr2.file_path()).unwrap();
    let has_label = entries.iter().any(|e| {
        if let SessionEntry::Label(label) = e {
            label.label == "test-label" && label.target_message_id == id
        } else {
            false
        }
    });
    assert!(has_label);
}

#[test]
fn test_record_label_no_active_leaf() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    // Try to record a label without any messages
    let result = mgr.record_label("test");
    assert!(result.is_err());
}
