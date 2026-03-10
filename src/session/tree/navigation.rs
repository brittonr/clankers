//! Tree navigation and traversal methods

use super::SessionTree;
use crate::provider::message::MessageId;
use crate::session::entry::MessageEntry;
use crate::session::entry::SessionEntry;

impl SessionTree {
    /// Walk from a leaf message to the root, collecting message entries.
    /// O(depth) thanks to the hash index.
    pub fn walk_branch(&self, leaf_id: &MessageId) -> Vec<&MessageEntry> {
        let mut path = Vec::new();
        let mut current_id = Some(leaf_id.clone());
        while let Some(id) = current_id.as_ref() {
            if let Some(entry) = self.find_message(id) {
                current_id.clone_from(&entry.parent_id);
                path.push(entry);
            } else {
                break;
            }
        }
        path.reverse();
        path
    }

    /// Get the most recent message entry (by insertion order)
    pub fn latest_message(&self) -> Option<&MessageEntry> {
        self.entries().iter().rev().find_map(|e| {
            if let SessionEntry::Message(msg) = e {
                Some(msg)
            } else {
                None
            }
        })
    }

    /// Get children of a given parent (direct descendants)
    pub fn get_children(&self, parent_id: &Option<MessageId>) -> Vec<&MessageEntry> {
        self.children()
            .get(parent_id)
            .map(|indices| {
                indices
                    .iter()
                    .filter_map(|&i| {
                        if let SessionEntry::Message(msg) = &self.entries()[i] {
                            Some(msg)
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Find the latest leaf by following the last child at each level from a given starting
    /// message. If `start_id` is None, starts from root messages.
    pub fn find_latest_leaf(&self, start_id: Option<&MessageId>) -> Option<&MessageEntry> {
        let mut current_id = start_id.cloned();
        let mut last_message = start_id.and_then(|id| self.find_message(id));

        loop {
            let children = self.get_children(&current_id);
            if children.is_empty() {
                break;
            }
            // Follow the last child (most recently added branch)
            // Safe: we just checked children.is_empty() above
            let child = children.last().expect("non-empty checked above");
            last_message = Some(child);
            current_id = Some(child.id.clone());
        }
        last_message
    }

    /// Find all leaf nodes (messages with no children) by doing a DFS from all roots.
    pub fn find_all_leaves(&self) -> Vec<&MessageEntry> {
        let mut leaves = Vec::new();
        let mut visited = std::collections::HashSet::new();

        // Start DFS from all root messages
        let roots = self.get_children(&None);
        for root in roots {
            self.dfs_collect_leaves(root, &mut leaves, &mut visited);
        }

        leaves
    }

    /// Helper for DFS traversal to collect leaf nodes
    fn dfs_collect_leaves<'a>(
        &'a self,
        node: &'a MessageEntry,
        leaves: &mut Vec<&'a MessageEntry>,
        visited: &mut std::collections::HashSet<MessageId>,
    ) {
        if visited.contains(&node.id) {
            return;
        }
        visited.insert(node.id.clone());

        let children = self.get_children(&Some(node.id.clone()));
        if children.is_empty() {
            // This is a leaf
            leaves.push(node);
        } else {
            // Recurse into children
            for child in children {
                self.dfs_collect_leaves(child, leaves, visited);
            }
        }
    }
}
