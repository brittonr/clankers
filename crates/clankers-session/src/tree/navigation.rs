//! Tree navigation and traversal methods
//!
//! # Tiger Style
//!
//! All traversals enforce explicit depth limits to prevent unbounded
//! iteration in the presence of corrupted session data (e.g., cycles
//! introduced by a bad merge). The hard limit matches the maximum
//! conversation depth a session can realistically reach.

use clanker_message::MessageId;

use super::SessionTree;
use crate::entry::MessageEntry;
use crate::entry::SessionEntry;

/// Tiger Style: hard limit on tree traversal depth to prevent cycles
/// from causing unbounded iteration. A 50K-message conversation would
/// be ~100K API calls — well beyond any realistic session.
const MAX_TRAVERSAL_DEPTH: u32 = 50_000;

// Tiger Style: compile-time assertions on traversal limits.
const _: () = assert!(MAX_TRAVERSAL_DEPTH > 0);
const _: () = assert!(MAX_TRAVERSAL_DEPTH <= 100_000);

impl SessionTree {
    /// Walk from a leaf message to the root, collecting message entries.
    /// O(depth) thanks to the hash index.
    ///
    /// # Tiger Style
    ///
    /// Bounded by `MAX_TRAVERSAL_DEPTH` to prevent infinite loops if
    /// the tree contains a cycle (corrupted session data).
    // r[impl session.walk.path-valid]
    // r[impl session.walk.root-anchored]
    // r[impl session.walk.terminates]
    pub fn walk_branch(&self, leaf_id: &MessageId) -> Vec<&MessageEntry> {
        let mut path = Vec::new();
        let mut current_id = Some(leaf_id.clone());
        let mut steps: u32 = 0;
        while let Some(id) = current_id.as_ref() {
            debug_assert!(steps < MAX_TRAVERSAL_DEPTH, "walk_branch exceeded depth limit — possible cycle");
            if steps >= MAX_TRAVERSAL_DEPTH {
                eprintln!("ERROR: walk_branch hit depth limit ({}) — breaking to prevent hang", MAX_TRAVERSAL_DEPTH);
                break;
            }
            if let Some(entry) = self.find_message(id) {
                current_id.clone_from(&entry.parent_id);
                path.push(entry);
            } else {
                break;
            }
            steps += 1;
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
    ///
    /// # Tiger Style
    ///
    /// Bounded by `MAX_TRAVERSAL_DEPTH` to prevent infinite loops.
    #[cfg_attr(
        dylint_lib = "tigerstyle",
        allow(no_unwrap, reason = "children.last() guarded by is_empty check")
    )]
    #[cfg_attr(
        dylint_lib = "tigerstyle",
        allow(unbounded_loop, reason = "traversal loop; bounded by MAX_TRAVERSAL_DEPTH")
    )]
    pub fn find_latest_leaf(&self, start_id: Option<&MessageId>) -> Option<&MessageEntry> {
        let mut current_id = start_id.cloned();
        let mut last_message = start_id.and_then(|id| self.find_message(id));

        let mut depth: u32 = 0;
        loop {
            debug_assert!(depth < MAX_TRAVERSAL_DEPTH, "find_latest_leaf exceeded depth limit");
            if depth >= MAX_TRAVERSAL_DEPTH {
                eprintln!("ERROR: find_latest_leaf hit depth limit ({})", MAX_TRAVERSAL_DEPTH);
                break;
            }
            let children = self.get_children(&current_id);
            if children.is_empty() {
                break;
            }
            let child = children.last().expect("non-empty checked above");
            last_message = Some(child);
            current_id = Some(child.id.clone());
            depth += 1;
        }
        last_message
    }

    /// Find all leaf nodes (messages with no children) by doing a DFS from all roots.
    ///
    /// # Tiger Style
    ///
    /// Uses an explicit stack instead of recursion to avoid stack overflow
    /// on deep trees. Bounded by total entry count (each node visited once).
    pub fn find_all_leaves(&self) -> Vec<&MessageEntry> {
        let mut leaves = Vec::new();
        let mut visited = std::collections::HashSet::new();

        // Tiger Style: use iterative DFS with explicit stack, not recursion.
        let mut stack: Vec<&MessageEntry> = self.get_children(&None);
        stack.reverse(); // process in original order

        while let Some(node) = stack.pop() {
            if visited.contains(&node.id) {
                continue;
            }
            visited.insert(node.id.clone());

            let children = self.get_children(&Some(node.id.clone()));
            if children.is_empty() {
                leaves.push(node);
            } else {
                // Push children in reverse so they're processed in order.
                for child in children.iter().rev() {
                    if !visited.contains(&child.id) {
                        stack.push(child);
                    }
                }
            }

            // Tiger Style: safety valve — total visited nodes bounded by entry count.
            debug_assert!(visited.len() <= self.entries().len(), "DFS visited more nodes than entries exist");
        }

        leaves
    }
}
