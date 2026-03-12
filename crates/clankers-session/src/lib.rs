//! Session persistence and tree management for agent conversations
//!
//! Manages JSONL session files that record the full conversation history
//! as an append-only log with branching, merging, and label support.

pub mod context;
pub mod entry;
pub mod error;
pub mod export;
pub mod merge;
pub mod store;
pub mod tree;

use std::path::Path;
use std::path::PathBuf;

use chrono::Utc;
use clankers_message::AgentMessage;
use clankers_message::MessageId;

use self::entry::*;
use self::error::Result;
use self::tree::SessionTree;

#[derive(Debug, Clone)]
pub struct BranchInfo {
    pub leaf_id: MessageId,
    pub name: String,
    pub message_count: usize,
    pub last_activity: chrono::DateTime<chrono::Utc>,
    pub divergence_point: Option<MessageId>,
    pub is_active: bool,
}

/// Set the internal message ID on any AgentMessage variant.
pub fn set_message_id(msg: &mut AgentMessage, new_id: MessageId) {
    match msg {
        AgentMessage::User(m) => m.id = new_id,
        AgentMessage::Assistant(m) => m.id = new_id,
        AgentMessage::ToolResult(m) => m.id = new_id,
        AgentMessage::BashExecution(m) => m.id = new_id,
        AgentMessage::Custom(m) => m.id = new_id,
        AgentMessage::BranchSummary(m) => m.id = new_id,
        AgentMessage::CompactionSummary(m) => m.id = new_id,
    }
}

pub struct SessionManager {
    session_id: String,
    file_path: PathBuf,
    cwd: String,
    model: String,
    /// Track which message IDs have already been persisted to avoid re-reading the file
    persisted_ids: std::collections::HashSet<MessageId>,
    /// The ID of the last message on the currently active branch.
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
        let session_id = clankers_message::generate_id();
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
            .ok_or_else(|| error::SessionError {
                message: "No header entry".into(),
            })?;

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

    /// Find all branches in the session and return their metadata
    pub fn find_branches(&self) -> Result<Vec<BranchInfo>> {
        let tree = self.load_tree()?;
        let leaves = tree.find_all_leaves();
        let entries = store::read_entries(&self.file_path)?;

        let mut branches = Vec::new();

        for leaf in leaves {
            let leaf_id = leaf.id.clone();
            let branch_messages = tree.walk_branch(&leaf_id);
            let message_count = branch_messages.len();
            let last_activity = leaf.timestamp;
            let is_active = self.active_leaf_id.as_ref() == Some(&leaf_id);

            let name = self.resolve_branch_name(&leaf_id, &branch_messages, &entries);
            let divergence_point = self.find_divergence_point_for_branch(&branch_messages, &tree);

            branches.push(BranchInfo {
                leaf_id,
                name,
                message_count,
                last_activity,
                divergence_point,
                is_active,
            });
        }

        Ok(branches)
    }

    fn resolve_branch_name(
        &self,
        leaf_id: &MessageId,
        branch_messages: &[&entry::MessageEntry],
        entries: &[SessionEntry],
    ) -> String {
        let branch_ids: std::collections::HashSet<_> = branch_messages.iter().map(|m| &m.id).collect();

        for entry in entries.iter().rev() {
            if let SessionEntry::Label(label) = entry
                && (branch_ids.contains(&label.target_message_id) || &label.target_message_id == leaf_id)
            {
                return label.label.clone();
            }
        }

        for entry in entries.iter().rev() {
            if let SessionEntry::Branch(branch) = entry
                && branch_ids.contains(&branch.from_message_id)
                && !branch.reason.is_empty()
                && branch.reason.len() < 50
            {
                return branch.reason.clone();
            }
        }

        format!("branch-{}", leaf_id.0.chars().take(8).collect::<String>())
    }

    fn find_divergence_point_for_branch(
        &self,
        branch_messages: &[&entry::MessageEntry],
        tree: &SessionTree,
    ) -> Option<MessageId> {
        for msg in branch_messages.iter().rev() {
            if let Some(parent_id) = &msg.parent_id {
                let siblings = tree.get_children(&Some(parent_id.clone()));
                if siblings.len() > 1 {
                    return Some(parent_id.clone());
                }
            }
        }
        None
    }

    /// Set the active head to a specific message ID
    pub fn set_active_head(&mut self, message_id: MessageId) -> Result<()> {
        let tree = self.load_tree()?;
        tree.find_message_public(&message_id).ok_or_else(|| error::SessionError {
            message: format!("Message not found: {}", message_id.0),
        })?;
        self.active_leaf_id = Some(message_id);
        Ok(())
    }

    /// Rewind the active branch by a number of messages
    pub fn rewind(&mut self, offset: usize) -> Result<MessageId> {
        let tree = self.load_tree()?;
        let current_leaf = self.active_leaf_id.as_ref().ok_or_else(|| error::SessionError {
            message: "No active branch to rewind".to_string(),
        })?;

        let branch = tree.walk_branch(current_leaf);
        if offset >= branch.len() {
            return Err(error::SessionError {
                message: format!("Cannot rewind {} messages from a branch of length {}", offset, branch.len()),
            });
        }

        let target_index = branch.len() - offset - 1;
        let new_head = branch[target_index].id.clone();
        self.active_leaf_id = Some(new_head.clone());
        Ok(new_head)
    }

    /// Resolve a target string to a MessageId
    pub fn resolve_target(&self, target: &str) -> Result<MessageId> {
        let tree = self.load_tree()?;
        let entries = store::read_entries(&self.file_path)?;

        if tree.find_message_public(&MessageId::new(target)).is_some() {
            return Ok(MessageId::new(target));
        }

        if let Ok(offset) = target.parse::<usize>()
            && offset < 1000
        {
            let current_leaf = self.active_leaf_id.as_ref().ok_or_else(|| error::SessionError {
                message: "No active branch for offset resolution".to_string(),
            })?;
            let branch = tree.walk_branch(current_leaf);
            if offset < branch.len() {
                let target_index = branch.len() - offset - 1;
                return Ok(branch[target_index].id.clone());
            }
        }

        for entry in entries.iter().rev() {
            if let SessionEntry::Label(label) = entry
                && label.label == target
            {
                return Ok(label.target_message_id.clone());
            }
        }

        let branches = self.find_branches()?;
        for branch in branches {
            if branch.name == target {
                return Ok(branch.leaf_id);
            }
        }

        Err(error::SessionError {
            message: format!("Could not resolve target: {}", target),
        })
    }

    /// Record a label for the current active leaf
    pub fn record_label(&mut self, label: &str) -> Result<()> {
        let target_id = self.active_leaf_id.as_ref().ok_or_else(|| error::SessionError {
            message: "No active leaf to label".to_string(),
        })?;

        let entry = SessionEntry::Label(LabelEntry {
            id: MessageId::generate(),
            target_message_id: target_id.clone(),
            label: label.to_string(),
            timestamp: Utc::now(),
        });

        store::append_entry(&self.file_path, &entry)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
