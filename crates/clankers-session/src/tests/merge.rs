use super::*;

#[test]
fn test_merge_branch_records_annotation() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    let make_msg = |id: &MessageId, text: &str| -> AgentMessage {
        AgentMessage::User(UserMessage {
            id: id.clone(),
            content: vec![Content::Text { text: text.to_string() }],
            timestamp: Utc::now(),
        })
    };

    let root = MessageId::generate();
    let a1 = MessageId::generate();
    let a2 = MessageId::generate();
    let b1 = MessageId::generate();

    mgr.append_message(make_msg(&root, "Root"), None).unwrap();
    mgr.append_message(make_msg(&a1, "Branch A msg 1"), Some(root.clone())).unwrap();
    mgr.append_message(make_msg(&a2, "Branch A msg 2"), Some(a1.clone())).unwrap();
    mgr.append_message(make_msg(&b1, "Branch B msg 1"), Some(root.clone())).unwrap();

    // Merge branch A into branch B (annotation only — no message cloning)
    let (count, new_leaf) = mgr.merge_branch(a2.clone(), b1.clone()).unwrap();
    assert_eq!(count, 2); // a1 and a2 are unique to branch A
    assert_eq!(new_leaf, b1); // active head stays on target

    // Active head should be on the target branch
    assert_eq!(mgr.active_leaf_id(), Some(&b1));

    // Context on target branch is unchanged — merge is annotation-only
    let context = mgr.build_context().unwrap();
    assert_eq!(context.len(), 2); // root -> b1

    // Source branch is still accessible
    mgr.set_active_head(a2.clone()).unwrap();
    let source_context = mgr.build_context().unwrap();
    assert_eq!(source_context.len(), 3); // root -> a1 -> a2
}

#[test]
fn test_merge_branch_same_branch_error() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    let id = MessageId::generate();
    let msg = AgentMessage::User(UserMessage {
        id: id.clone(),
        content: vec![Content::Text {
            text: "msg".to_string(),
        }],
        timestamp: Utc::now(),
    });
    mgr.append_message(msg, None).unwrap();

    let result = mgr.merge_branch(id.clone(), id);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("itself"));
}

#[test]
fn test_merge_branch_no_unique_messages() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    let make_msg = |id: &MessageId, text: &str| -> AgentMessage {
        AgentMessage::User(UserMessage {
            id: id.clone(),
            content: vec![Content::Text { text: text.to_string() }],
            timestamp: Utc::now(),
        })
    };

    // Linear chain: root -> child
    let root = MessageId::generate();
    let child = MessageId::generate();
    mgr.append_message(make_msg(&root, "Root"), None).unwrap();
    mgr.append_message(make_msg(&child, "Child"), Some(root.clone())).unwrap();

    // Merging ancestor into descendant — no unique messages
    let result = mgr.merge_branch(root, child);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("No new messages"));
}

#[test]
fn test_merge_records_metadata() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    let make_msg = |id: &MessageId, text: &str| -> AgentMessage {
        AgentMessage::User(UserMessage {
            id: id.clone(),
            content: vec![Content::Text { text: text.to_string() }],
            timestamp: Utc::now(),
        })
    };

    let root = MessageId::generate();
    let a1 = MessageId::generate();
    let b1 = MessageId::generate();

    mgr.append_message(make_msg(&root, "Root"), None).unwrap();
    mgr.append_message(make_msg(&a1, "A"), Some(root.clone())).unwrap();
    mgr.append_message(make_msg(&b1, "B"), Some(root.clone())).unwrap();

    mgr.merge_branch(a1.clone(), b1.clone()).unwrap();

    // Check that a merge annotation was written
    let doc = automerge_store::load_document(mgr.file_path()).unwrap();
    let annotations = automerge_store::read_annotations(&doc).unwrap();
    let merge_ann = annotations.iter().find(|a| a.kind_str() == "custom");
    assert!(merge_ann.is_some());
}

#[test]
fn test_merge_branch_nonexistent_source() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    let id = MessageId::generate();
    let msg = AgentMessage::User(UserMessage {
        id: id.clone(),
        content: vec![Content::Text {
            text: "msg".to_string(),
        }],
        timestamp: Utc::now(),
    });
    mgr.append_message(msg, None).unwrap();

    let result = mgr.merge_branch(MessageId::new("nonexistent"), id);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Source"));
}

#[test]
fn test_merge_selective() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    let make_msg = |id: &MessageId, text: &str| -> AgentMessage {
        AgentMessage::User(UserMessage {
            id: id.clone(),
            content: vec![Content::Text { text: text.to_string() }],
            timestamp: Utc::now(),
        })
    };

    let root = MessageId::generate();
    let a1 = MessageId::generate();
    let a2 = MessageId::generate();
    let a3 = MessageId::generate();
    let b1 = MessageId::generate();

    mgr.append_message(make_msg(&root, "Root"), None).unwrap();
    mgr.append_message(make_msg(&a1, "A1"), Some(root.clone())).unwrap();
    mgr.append_message(make_msg(&a2, "A2"), Some(a1.clone())).unwrap();
    mgr.append_message(make_msg(&a3, "A3"), Some(a2.clone())).unwrap();
    mgr.append_message(make_msg(&b1, "B1"), Some(root.clone())).unwrap();

    // Selectively merge only a1 and a3 (skip a2) — copies messages to target
    let (count, _new_leaf) = mgr.merge_selective(a3.clone(), b1.clone(), &[a1.clone(), a3.clone()]).unwrap();
    assert_eq!(count, 2);

    // Context: root -> b1 -> a1' -> a3'
    let context = mgr.build_context().unwrap();
    assert_eq!(context.len(), 4);
}

#[test]
fn test_merge_selective_empty_selection() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    let make_msg = |id: &MessageId, text: &str| -> AgentMessage {
        AgentMessage::User(UserMessage {
            id: id.clone(),
            content: vec![Content::Text { text: text.to_string() }],
            timestamp: Utc::now(),
        })
    };

    let root = MessageId::generate();
    let a1 = MessageId::generate();
    let b1 = MessageId::generate();

    mgr.append_message(make_msg(&root, "Root"), None).unwrap();
    mgr.append_message(make_msg(&a1, "A"), Some(root.clone())).unwrap();
    mgr.append_message(make_msg(&b1, "B"), Some(root.clone())).unwrap();

    let result = mgr.merge_selective(a1, b1, &[]);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("No messages selected"));
}

#[test]
fn test_cherry_pick_single() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    let make_msg = |id: &MessageId, text: &str| -> AgentMessage {
        AgentMessage::User(UserMessage {
            id: id.clone(),
            content: vec![Content::Text { text: text.to_string() }],
            timestamp: Utc::now(),
        })
    };

    let root = MessageId::generate();
    let a1 = MessageId::generate();
    let b1 = MessageId::generate();

    mgr.append_message(make_msg(&root, "Root"), None).unwrap();
    mgr.append_message(make_msg(&a1, "Branch A"), Some(root.clone())).unwrap();
    mgr.append_message(make_msg(&b1, "Branch B"), Some(root.clone())).unwrap();

    // Cherry-pick a1 into branch B
    let (count, new_leaf) = mgr.cherry_pick(a1.clone(), b1.clone(), false).unwrap();
    assert_eq!(count, 1);
    assert_eq!(mgr.active_leaf_id(), Some(&new_leaf));

    // Context: root -> b1 -> a1'
    let context = mgr.build_context().unwrap();
    assert_eq!(context.len(), 3);
}

#[test]
fn test_cherry_pick_with_children() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    let make_msg = |id: &MessageId, text: &str| -> AgentMessage {
        AgentMessage::User(UserMessage {
            id: id.clone(),
            content: vec![Content::Text { text: text.to_string() }],
            timestamp: Utc::now(),
        })
    };

    let root = MessageId::generate();
    let a1 = MessageId::generate();
    let a2 = MessageId::generate();
    let b1 = MessageId::generate();

    mgr.append_message(make_msg(&root, "Root"), None).unwrap();
    mgr.append_message(make_msg(&a1, "A1"), Some(root.clone())).unwrap();
    mgr.append_message(make_msg(&a2, "A2"), Some(a1.clone())).unwrap();
    mgr.append_message(make_msg(&b1, "B1"), Some(root.clone())).unwrap();

    // Cherry-pick a1 with children into branch B
    let (count, _new_leaf) = mgr.cherry_pick(a1.clone(), b1.clone(), true).unwrap();
    assert_eq!(count, 2); // a1 + a2

    // Context: root -> b1 -> a1' -> a2'
    let context = mgr.build_context().unwrap();
    assert_eq!(context.len(), 4);
}

#[test]
fn test_cherry_pick_nonexistent_message() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    let id = MessageId::generate();
    let msg = AgentMessage::User(UserMessage {
        id: id.clone(),
        content: vec![Content::Text {
            text: "msg".to_string(),
        }],
        timestamp: Utc::now(),
    });
    mgr.append_message(msg, None).unwrap();

    let result = mgr.cherry_pick(MessageId::new("fake"), id, false);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Message not found"));
}

#[test]
fn test_cherry_pick_records_metadata() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    let make_msg = |id: &MessageId, text: &str| -> AgentMessage {
        AgentMessage::User(UserMessage {
            id: id.clone(),
            content: vec![Content::Text { text: text.to_string() }],
            timestamp: Utc::now(),
        })
    };

    let root = MessageId::generate();
    let a1 = MessageId::generate();
    let b1 = MessageId::generate();

    mgr.append_message(make_msg(&root, "Root"), None).unwrap();
    mgr.append_message(make_msg(&a1, "A1"), Some(root.clone())).unwrap();
    mgr.append_message(make_msg(&b1, "B1"), Some(root.clone())).unwrap();

    mgr.cherry_pick(a1.clone(), b1.clone(), false).unwrap();

    let doc = automerge_store::load_document(mgr.file_path()).unwrap();
    let annotations = automerge_store::read_annotations(&doc).unwrap();
    let cp_ann = annotations.iter().find(|a| a.kind_str() == "custom");
    assert!(cp_ann.is_some());
}

#[test]
fn test_merge_selective_preserves_message_content() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    let make_msg = |id: &MessageId, text: &str| -> AgentMessage {
        AgentMessage::User(UserMessage {
            id: id.clone(),
            content: vec![Content::Text { text: text.to_string() }],
            timestamp: Utc::now(),
        })
    };

    let root = MessageId::generate();
    let a1 = MessageId::generate();
    let b1 = MessageId::generate();

    mgr.append_message(make_msg(&root, "Root"), None).unwrap();
    mgr.append_message(make_msg(&a1, "Unique content from A"), Some(root.clone())).unwrap();
    mgr.append_message(make_msg(&b1, "B1"), Some(root.clone())).unwrap();

    mgr.merge_selective(a1.clone(), b1.clone(), &[a1.clone()]).unwrap();

    // The merged message should have new ID but same content
    let context = mgr.build_context().unwrap();
    assert_eq!(context.len(), 3); // root -> b1 -> a1'

    // Last message should contain the original text
    let last_msg = &context[2];
    if let AgentMessage::User(u) = last_msg {
        if let Content::Text { text } = &u.content[0] {
            assert_eq!(text, "Unique content from A");
        } else {
            panic!("Expected text content");
        }
        // ID should be different from original
        assert_ne!(u.id, a1);
    } else {
        panic!("Expected user message");
    }
}
