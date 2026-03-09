use super::*;

#[test]
fn test_active_leaf_tracking() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    assert!(mgr.active_leaf_id().is_none());

    let id1 = MessageId::generate();
    let msg1 = AgentMessage::User(UserMessage {
        id: id1.clone(),
        content: vec![Content::Text {
            text: "First".to_string(),
        }],
        timestamp: Utc::now(),
    });
    mgr.append_message(msg1, None).unwrap();
    assert_eq!(mgr.active_leaf_id(), Some(&id1));

    let id2 = MessageId::generate();
    let msg2 = AgentMessage::User(UserMessage {
        id: id2.clone(),
        content: vec![Content::Text {
            text: "Second".to_string(),
        }],
        timestamp: Utc::now(),
    });
    mgr.append_message(msg2, Some(id1.clone())).unwrap();
    assert_eq!(mgr.active_leaf_id(), Some(&id2));
}

#[test]
fn test_record_branch() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    // Create a linear conversation: msg1 -> msg2 -> msg3
    let id1 = MessageId::generate();
    let id2 = MessageId::generate();
    let id3 = MessageId::generate();
    for (id, parent) in [
        (id1.clone(), None),
        (id2.clone(), Some(id1.clone())),
        (id3.clone(), Some(id2.clone())),
    ] {
        let msg = AgentMessage::User(UserMessage {
            id: id.clone(),
            content: vec![Content::Text {
                text: "msg".to_string(),
            }],
            timestamp: Utc::now(),
        });
        mgr.append_message(msg, parent).unwrap();
    }
    assert_eq!(mgr.active_leaf_id(), Some(&id3));

    // Branch from msg1
    mgr.record_branch(id1.clone(), "testing branch").unwrap();
    assert_eq!(mgr.active_leaf_id(), Some(&id1));

    // Add a new message on the branch
    let id4 = MessageId::generate();
    let msg4 = AgentMessage::User(UserMessage {
        id: id4.clone(),
        content: vec![Content::Text {
            text: "branch msg".to_string(),
        }],
        timestamp: Utc::now(),
    });
    mgr.append_message(msg4, Some(id1.clone())).unwrap();
    assert_eq!(mgr.active_leaf_id(), Some(&id4));

    // Build context should follow the new branch: msg1 -> msg4
    let context = mgr.build_context().unwrap();
    assert_eq!(context.len(), 2);
    assert_eq!(context[0].id(), &id1);
    assert_eq!(context[1].id(), &id4);
}

#[test]
fn test_open_resumes_latest_branch() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    // Create: root -> branch_a, root -> branch_b -> branch_b2
    let root = MessageId::generate();
    let branch_a = MessageId::generate();
    let branch_b = MessageId::generate();
    let branch_b2 = MessageId::generate();

    for (id, parent, text) in [
        (root.clone(), None, "root"),
        (branch_a.clone(), Some(root.clone()), "branch a"),
        (branch_b.clone(), Some(root.clone()), "branch b"),
        (branch_b2.clone(), Some(branch_b.clone()), "branch b2"),
    ] {
        let msg = AgentMessage::User(UserMessage {
            id: id.clone(),
            content: vec![Content::Text { text: text.to_string() }],
            timestamp: Utc::now(),
        });
        mgr.append_message(msg, parent).unwrap();
    }

    // Re-open — should follow the latest branch (branch_b2)
    let mgr2 = SessionManager::open(mgr.file_path().to_path_buf()).unwrap();
    let context = mgr2.build_context().unwrap();
    assert_eq!(context.len(), 3); // root -> branch_b -> branch_b2
    assert_eq!(context[0].id(), &root);
    assert_eq!(context[1].id(), &branch_b);
    assert_eq!(context[2].id(), &branch_b2);
}

#[test]
fn test_find_branches_linear() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    // Create a linear conversation
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

    let branches = mgr.find_branches().unwrap();
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].leaf_id, id2);
    assert_eq!(branches[0].message_count, 2);
    assert!(branches[0].is_active);
    assert!(branches[0].divergence_point.is_none());
}

#[test]
fn test_find_branches_with_fork() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    // Create: root -> branch_a, root -> branch_b
    let root = MessageId::generate();
    let branch_a = MessageId::generate();
    let branch_b = MessageId::generate();

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
    let msg_b = AgentMessage::User(UserMessage {
        id: branch_b.clone(),
        content: vec![Content::Text {
            text: "Branch B".to_string(),
        }],
        timestamp: Utc::now(),
    });

    mgr.append_message(msg_root, None).unwrap();
    mgr.append_message(msg_a, Some(root.clone())).unwrap();
    mgr.append_message(msg_b, Some(root.clone())).unwrap();

    let branches = mgr.find_branches().unwrap();
    assert_eq!(branches.len(), 2);

    // Both branches should have the root as divergence point
    for branch in &branches {
        assert_eq!(branch.message_count, 2);
        assert_eq!(branch.divergence_point, Some(root.clone()));
    }

    // The last branch created (branch_b) should be active
    assert!(branches.iter().any(|b| b.leaf_id == branch_b && b.is_active));
}

#[test]
fn test_find_branches_with_labels() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    let id1 = MessageId::generate();
    let msg1 = AgentMessage::User(UserMessage {
        id: id1.clone(),
        content: vec![Content::Text {
            text: "Message".to_string(),
        }],
        timestamp: Utc::now(),
    });
    mgr.append_message(msg1, None).unwrap();

    // Add a label
    mgr.record_label("my-checkpoint").unwrap();

    let branches = mgr.find_branches().unwrap();
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].name, "my-checkpoint");
}

#[test]
fn test_branch_name_from_branch_entry() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    // Create root and first branch
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

    // Record a branch with a reason
    mgr.record_branch(root.clone(), "alternate-approach").unwrap();

    // Create second branch
    let branch_b = MessageId::generate();
    let msg_b = AgentMessage::User(UserMessage {
        id: branch_b.clone(),
        content: vec![Content::Text {
            text: "Branch B".to_string(),
        }],
        timestamp: Utc::now(),
    });
    mgr.append_message(msg_b, Some(root.clone())).unwrap();

    let branches = mgr.find_branches().unwrap();
    // One of the branches should have the name from the branch entry
    let has_named_branch = branches.iter().any(|b| b.name == "alternate-approach");
    assert!(has_named_branch);
}
