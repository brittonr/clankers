//! Session persistence and tree management for agent conversations
//!
//! Manages session files as Automerge documents that record the full
//! conversation history with branching, merging, and label support.
//! Legacy JSONL files are auto-migrated to Automerge on open.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

pub mod automerge_store;
pub mod context;
pub mod entry;
pub mod error;
pub mod export;
pub mod merge;
pub mod store;
pub mod tree;

use std::path::Path;
use std::path::PathBuf;

use automerge::AutoCommit;
use chrono::Utc;
use clanker_message::AgentMessage;
use clanker_message::MessageId;

use self::automerge_store::AnnotationEntry;
use self::entry::*;
use self::error::Result;
use self::error::SessionError;
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
    /// In-memory Automerge document holding the full session state.
    doc: AutoCommit,
    /// Track which message IDs have already been persisted to avoid duplicates.
    persisted_ids: std::collections::HashSet<MessageId>,
    /// The ID of the last message on the currently active branch.
    active_leaf_id: Option<MessageId>,
    /// Latest persisted compaction summary for iterative structured compaction.
    latest_compaction_summary: Option<String>,
    /// Worktree path if this session is running in a worktree
    worktree_path: Option<String>,
    /// Worktree branch name
    worktree_branch: Option<String>,
}

impl SessionManager {
    /// Create a new session backed by an Automerge document.
    pub fn create(
        sessions_dir: &Path,
        cwd: &str,
        model: &str,
        agent: Option<&str>,
        worktree_path: Option<&str>,
        worktree_branch: Option<&str>,
    ) -> Result<Self> {
        let session_id = clanker_message::generate_id();
        let file_path = store::session_file_path_automerge(sessions_dir, cwd, &session_id);

        let header = HeaderEntry {
            session_id: session_id.clone(),
            created_at: Utc::now(),
            cwd: cwd.to_string(),
            model: model.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            agent: agent.map(String::from),
            parent_session_id: None,
            worktree_path: worktree_path.map(String::from),
            worktree_branch: worktree_branch.map(String::from),
        };

        let mut doc = automerge_store::create_document(&header)?;
        automerge_store::save_document(&mut doc, &file_path)?;

        Ok(Self {
            session_id,
            file_path,
            cwd: cwd.to_string(),
            model: model.to_string(),
            doc,
            persisted_ids: std::collections::HashSet::new(),
            active_leaf_id: None,
            latest_compaction_summary: None,
            worktree_path: worktree_path.map(String::from),
            worktree_branch: worktree_branch.map(String::from),
        })
    }

    /// Open an existing session.
    ///
    /// Supports both `.automerge` (native) and `.jsonl` (legacy) files.
    /// Legacy JSONL files are auto-migrated: an `.automerge` file is saved
    /// alongside and all subsequent writes go to it.
    pub fn open(file_path: PathBuf) -> Result<Self> {
        let is_jsonl = file_path.extension().is_some_and(|ext| ext == "jsonl");

        let (doc, file_path) = if is_jsonl {
            // Legacy path: read JSONL, build Automerge doc, save alongside
            let entries = store::read_entries(&file_path)?;
            let mut doc = Self::build_doc_from_entries(&entries)?;
            let automerge_path = file_path.with_extension("automerge");
            automerge_store::save_document(&mut doc, &automerge_path)?;
            (doc, automerge_path)
        } else {
            let doc = automerge_store::load_document(&file_path)?;
            (doc, file_path)
        };

        let header = automerge_store::read_header(&doc)?;
        let messages = automerge_store::read_messages(&doc)?;

        let persisted_ids: std::collections::HashSet<MessageId> = messages.iter().map(|m| m.id.clone()).collect();

        let entries = automerge_store::to_session_entries(&doc)?;
        let latest_compaction_summary = entries
            .iter()
            .filter_map(|entry| {
                if let SessionEntry::Compaction(compaction) = entry {
                    Some(compaction.summary.clone())
                } else {
                    None
                }
            })
            .last();
        let tree = SessionTree::build(entries);
        let active_leaf_id = tree.find_latest_leaf(None).or_else(|| tree.latest_message()).map(|m| m.id.clone());

        Ok(Self {
            session_id: header.session_id,
            file_path,
            cwd: header.cwd,
            model: header.model,
            doc,
            persisted_ids,
            active_leaf_id,
            latest_compaction_summary,
            worktree_path: header.worktree_path,
            worktree_branch: header.worktree_branch,
        })
    }

    /// Build an Automerge doc from parsed JSONL entries (for migration).
    fn build_doc_from_entries(entries: &[SessionEntry]) -> Result<AutoCommit> {
        let header = entries
            .iter()
            .find_map(|e| {
                if let SessionEntry::Header(h) = e {
                    Some(h.clone())
                } else {
                    None
                }
            })
            .ok_or_else(|| SessionError {
                message: "No header entry".into(),
            })?;

        let mut doc = automerge_store::create_document(&header)?;

        for entry in entries {
            match entry {
                SessionEntry::Message(m) => {
                    automerge_store::put_message(&mut doc, m)?;
                }
                SessionEntry::Header(_) => {} // already handled
                other => {
                    if let Some(annotation) = AnnotationEntry::from_session_entry(other) {
                        automerge_store::put_annotation(&mut doc, &annotation)?;
                    }
                }
            }
        }

        Ok(doc)
    }

    /// Append a message to the session (skips if already persisted).
    pub fn append_message(&mut self, message: AgentMessage, parent_id: Option<MessageId>) -> Result<()> {
        let id = message.id().clone();
        if self.persisted_ids.contains(&id) {
            return Ok(());
        }

        let entry = MessageEntry {
            id: id.clone(),
            parent_id,
            message,
            timestamp: Utc::now(),
        };

        automerge_store::put_message(&mut self.doc, &entry)?;
        automerge_store::save_incremental(&mut self.doc, &self.file_path)?;
        self.persisted_ids.insert(id.clone());
        self.active_leaf_id = Some(id);
        Ok(())
    }

    /// Record a branch point and update the active leaf.
    pub fn record_branch(&mut self, from_message_id: MessageId, reason: &str) -> Result<()> {
        let annotation = AnnotationEntry::Branch(BranchEntry {
            id: MessageId::generate(),
            from_message_id: from_message_id.clone(),
            reason: reason.to_string(),
            timestamp: Utc::now(),
        });

        automerge_store::put_annotation(&mut self.doc, &annotation)?;
        automerge_store::save_incremental(&mut self.doc, &self.file_path)?;
        self.active_leaf_id = Some(from_message_id);
        Ok(())
    }

    /// Record a label for the current active leaf.
    pub fn record_label(&mut self, label: &str) -> Result<()> {
        let target_id = self.active_leaf_id.as_ref().ok_or_else(|| SessionError {
            message: "No active leaf to label".to_string(),
        })?;

        let annotation = AnnotationEntry::Label(LabelEntry {
            id: MessageId::generate(),
            target_message_id: target_id.clone(),
            label: label.to_string(),
            timestamp: Utc::now(),
        });

        automerge_store::put_annotation(&mut self.doc, &annotation)?;
        automerge_store::save_incremental(&mut self.doc, &self.file_path)?;
        Ok(())
    }

    /// Record a session resume event.
    pub fn record_resume(&mut self, from_entry_id: MessageId) -> Result<()> {
        let annotation = AnnotationEntry::Resume(ResumeEntry {
            id: MessageId::generate(),
            resumed_at: Utc::now(),
            from_entry_id,
        });

        automerge_store::put_annotation(&mut self.doc, &annotation)?;
        automerge_store::save_incremental(&mut self.doc, &self.file_path)?;
        Ok(())
    }

    /// Record a compaction summary annotation for iterative reuse.
    pub fn record_compaction_summary(&mut self, summary: String) -> Result<()> {
        let annotation = AnnotationEntry::Compaction(CompactionEntry {
            id: MessageId::generate(),
            compacted_range: Vec::new(),
            summary: summary.clone(),
            tokens_before: 0,
            tokens_after: 0,
            timestamp: Utc::now(),
        });

        automerge_store::put_annotation(&mut self.doc, &annotation)?;
        automerge_store::save_incremental(&mut self.doc, &self.file_path)?;
        self.latest_compaction_summary = Some(summary);
        Ok(())
    }

    pub fn latest_compaction_summary(&self) -> Option<&str> {
        self.latest_compaction_summary.as_deref()
    }

    /// Get the current active leaf message ID.
    pub fn active_leaf_id(&self) -> Option<&MessageId> {
        self.active_leaf_id.as_ref()
    }

    /// Check if a message has already been persisted.
    pub fn is_persisted(&self, id: &MessageId) -> bool {
        self.persisted_ids.contains(id)
    }

    /// Load all session entries from the in-memory document.
    fn load_entries(&self) -> Result<Vec<SessionEntry>> {
        automerge_store::to_session_entries(&self.doc)
    }

    /// Load the session tree from the in-memory document.
    pub fn load_tree(&self) -> Result<SessionTree> {
        let entries = self.load_entries()?;
        Ok(SessionTree::build(entries))
    }

    /// Build LLM context messages from the session's active branch.
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

    /// Perform a full compacted save of the document.
    ///
    /// Call on session close or when incremental size grows large.
    pub fn save_compact(&mut self) -> Result<()> {
        automerge_store::save_document(&mut self.doc, &self.file_path)
    }

    /// Find all branches in the session and return their metadata.
    pub fn find_branches(&self) -> Result<Vec<BranchInfo>> {
        let entries = self.load_entries()?;
        let tree = SessionTree::build(entries.clone());
        let leaves = tree.find_all_leaves();

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

    /// Set the active head to a specific message ID.
    pub fn set_active_head(&mut self, message_id: MessageId) -> Result<()> {
        let tree = self.load_tree()?;
        tree.find_message_public(&message_id).ok_or_else(|| SessionError {
            message: format!("Message not found: {}", message_id.0),
        })?;
        self.active_leaf_id = Some(message_id);
        Ok(())
    }

    /// Rewind the active branch by a number of messages.
    pub fn rewind(&mut self, offset: usize) -> Result<MessageId> {
        let tree = self.load_tree()?;
        let current_leaf = self.active_leaf_id.as_ref().ok_or_else(|| SessionError {
            message: "No active branch to rewind".to_string(),
        })?;

        let branch = tree.walk_branch(current_leaf);
        if offset >= branch.len() {
            return Err(SessionError {
                message: format!("Cannot rewind {} messages from a branch of length {}", offset, branch.len()),
            });
        }

        let target_index = branch.len() - offset - 1;
        let new_head = branch[target_index].id.clone();
        self.active_leaf_id = Some(new_head.clone());
        Ok(new_head)
    }

    /// Resolve a target string to a MessageId.
    ///
    /// Supports: exact message ID, numeric offset from active leaf, label name,
    /// and branch name.
    pub fn resolve_target(&self, target: &str) -> Result<MessageId> {
        let entries = self.load_entries()?;
        let tree = SessionTree::build(entries.clone());

        if tree.find_message_public(&MessageId::new(target)).is_some() {
            return Ok(MessageId::new(target));
        }

        if let Ok(offset) = target.parse::<usize>()
            && offset < 1000
        {
            let current_leaf = self.active_leaf_id.as_ref().ok_or_else(|| SessionError {
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

        Err(SessionError {
            message: format!("Could not resolve target: {}", target),
        })
    }
}

#[cfg(test)]
mod tests;
