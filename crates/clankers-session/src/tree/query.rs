//! Tree query and analysis methods

use clankers_message::MessageId;

use super::SessionTree;
use crate::entry::MessageEntry;

impl SessionTree {
    /// Returns true if the message has more than one child (is a branch point).
    pub fn is_branch_point(&self, message_id: &MessageId) -> bool {
        self.get_children(&Some(message_id.clone())).len() > 1
    }

    /// Find the last common ancestor (divergence point) of two branches.
    pub fn find_divergence_point(&self, leaf_a: &MessageId, leaf_b: &MessageId) -> Option<&MessageEntry> {
        let branch_a = self.walk_branch(leaf_a);
        let branch_b = self.walk_branch(leaf_b);

        if branch_a.is_empty() || branch_b.is_empty() {
            return None;
        }

        let mut last_common: Option<&MessageEntry> = None;
        let min_len = branch_a.len().min(branch_b.len());

        for i in 0..min_len {
            if branch_a[i].id == branch_b[i].id {
                last_common = Some(branch_a[i]);
            } else {
                break;
            }
        }

        last_common
    }

    /// Find messages unique to `source_leaf` that are NOT in the `target_leaf` branch.
    pub fn find_unique_messages(&self, source_leaf: &MessageId, target_leaf: &MessageId) -> Vec<&MessageEntry> {
        let source_path = self.walk_branch(source_leaf);
        let target_path = self.walk_branch(target_leaf);

        if source_path.is_empty() {
            return vec![];
        }

        let target_ids: std::collections::HashSet<&MessageId> = target_path.iter().map(|m| &m.id).collect();
        source_path.into_iter().filter(|m| !target_ids.contains(&m.id)).collect()
    }

    /// Find messages unique to a branch (after divergence from nearest sibling).
    pub fn find_branch_messages(&self, leaf_id: &MessageId) -> Vec<&MessageEntry> {
        let branch = self.walk_branch(leaf_id);
        if branch.is_empty() {
            return vec![];
        }

        let mut divergence_idx = 0;

        for (i, msg) in branch.iter().enumerate() {
            if let Some(parent_id) = &msg.parent_id
                && self.is_branch_point(parent_id)
            {
                divergence_idx = i;
                break;
            }
        }

        branch[divergence_idx..].to_vec()
    }
}
