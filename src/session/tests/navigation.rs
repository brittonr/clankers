use super::*;

#[test]
fn test_set_active_head() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    let id1 = MessageId::generate();
    let id2 = MessageId::generate();
    let msg1 = AgentMessage::User(UserMessage {
        id: id1.clone(),
        content: vec![Content::Text {
            text: "First".to_string(),
        }],
        timestamp: Utc::now(),
    });
    let msg2 = AgentMessage::User(UserMessage {
        id: id2.clone(),
        content: vec![Content::Text {
            text: "Second".to_string(),
        }],
        timestamp: Utc::now(),
    });
    mgr.append_message(msg1, None).unwrap();
    mgr.append_message(msg2, Some(id1.clone())).unwrap();

    // Switch back to first message
    mgr.set_active_head(id1.clone()).unwrap();
    assert_eq!(mgr.active_leaf_id(), Some(&id1));

    // Build context should only have the first message
    let context = mgr.build_context().unwrap();
    assert_eq!(context.len(), 1);
    assert_eq!(context[0].id(), &id1);
}

#[test]
fn test_set_active_head_invalid() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    let fake_id = MessageId::new("nonexistent");
    let result = mgr.set_active_head(fake_id);
    assert!(result.is_err());
}

#[test]
fn test_rewind() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    // Create 3 messages
    let mut ids: Vec<MessageId> = Vec::new();
    for i in 0..3 {
        let id = MessageId::generate();
        let msg = AgentMessage::User(UserMessage {
            id: id.clone(),
            content: vec![Content::Text {
                text: format!("Message {}", i),
            }],
            timestamp: Utc::now(),
        });
        let parent = if i == 0 { None } else { Some(ids[i - 1].clone()) };
        mgr.append_message(msg, parent).unwrap();
        ids.push(id);
    }

    // Rewind by 1 message
    let new_head = mgr.rewind(1).unwrap();
    assert_eq!(new_head, ids[1]);
    assert_eq!(mgr.active_leaf_id(), Some(&ids[1]));

    // Context should have 2 messages
    let context = mgr.build_context().unwrap();
    assert_eq!(context.len(), 2);
}

#[test]
fn test_rewind_too_far() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    let id = MessageId::generate();
    let msg = AgentMessage::User(UserMessage {
        id: id.clone(),
        content: vec![Content::Text {
            text: "Only message".to_string(),
        }],
        timestamp: Utc::now(),
    });
    mgr.append_message(msg, None).unwrap();

    // Try to rewind past the beginning
    let result = mgr.rewind(1);
    assert!(result.is_err());
}

#[test]
fn test_resolve_target_numeric() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    let mut ids: Vec<MessageId> = Vec::new();
    for i in 0..3 {
        let id = MessageId::generate();
        let msg = AgentMessage::User(UserMessage {
            id: id.clone(),
            content: vec![Content::Text {
                text: format!("Message {}", i),
            }],
            timestamp: Utc::now(),
        });
        let parent = if i == 0 { None } else { Some(ids[i - 1].clone()) };
        mgr.append_message(msg, parent).unwrap();
        ids.push(id);
    }

    // Resolve offset 1 (should be second-to-last message)
    let resolved = mgr.resolve_target("1").unwrap();
    assert_eq!(resolved, ids[1]);

    // Resolve offset 0 (should be last message)
    let resolved = mgr.resolve_target("0").unwrap();
    assert_eq!(resolved, ids[2]);
}

#[test]
fn test_resolve_target_message_id() {
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

    // Resolve by exact message ID
    let resolved = mgr.resolve_target(&id.0).unwrap();
    assert_eq!(resolved, id);
}

#[test]
fn test_resolve_target_label() {
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
    mgr.record_label("checkpoint").unwrap();

    // Resolve by label
    let resolved = mgr.resolve_target("checkpoint").unwrap();
    assert_eq!(resolved, id);
}

#[test]
fn test_resolve_target_branch_name() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    let root = MessageId::generate();
    let branch_a = MessageId::generate();

    let msg_root = AgentMessage::User(UserMessage {
        id: root.clone(),
        content: vec![Content::Text {
            text: "Root".to_string(),
        }],
        timestamp: Utc::now(),
    });
    let msg_a = AgentMessage::User(UserMessage {
        id: branch_a.clone(),
        content: vec![Content::Text {
            text: "Branch A".to_string(),
        }],
        timestamp: Utc::now(),
    });

    mgr.append_message(msg_root, None).unwrap();
    mgr.append_message(msg_a, Some(root.clone())).unwrap();
    mgr.record_label("feature-branch").unwrap();

    // Resolve by branch name (label)
    let resolved = mgr.resolve_target("feature-branch").unwrap();
    assert_eq!(resolved, branch_a);
}

#[test]
fn test_resolve_target_invalid() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    let result = mgr.resolve_target("nonexistent");
    assert!(result.is_err());
}
