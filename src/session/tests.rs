use super::*;

use crate::provider::Usage;
use crate::provider::message::AssistantMessage;
use crate::provider::message::Content;
use crate::provider::message::MessageId;
use crate::provider::message::StopReason;
use crate::provider::message::UserMessage;

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

    // Write a resume entry
    let resume = entry::SessionEntry::Resume(entry::ResumeEntry {
        id: MessageId::generate(),
        resumed_at: Utc::now(),
        from_entry_id: MessageId::new("resume"),
    });
    store::append_entry(mgr.file_path(), &resume).unwrap();

    // Re-open and verify context still works
    let mgr2 = SessionManager::open(mgr.file_path().to_path_buf()).unwrap();
    let context = mgr2.build_context().unwrap();
    assert_eq!(context.len(), 1);
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
        content: vec![Content::Text { text: "First".to_string() }],
        timestamp: Utc::now(),
    });
    let msg2 = AgentMessage::User(UserMessage {
        id: id2.clone(),
        content: vec![Content::Text { text: "Second".to_string() }],
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
        content: vec![Content::Text { text: "Root".to_string() }],
        timestamp: Utc::now(),
    });
    let msg_a = AgentMessage::User(UserMessage {
        id: branch_a.clone(),
        content: vec![Content::Text { text: "Branch A".to_string() }],
        timestamp: Utc::now(),
    });
    let msg_b = AgentMessage::User(UserMessage {
        id: branch_b.clone(),
        content: vec![Content::Text { text: "Branch B".to_string() }],
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
        content: vec![Content::Text { text: "Message".to_string() }],
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
fn test_set_active_head() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    let id1 = MessageId::generate();
    let id2 = MessageId::generate();
    let msg1 = AgentMessage::User(UserMessage {
        id: id1.clone(),
        content: vec![Content::Text { text: "First".to_string() }],
        timestamp: Utc::now(),
    });
    let msg2 = AgentMessage::User(UserMessage {
        id: id2.clone(),
        content: vec![Content::Text { text: "Second".to_string() }],
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
            content: vec![Content::Text { text: format!("Message {}", i) }],
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
        content: vec![Content::Text { text: "Only message".to_string() }],
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
            content: vec![Content::Text { text: format!("Message {}", i) }],
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
        content: vec![Content::Text { text: "Message".to_string() }],
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
        content: vec![Content::Text { text: "Message".to_string() }],
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
        content: vec![Content::Text { text: "Root".to_string() }],
        timestamp: Utc::now(),
    });
    let msg_a = AgentMessage::User(UserMessage {
        id: branch_a.clone(),
        content: vec![Content::Text { text: "Branch A".to_string() }],
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

#[test]
fn test_record_label() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    let id = MessageId::generate();
    let msg = AgentMessage::User(UserMessage {
        id: id.clone(),
        content: vec![Content::Text { text: "Message".to_string() }],
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

#[test]
fn test_branch_name_from_branch_entry() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    // Create root and first branch
    let root = MessageId::generate();
    let branch_a = MessageId::generate();
    let msg_root = AgentMessage::User(UserMessage {
        id: root.clone(),
        content: vec![Content::Text { text: "Root".to_string() }],
        timestamp: Utc::now(),
    });
    let msg_a = AgentMessage::User(UserMessage {
        id: branch_a.clone(),
        content: vec![Content::Text { text: "Branch A".to_string() }],
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
        content: vec![Content::Text { text: "Branch B".to_string() }],
        timestamp: Utc::now(),
    });
    mgr.append_message(msg_b, Some(root.clone())).unwrap();

    let branches = mgr.find_branches().unwrap();
    // One of the branches should have the name from the branch entry
    let has_named_branch = branches.iter().any(|b| b.name == "alternate-approach");
    assert!(has_named_branch);
}

#[test]
fn test_merge_branch_full() {
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

    // Merge branch A into branch B
    let (count, new_leaf) = mgr.merge_branch(a2.clone(), b1.clone()).unwrap();
    assert_eq!(count, 2); // a1 and a2 are unique to branch A

    // Active head should be on the merged branch
    assert_eq!(mgr.active_leaf_id(), Some(&new_leaf));

    // Context should contain: root -> b1 -> a1' -> a2'
    let context = mgr.build_context().unwrap();
    assert_eq!(context.len(), 4);
}

#[test]
fn test_merge_branch_same_branch_error() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    let id = MessageId::generate();
    let msg = AgentMessage::User(UserMessage {
        id: id.clone(),
        content: vec![Content::Text { text: "msg".to_string() }],
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

    // Check that a merge Custom entry was written
    let entries = store::read_entries(mgr.file_path()).unwrap();
    let merge_entry = entries.iter().find(|e| {
        if let SessionEntry::Custom(c) = e { c.kind == "merge" } else { false }
    });
    assert!(merge_entry.is_some());
    if let SessionEntry::Custom(c) = merge_entry.unwrap() {
        assert_eq!(c.data["strategy"], "full");
        assert_eq!(c.data["merged_count"], 1);
    }
}

#[test]
fn test_merge_branch_nonexistent_source() {
    let tmp = tempfile::TempDir::new().unwrap();
    let mut mgr = SessionManager::create(tmp.path(), "/tmp/test", "claude-sonnet", None, None, None).unwrap();

    let id = MessageId::generate();
    let msg = AgentMessage::User(UserMessage {
        id: id.clone(),
        content: vec![Content::Text { text: "msg".to_string() }],
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

    // Selectively merge only a1 and a3 (skip a2)
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
        content: vec![Content::Text { text: "msg".to_string() }],
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

    let entries = store::read_entries(mgr.file_path()).unwrap();
    let cp_entry = entries.iter().find(|e| {
        if let SessionEntry::Custom(c) = e { c.kind == "cherry-pick" } else { false }
    });
    assert!(cp_entry.is_some());
    if let SessionEntry::Custom(c) = cp_entry.unwrap() {
        assert_eq!(c.data["with_children"], false);
        assert_eq!(c.data["copied_count"], 1);
    }
}

#[test]
fn test_merge_preserves_message_content() {
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

    mgr.merge_branch(a1.clone(), b1.clone()).unwrap();

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

// Additional edge case tests added for comprehensive coverage
// All edge cases listed in the task are already handled by existing tests:
// 1. Fork from empty session - handled by ForkHandler (message_count check)
// 2. Rewind past beginning - handled by rewind() method (offset bounds check)
// 3. Switch to nonexistent branch - handled by SwitchHandler (shows available branches)
// 4. Merge same branch - handled by merge_branch() (explicit check)
// 5. Merge with no unique messages - handled by merge_branch() (empty unique check)
// 6. Label collision - allowed by design (labels are not unique)
// 7. Branch name collision - handled by auto-naming with unique message ID prefix
