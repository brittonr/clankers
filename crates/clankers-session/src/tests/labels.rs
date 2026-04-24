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

    // Verify the label was persisted by resolving it
    let resolved = mgr.resolve_target("test-label").unwrap();
    assert_eq!(resolved, id);

    // Re-open and verify the label survived persistence
    let mgr2 = SessionManager::open(mgr.file_path().to_path_buf()).unwrap();
    let resolved2 = mgr2.resolve_target("test-label").unwrap();
    assert_eq!(resolved2, id);
}

#[test]
fn test_record_label_persisted_in_annotations() {
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
    mgr.record_label("my-label").unwrap();

    // Verify via entries loaded from the document
    let entries = mgr.load_tree().unwrap();
    // The label should be in the entries as a Label type
    let all_entries =
        automerge_store::to_session_entries(&automerge_store::load_document(mgr.file_path()).unwrap()).unwrap();
    let has_label = all_entries.iter().any(|e| {
        if let SessionEntry::Label(label) = e {
            label.label == "my-label" && label.target_message_id == id
        } else {
            false
        }
    });
    assert!(has_label);
    // Suppress unused variable warning — we verified the tree loads fine
    let _ = entries;
}

#[test]
fn test_record_label_no_active_leaf() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    // Try to record a label without any messages
    let result = mgr.record_label("test");
    assert!(result.is_err());
}
