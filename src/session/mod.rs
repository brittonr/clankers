//! Session persistence manager

pub mod context;
pub mod entry;
pub mod store;
pub mod tree;

use std::path::Path;
use std::path::PathBuf;

use chrono::Utc;

use self::entry::*;
use self::tree::SessionTree;
use crate::error::Result;
use crate::provider::message::AgentMessage;
use crate::provider::message::MessageId;

pub struct SessionManager {
    session_id: String,
    file_path: PathBuf,
    cwd: String,
    model: String,
    /// Track which message IDs have already been persisted to avoid re-reading the file
    persisted_ids: std::collections::HashSet<MessageId>,
    /// The ID of the last message on the currently active branch.
    /// Used to chain `parent_id` when persisting new messages after a branch.
    active_leaf_id: Option<MessageId>,
    /// Worktree path if this session is running in a worktree
    worktree_path: Option<String>,
    /// Worktree branch name
    worktree_branch: Option<String>,
}

impl SessionManager {
    /// Create a new session
    pub fn create(
        sessions_dir: &Path,
        cwd: &str,
        model: &str,
        agent: Option<&str>,
        worktree_path: Option<&str>,
        worktree_branch: Option<&str>,
    ) -> Result<Self> {
        let session_id = crate::util::id::generate_id();
        let file_path = store::session_file_path(sessions_dir, cwd, &session_id);

        let header = SessionEntry::Header(HeaderEntry {
            session_id: session_id.clone(),
            created_at: Utc::now(),
            cwd: cwd.to_string(),
            model: model.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            agent: agent.map(String::from),
            parent_session_id: None,
            worktree_path: worktree_path.map(String::from),
            worktree_branch: worktree_branch.map(String::from),
        });

        store::append_entry(&file_path, &header)?;

        Ok(Self {
            session_id,
            file_path,
            cwd: cwd.to_string(),
            model: model.to_string(),
            persisted_ids: std::collections::HashSet::new(),
            active_leaf_id: None,
            worktree_path: worktree_path.map(String::from),
            worktree_branch: worktree_branch.map(String::from),
        })
    }

    /// Open an existing session
    pub fn open(file_path: PathBuf) -> Result<Self> {
        let entries = store::read_entries(&file_path)?;
        let header = entries
            .iter()
            .find_map(|e| {
                if let SessionEntry::Header(h) = e {
                    Some(h.clone())
                } else {
                    None
                }
            })
            .ok_or_else(|| crate::error::Error::Session {
                message: "No header entry".into(),
            })?;

        // Collect already-persisted message IDs so we don't re-write them
        let persisted_ids: std::collections::HashSet<MessageId> = entries
            .iter()
            .filter_map(|e| {
                if let SessionEntry::Message(m) = e {
                    Some(m.id.clone())
                } else {
                    None
                }
            })
            .collect();

        // Find the latest leaf to set the active branch
        let tree = SessionTree::build(entries);
        let active_leaf_id = tree.find_latest_leaf(None).or_else(|| tree.latest_message()).map(|m| m.id.clone());

        Ok(Self {
            session_id: header.session_id.clone(),
            file_path,
            cwd: header.cwd.clone(),
            model: header.model.clone(),
            persisted_ids,
            active_leaf_id,
            worktree_path: header.worktree_path.clone(),
            worktree_branch: header.worktree_branch.clone(),
        })
    }

    /// Append a message to the session (skips if already persisted)
    pub fn append_message(&mut self, message: AgentMessage, parent_id: Option<MessageId>) -> Result<()> {
        let id = message.id().clone();
        if self.persisted_ids.contains(&id) {
            return Ok(());
        }
        let entry = SessionEntry::Message(MessageEntry {
            id: id.clone(),
            parent_id,
            message,
            timestamp: Utc::now(),
        });
        store::append_entry(&self.file_path, &entry)?;
        self.persisted_ids.insert(id.clone());
        self.active_leaf_id = Some(id);
        Ok(())
    }

    /// Record a branch point in the session file and update the active leaf.
    /// `from_message_id` is the message we're branching from (the fork point).
    /// After calling this, subsequent `append_message` calls should use the
    /// fork point as the parent for the first new message.
    pub fn record_branch(&mut self, from_message_id: MessageId, reason: &str) -> Result<()> {
        let entry = SessionEntry::Branch(BranchEntry {
            id: MessageId::generate(),
            from_message_id: from_message_id.clone(),
            reason: reason.to_string(),
            timestamp: Utc::now(),
        });
        store::append_entry(&self.file_path, &entry)?;
        self.active_leaf_id = Some(from_message_id);
        Ok(())
    }

    /// Get the current active leaf message ID
    pub fn active_leaf_id(&self) -> Option<&MessageId> {
        self.active_leaf_id.as_ref()
    }

    /// Check if a message has already been persisted
    pub fn is_persisted(&self, id: &MessageId) -> bool {
        self.persisted_ids.contains(id)
    }

    /// Load the session tree
    pub fn load_tree(&self) -> Result<SessionTree> {
        let entries = store::read_entries(&self.file_path)?;
        Ok(SessionTree::build(entries))
    }

    /// Build LLM context messages from the session's active branch
    pub fn build_context(&self) -> Result<Vec<AgentMessage>> {
        let tree = self.load_tree()?;
        Ok(context::build_messages_for_branch(&tree, self.active_leaf_id.as_ref()))
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }
    pub fn file_path(&self) -> &Path {
        &self.file_path
    }
    pub fn cwd(&self) -> &str {
        &self.cwd
    }
    pub fn model(&self) -> &str {
        &self.model
    }
    pub fn message_count(&self) -> usize {
        self.persisted_ids.len()
    }
    pub fn worktree_path(&self) -> Option<&str> {
        self.worktree_path.as_deref()
    }
    pub fn worktree_branch(&self) -> Option<&str> {
        self.worktree_branch.as_deref()
    }
}

#[cfg(test)]
mod tests {
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
}
