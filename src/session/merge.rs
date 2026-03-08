//! Merge, selective merge, and cherry-pick operations on session branches.

use chrono::Utc;
use std::collections::{HashMap, HashSet};

use super::entry::*;
use super::store;
use super::tree::SessionTree;
use super::{set_message_id, SessionManager};
use crate::error::Result;
use crate::provider::message::MessageId;

impl SessionManager {
    /// Merge messages from one branch into another.
    ///
    /// Copies messages unique to `source_leaf` (not shared with `target_leaf`)
    /// and appends them as children of the target branch's leaf. Returns the
    /// number of messages merged and the new leaf ID on the target branch.
    pub fn merge_branch(
        &mut self,
        source_leaf: MessageId,
        target_leaf: MessageId,
    ) -> Result<(usize, MessageId)> {
        if source_leaf == target_leaf {
            return Err(crate::error::Error::Session {
                message: "Cannot merge a branch into itself".into(),
            });
        }

        let tree = self.load_tree()?;

        // Verify both leaves exist
        tree.find_message_public(&source_leaf).ok_or_else(|| crate::error::Error::Session {
            message: format!("Source branch leaf not found: {}", source_leaf.0),
        })?;
        tree.find_message_public(&target_leaf).ok_or_else(|| crate::error::Error::Session {
            message: format!("Target branch leaf not found: {}", target_leaf.0),
        })?;

        // Find messages unique to source (not shared with target)
        let unique = tree.find_unique_messages(&source_leaf, &target_leaf);
        if unique.is_empty() {
            return Err(crate::error::Error::Session {
                message: "No new messages to merge — source branch is already merged or is an ancestor of target".into(),
            });
        }

        let merged_count = unique.len();
        let source_ids: Vec<MessageId> = unique.iter().map(|m| m.id.clone()).collect();

        // Copy messages with new IDs, chaining parent_id from target leaf
        let mut parent = target_leaf.clone();
        let mut new_leaf = parent.clone();
        for msg in &unique {
            let new_id = MessageId::generate();
            let mut cloned_message = msg.message.clone();
            set_message_id(&mut cloned_message, new_id.clone());
            let entry = SessionEntry::Message(MessageEntry {
                id: new_id.clone(),
                parent_id: Some(parent.clone()),
                message: cloned_message,
                timestamp: Utc::now(),
            });
            store::append_entry(&self.file_path, &entry)?;
            self.persisted_ids.insert(new_id.clone());
            parent = new_id.clone();
            new_leaf = new_id;
        }

        // Record merge metadata as a CustomEntry
        let merge_entry = SessionEntry::Custom(CustomEntry {
            id: MessageId::generate(),
            kind: "merge".to_string(),
            data: serde_json::json!({
                "source_leaf": source_leaf.0,
                "target_leaf": target_leaf.0,
                "merged_message_ids": source_ids.iter().map(|id| &id.0).collect::<Vec<_>>(),
                "merged_count": merged_count,
                "strategy": "full",
            }),
            timestamp: Utc::now(),
        });
        store::append_entry(&self.file_path, &merge_entry)?;

        // Switch to the target branch's new leaf
        self.active_leaf_id = Some(new_leaf.clone());

        Ok((merged_count, new_leaf))
    }

    /// Merge only selected messages from one branch into another.
    /// `selected_ids` specifies which source message IDs to copy.
    /// Messages are copied in their original order (root→leaf).
    pub fn merge_selective(
        &mut self,
        source_leaf: MessageId,
        target_leaf: MessageId,
        selected_ids: &[MessageId],
    ) -> Result<(usize, MessageId)> {
        if source_leaf == target_leaf {
            return Err(crate::error::Error::Session {
                message: "Cannot merge a branch into itself".into(),
            });
        }
        if selected_ids.is_empty() {
            return Err(crate::error::Error::Session {
                message: "No messages selected for merge".into(),
            });
        }

        let tree = self.load_tree()?;

        // Verify both leaves exist
        tree.find_message_public(&source_leaf).ok_or_else(|| crate::error::Error::Session {
            message: format!("Source branch leaf not found: {}", source_leaf.0),
        })?;
        tree.find_message_public(&target_leaf).ok_or_else(|| crate::error::Error::Session {
            message: format!("Target branch leaf not found: {}", target_leaf.0),
        })?;

        // Get unique messages and filter to selected ones (preserving order)
        let unique = tree.find_unique_messages(&source_leaf, &target_leaf);
        let selected_set: HashSet<&MessageId> = selected_ids.iter().collect();
        let to_merge: Vec<_> = unique.into_iter().filter(|m| selected_set.contains(&m.id)).collect();

        if to_merge.is_empty() {
            return Err(crate::error::Error::Session {
                message: "None of the selected messages are unique to the source branch".into(),
            });
        }

        let merged_count = to_merge.len();
        let source_ids: Vec<MessageId> = to_merge.iter().map(|m| m.id.clone()).collect();

        // Copy selected messages with new IDs
        let mut parent = target_leaf.clone();
        let mut new_leaf = parent.clone();
        for msg in &to_merge {
            let new_id = MessageId::generate();
            let mut cloned_message = msg.message.clone();
            set_message_id(&mut cloned_message, new_id.clone());
            let entry = SessionEntry::Message(MessageEntry {
                id: new_id.clone(),
                parent_id: Some(parent.clone()),
                message: cloned_message,
                timestamp: Utc::now(),
            });
            store::append_entry(&self.file_path, &entry)?;
            self.persisted_ids.insert(new_id.clone());
            parent = new_id.clone();
            new_leaf = new_id;
        }

        // Record merge metadata
        let merge_entry = SessionEntry::Custom(CustomEntry {
            id: MessageId::generate(),
            kind: "merge".to_string(),
            data: serde_json::json!({
                "source_leaf": source_leaf.0,
                "target_leaf": target_leaf.0,
                "merged_message_ids": source_ids.iter().map(|id| &id.0).collect::<Vec<_>>(),
                "merged_count": merged_count,
                "strategy": "selective",
            }),
            timestamp: Utc::now(),
        });
        store::append_entry(&self.file_path, &merge_entry)?;

        self.active_leaf_id = Some(new_leaf.clone());
        Ok((merged_count, new_leaf))
    }

    /// Cherry-pick a single message (and optionally its children) into a target branch.
    /// Returns the number of messages copied and the new leaf ID.
    pub fn cherry_pick(
        &mut self,
        message_id: MessageId,
        target_leaf: MessageId,
        with_children: bool,
    ) -> Result<(usize, MessageId)> {
        let tree = self.load_tree()?;

        // Verify both exist
        tree.find_message_public(&message_id).ok_or_else(|| crate::error::Error::Session {
            message: format!("Message not found: {}", message_id.0),
        })?;
        tree.find_message_public(&target_leaf).ok_or_else(|| crate::error::Error::Session {
            message: format!("Target branch leaf not found: {}", target_leaf.0),
        })?;

        // Collect messages to copy
        let messages_to_copy = if with_children {
            // DFS from message_id to collect it and all descendants
            let mut collected = Vec::new();
            Self::collect_subtree(&tree, &message_id, &mut collected);
            collected
        } else {
            let msg = tree.find_message_public(&message_id).expect("verified above");
            vec![msg.clone()]
        };

        if messages_to_copy.is_empty() {
            return Err(crate::error::Error::Session {
                message: "No messages to cherry-pick".into(),
            });
        }

        let count = messages_to_copy.len();

        // Copy with new IDs, maintaining relative parent structure
        // For with_children, we need to map old IDs → new IDs
        let mut id_map: HashMap<MessageId, MessageId> = HashMap::new();
        let mut new_leaf = target_leaf.clone();

        for msg in &messages_to_copy {
            let new_id = MessageId::generate();
            id_map.insert(msg.id.clone(), new_id.clone());

            // Determine parent: if this message's original parent was also copied,
            // use the new ID; otherwise chain from target_leaf
            let new_parent = if let Some(orig_parent) = &msg.parent_id
                && let Some(mapped) = id_map.get(orig_parent)
            {
                mapped.clone()
            } else {
                target_leaf.clone()
            };

            let mut cloned_message = msg.message.clone();
            set_message_id(&mut cloned_message, new_id.clone());

            let entry = SessionEntry::Message(MessageEntry {
                id: new_id.clone(),
                parent_id: Some(new_parent),
                message: cloned_message,
                timestamp: Utc::now(),
            });
            store::append_entry(&self.file_path, &entry)?;
            self.persisted_ids.insert(new_id.clone());
            new_leaf = new_id;
        }

        // Record cherry-pick metadata
        let cp_entry = SessionEntry::Custom(CustomEntry {
            id: MessageId::generate(),
            kind: "cherry-pick".to_string(),
            data: serde_json::json!({
                "source_message_id": message_id.0,
                "target_leaf": target_leaf.0,
                "with_children": with_children,
                "copied_count": count,
            }),
            timestamp: Utc::now(),
        });
        store::append_entry(&self.file_path, &cp_entry)?;

        self.active_leaf_id = Some(new_leaf.clone());
        Ok((count, new_leaf))
    }

    /// Collect a message and all its descendants via DFS
    fn collect_subtree(tree: &SessionTree, root_id: &MessageId, out: &mut Vec<MessageEntry>) {
        if let Some(msg) = tree.find_message_public(root_id) {
            out.push(msg.clone());
            let children = tree.get_children(&Some(root_id.clone()));
            for child in children {
                Self::collect_subtree(tree, &child.id, out);
            }
        }
    }
}
