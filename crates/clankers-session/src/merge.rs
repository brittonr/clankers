//! Merge and cherry-pick operations on session branches.
//!
//! With Automerge as the storage backend, merge operations are simplified:
//!
//! - `merge_branch`: Records a merge annotation. Both branches remain visible
//!   in the DAG. No message cloning.
//! - `merge_selective`: Copies selected messages from one branch to another
//!   via plain `append_message` calls.
//! - `cherry_pick`: Copies a message (and optionally its children) to a target
//!   branch via plain `append_message` calls.

use chrono::Utc;
use clankers_message::MessageId;
use serde_json::json;

use super::SessionManager;
use super::automerge_store::AnnotationEntry;
use super::entry::*;
use super::set_message_id;
use crate::error::Result;
use crate::error::SessionError;

impl SessionManager {
    /// Merge two branches by recording an annotation.
    ///
    /// Both branches remain visible in the DAG — no messages are cloned.
    /// The active head moves to `target_leaf`. Returns the number of
    /// messages unique to the source branch (informational).
    pub fn merge_branch(&mut self, source_leaf: MessageId, target_leaf: MessageId) -> Result<(usize, MessageId)> {
        if source_leaf == target_leaf {
            return Err(SessionError {
                message: "Cannot merge a branch into itself".into(),
            });
        }

        let tree = self.load_tree()?;

        tree.find_message_public(&source_leaf).ok_or_else(|| SessionError {
            message: format!("Source branch leaf not found: {}", source_leaf.0),
        })?;
        tree.find_message_public(&target_leaf).ok_or_else(|| SessionError {
            message: format!("Target branch leaf not found: {}", target_leaf.0),
        })?;

        let unique = tree.find_unique_messages(&source_leaf, &target_leaf);
        if unique.is_empty() {
            return Err(SessionError {
                message: "No new messages to merge — source branch is already merged or is an ancestor of target"
                    .into(),
            });
        }

        let merged_count = unique.len();
        let source_ids: Vec<&str> = unique.iter().map(|m| m.id.0.as_str()).collect();

        let annotation = AnnotationEntry::Custom(CustomEntry {
            id: MessageId::generate(),
            kind: "merge".to_string(),
            data: json!({
                "source_leaf": source_leaf.0,
                "target_leaf": target_leaf.0,
                "merged_message_ids": source_ids,
                "merged_count": merged_count,
                "strategy": "full",
            }),
            timestamp: Utc::now(),
        });

        crate::automerge_store::put_annotation(&mut self.doc, &annotation)?;
        crate::automerge_store::save_incremental(&mut self.doc, &self.file_path)?;

        self.active_leaf_id = Some(target_leaf.clone());
        Ok((merged_count, target_leaf))
    }

    /// Selectively merge messages from one branch into another.
    ///
    /// Copies the selected messages (by ID) into the target branch, each
    /// appended as a new message with a fresh ID.
    pub fn merge_selective(
        &mut self,
        source_leaf: MessageId,
        target_leaf: MessageId,
        selected_ids: &[MessageId],
    ) -> Result<(usize, MessageId)> {
        if source_leaf == target_leaf {
            return Err(SessionError {
                message: "Cannot merge a branch into itself".into(),
            });
        }
        if selected_ids.is_empty() {
            return Err(SessionError {
                message: "No messages selected for merge".into(),
            });
        }

        let tree = self.load_tree()?;

        tree.find_message_public(&source_leaf).ok_or_else(|| SessionError {
            message: format!("Source branch leaf not found: {}", source_leaf.0),
        })?;
        tree.find_message_public(&target_leaf).ok_or_else(|| SessionError {
            message: format!("Target branch leaf not found: {}", target_leaf.0),
        })?;

        let unique = tree.find_unique_messages(&source_leaf, &target_leaf);
        let selected_set: std::collections::HashSet<&MessageId> = selected_ids.iter().collect();
        let to_merge: Vec<_> = unique.into_iter().filter(|m| selected_set.contains(&m.id)).collect();

        if to_merge.is_empty() {
            return Err(SessionError {
                message: "None of the selected messages are unique to the source branch".into(),
            });
        }

        let merged_count = to_merge.len();
        let mut current_parent = target_leaf.clone();

        for msg in &to_merge {
            let new_id = MessageId::generate();
            let mut cloned_message = msg.message.clone();
            set_message_id(&mut cloned_message, new_id.clone());
            self.append_message(cloned_message, Some(current_parent))?;
            current_parent = new_id;
        }

        let new_leaf = current_parent;

        let annotation = AnnotationEntry::Custom(CustomEntry {
            id: MessageId::generate(),
            kind: "merge".to_string(),
            data: json!({
                "source_leaf": source_leaf.0,
                "target_leaf": target_leaf.0,
                "merged_count": merged_count,
                "strategy": "selective",
            }),
            timestamp: Utc::now(),
        });

        crate::automerge_store::put_annotation(&mut self.doc, &annotation)?;
        crate::automerge_store::save_incremental(&mut self.doc, &self.file_path)?;

        self.active_leaf_id = Some(new_leaf.clone());
        Ok((merged_count, new_leaf))
    }

    /// Cherry-pick a message (and optionally its children) into a target branch.
    ///
    /// Each message is copied with a fresh ID via `append_message`.
    #[cfg_attr(dylint_lib = "tigerstyle", allow(no_unwrap, reason = "message ID verified to exist before lookup"))]
    pub fn cherry_pick(
        &mut self,
        message_id: MessageId,
        target_leaf: MessageId,
        with_children: bool,
    ) -> Result<(usize, MessageId)> {
        let tree = self.load_tree()?;

        tree.find_message_public(&message_id).ok_or_else(|| SessionError {
            message: format!("Message not found: {}", message_id.0),
        })?;
        tree.find_message_public(&target_leaf).ok_or_else(|| SessionError {
            message: format!("Target branch leaf not found: {}", target_leaf.0),
        })?;

        let messages_to_copy = if with_children {
            // Walk children iteratively via the tree's child index.
            let mut collected = Vec::new();
            let mut stack = vec![message_id.clone()];
            while let Some(id) = stack.pop() {
                if let Some(msg) = tree.find_message_public(&id) {
                    collected.push(msg.clone());
                    let children = tree.get_children(&Some(id));
                    // Push in reverse so they're processed in insertion order.
                    for child in children.iter().rev() {
                        stack.push(child.id.clone());
                    }
                }
            }
            collected
        } else {
            let msg = tree.find_message_public(&message_id).expect("verified above");
            vec![msg.clone()]
        };

        if messages_to_copy.is_empty() {
            return Err(SessionError {
                message: "No messages to cherry-pick".into(),
            });
        }

        let count = messages_to_copy.len();
        let mut current_parent = target_leaf.clone();

        for msg in &messages_to_copy {
            let new_id = MessageId::generate();
            let mut cloned_message = msg.message.clone();
            set_message_id(&mut cloned_message, new_id.clone());
            self.append_message(cloned_message, Some(current_parent))?;
            current_parent = new_id;
        }

        let new_leaf = current_parent;

        let annotation = AnnotationEntry::Custom(CustomEntry {
            id: MessageId::generate(),
            kind: "cherry-pick".to_string(),
            data: json!({
                "source_message_id": message_id.0,
                "target_leaf": target_leaf.0,
                "with_children": with_children,
                "copied_count": count,
            }),
            timestamp: Utc::now(),
        });

        crate::automerge_store::put_annotation(&mut self.doc, &annotation)?;
        crate::automerge_store::save_incremental(&mut self.doc, &self.file_path)?;

        self.active_leaf_id = Some(new_leaf.clone());
        Ok((count, new_leaf))
    }
}
