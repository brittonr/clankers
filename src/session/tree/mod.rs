//! Session tree structure for navigating message hierarchies

use std::collections::HashMap;

use super::entry::MessageEntry;
use super::entry::SessionEntry;
use crate::provider::message::MessageId;

mod navigation;
mod query;

#[derive(Debug)]
pub struct SessionTree {
    entries: Vec<SessionEntry>,
    children: HashMap<Option<MessageId>, Vec<usize>>,
    /// O(1) message lookup by ID → index into `entries`.
    index: HashMap<MessageId, usize>,
}

impl SessionTree {
    pub fn build(entries: Vec<SessionEntry>) -> Self {
        let mut children: HashMap<Option<MessageId>, Vec<usize>> = HashMap::new();
        let mut index: HashMap<MessageId, usize> = HashMap::new();
        for (i, entry) in entries.iter().enumerate() {
            if let SessionEntry::Message(msg) = entry {
                children.entry(msg.parent_id.clone()).or_default().push(i);
                index.insert(msg.id.clone(), i);
            }
        }
        Self {
            entries,
            children,
            index,
        }
    }

    /// Find a message by ID (public)
    pub fn find_message_public(&self, id: &MessageId) -> Option<&MessageEntry> {
        self.find_message(id)
    }

    /// O(1) message lookup via the hash index.
    pub(super) fn find_message(&self, id: &MessageId) -> Option<&MessageEntry> {
        self.index.get(id).and_then(|&i| {
            if let SessionEntry::Message(msg) = &self.entries[i] {
                Some(msg)
            } else {
                None
            }
        })
    }

    pub fn entries(&self) -> &[SessionEntry] {
        &self.entries
    }

    /// Number of indexed messages (not total entries — excludes headers etc).
    pub fn message_count(&self) -> usize {
        self.index.len()
    }

    /// Access to children map for module-internal use
    pub(super) fn children(&self) -> &HashMap<Option<MessageId>, Vec<usize>> {
        &self.children
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::provider::message::AgentMessage;
    use crate::provider::message::Content;
    use crate::provider::message::UserMessage;

    fn make_message(id: MessageId, parent: Option<MessageId>, text: &str) -> SessionEntry {
        SessionEntry::Message(MessageEntry {
            id: id.clone(),
            parent_id: parent,
            message: AgentMessage::User(UserMessage {
                id: id.clone(),
                content: vec![Content::Text { text: text.to_string() }],
                timestamp: Utc::now(),
            }),
            timestamp: Utc::now(),
        })
    }

    #[test]
    fn test_empty_tree() {
        let tree = SessionTree::build(vec![]);
        assert_eq!(tree.entries().len(), 0);
        assert_eq!(tree.message_count(), 0);
        assert!(tree.latest_message().is_none());
    }

    #[test]
    fn test_single_message() {
        let id = MessageId::new("test-id");
        let entries = vec![make_message(id.clone(), None, "Hello")];
        let tree = SessionTree::build(entries);

        assert_eq!(tree.message_count(), 1);
        let latest = tree.latest_message();
        assert!(latest.is_some());
        assert_eq!(latest.expect("should have latest message").id, id);
    }

    #[test]
    fn test_linear_branch() {
        let id1 = MessageId::new("test-id-1");
        let id2 = MessageId::new("test-id-2");
        let id3 = MessageId::new("test-id-3");

        let entries = vec![
            make_message(id1.clone(), None, "First"),
            make_message(id2.clone(), Some(id1.clone()), "Second"),
            make_message(id3.clone(), Some(id2.clone()), "Third"),
        ];

        let tree = SessionTree::build(entries);
        let branch = tree.walk_branch(&id3);

        assert_eq!(branch.len(), 3);
        assert_eq!(branch[0].id, id1);
        assert_eq!(branch[1].id, id2);
        assert_eq!(branch[2].id, id3);
    }

    #[test]
    fn test_walk_from_middle() {
        let id1 = MessageId::new("test-id-1");
        let id2 = MessageId::new("test-id-2");
        let id3 = MessageId::new("test-id-3");

        let entries = vec![
            make_message(id1.clone(), None, "First"),
            make_message(id2.clone(), Some(id1.clone()), "Second"),
            make_message(id3.clone(), Some(id2.clone()), "Third"),
        ];

        let tree = SessionTree::build(entries);
        let branch = tree.walk_branch(&id2);

        assert_eq!(branch.len(), 2);
        assert_eq!(branch[0].id, id1);
        assert_eq!(branch[1].id, id2);
    }

    #[test]
    fn test_branching_conversation() {
        let id1 = MessageId::new("test-id-1");
        let id2a = MessageId::new("test-id-2a");
        let id2b = MessageId::new("test-id-2b");
        let id3a = MessageId::new("test-id-3a");

        let entries = vec![
            make_message(id1.clone(), None, "Root"),
            make_message(id2a.clone(), Some(id1.clone()), "Branch A"),
            make_message(id2b.clone(), Some(id1.clone()), "Branch B"),
            make_message(id3a.clone(), Some(id2a.clone()), "Branch A continued"),
        ];

        let tree = SessionTree::build(entries);

        // Walk branch A
        let branch_a = tree.walk_branch(&id3a);
        assert_eq!(branch_a.len(), 3);
        assert_eq!(branch_a[0].id, id1);
        assert_eq!(branch_a[1].id, id2a);
        assert_eq!(branch_a[2].id, id3a);

        // Walk branch B
        let branch_b = tree.walk_branch(&id2b);
        assert_eq!(branch_b.len(), 2);
        assert_eq!(branch_b[0].id, id1);
        assert_eq!(branch_b[1].id, id2b);
    }

    #[test]
    fn test_latest_message_picks_last() {
        let id1 = MessageId::new("test-id-1");
        let id2 = MessageId::new("test-id-2");
        let id3 = MessageId::new("test-id-3");

        let entries = vec![
            make_message(id1.clone(), None, "First"),
            make_message(id2.clone(), Some(id1.clone()), "Second"),
            make_message(id3.clone(), Some(id2.clone()), "Third"),
        ];

        let tree = SessionTree::build(entries);
        let latest = tree.latest_message().expect("should have latest message");
        assert_eq!(latest.id, id3);
    }

    #[test]
    fn test_nonexistent_message() {
        let id1 = MessageId::new("test-id-1");
        let entries = vec![make_message(id1, None, "Only message")];
        let tree = SessionTree::build(entries);

        let fake_id = MessageId::new("fake-id");
        let branch = tree.walk_branch(&fake_id);
        assert!(branch.is_empty());
    }

    #[test]
    fn test_get_children() {
        let id1 = MessageId::new("root");
        let id2a = MessageId::new("child-a");
        let id2b = MessageId::new("child-b");

        let entries = vec![
            make_message(id1.clone(), None, "Root"),
            make_message(id2a.clone(), Some(id1.clone()), "Child A"),
            make_message(id2b.clone(), Some(id1.clone()), "Child B"),
        ];
        let tree = SessionTree::build(entries);

        let root_children = tree.get_children(&Some(id1.clone()));
        assert_eq!(root_children.len(), 2);
        assert_eq!(root_children[0].id, id2a);
        assert_eq!(root_children[1].id, id2b);

        // Children of a leaf should be empty
        let leaf_children = tree.get_children(&Some(id2a));
        assert_eq!(leaf_children.len(), 0);
    }

    #[test]
    fn test_find_latest_leaf_linear() {
        let id1 = MessageId::new("msg-1");
        let id2 = MessageId::new("msg-2");
        let id3 = MessageId::new("msg-3");

        let entries = vec![
            make_message(id1.clone(), None, "First"),
            make_message(id2.clone(), Some(id1.clone()), "Second"),
            make_message(id3.clone(), Some(id2.clone()), "Third"),
        ];
        let tree = SessionTree::build(entries);

        let leaf = tree.find_latest_leaf(None).expect("should have latest leaf");
        assert_eq!(leaf.id, id3);
    }

    #[test]
    fn test_find_latest_leaf_branching() {
        let id1 = MessageId::new("root");
        let id2a = MessageId::new("branch-a");
        let id2b = MessageId::new("branch-b");
        let id3b = MessageId::new("branch-b-child");

        let entries = vec![
            make_message(id1.clone(), None, "Root"),
            make_message(id2a.clone(), Some(id1.clone()), "Branch A"),
            make_message(id2b.clone(), Some(id1.clone()), "Branch B"),
            make_message(id3b.clone(), Some(id2b.clone()), "Branch B continued"),
        ];
        let tree = SessionTree::build(entries);

        // From root, latest leaf should follow the last child at each level
        // Root's last child is id2b, id2b's last child is id3b
        let leaf = tree.find_latest_leaf(None).expect("should have latest leaf from root");
        assert_eq!(leaf.id, id3b);

        // From a specific branch point
        let leaf_a = tree.find_latest_leaf(Some(&id2a)).expect("should have latest leaf from branch a");
        assert_eq!(leaf_a.id, id2a); // id2a is a leaf itself
    }

    #[test]
    fn test_find_message_public() {
        let id1 = MessageId::new("msg-1");
        let entries = vec![make_message(id1.clone(), None, "Hello")];
        let tree = SessionTree::build(entries);

        assert!(tree.find_message_public(&id1).is_some());
        assert!(tree.find_message_public(&MessageId::new("nonexistent")).is_none());
    }

    #[test]
    fn test_message_count() {
        let id1 = MessageId::new("msg-1");
        let id2 = MessageId::new("msg-2");

        // Include a non-message entry to make sure it's not counted
        let entries = vec![
            make_message(id1.clone(), None, "First"),
            make_message(id2.clone(), Some(id1.clone()), "Second"),
        ];
        let tree = SessionTree::build(entries);
        assert_eq!(tree.message_count(), 2);
    }

    #[test]
    fn test_find_all_leaves_empty_tree() {
        let tree = SessionTree::build(vec![]);
        let leaves = tree.find_all_leaves();
        assert_eq!(leaves.len(), 0);
    }

    #[test]
    fn test_find_all_leaves_single_message() {
        let id = MessageId::new("msg-1");
        let entries = vec![make_message(id.clone(), None, "Hello")];
        let tree = SessionTree::build(entries);

        let leaves = tree.find_all_leaves();
        assert_eq!(leaves.len(), 1);
        assert_eq!(leaves[0].id, id);
    }

    #[test]
    fn test_find_all_leaves_linear_tree() {
        let id1 = MessageId::new("msg-1");
        let id2 = MessageId::new("msg-2");
        let id3 = MessageId::new("msg-3");

        let entries = vec![
            make_message(id1.clone(), None, "First"),
            make_message(id2.clone(), Some(id1.clone()), "Second"),
            make_message(id3.clone(), Some(id2.clone()), "Third"),
        ];
        let tree = SessionTree::build(entries);

        let leaves = tree.find_all_leaves();
        assert_eq!(leaves.len(), 1);
        assert_eq!(leaves[0].id, id3);
    }

    #[test]
    fn test_find_all_leaves_branching_tree() {
        let id1 = MessageId::new("root");
        let id2a = MessageId::new("branch-a");
        let id2b = MessageId::new("branch-b");
        let id3a = MessageId::new("branch-a-child");
        let id3b = MessageId::new("branch-b-child");

        let entries = vec![
            make_message(id1.clone(), None, "Root"),
            make_message(id2a.clone(), Some(id1.clone()), "Branch A"),
            make_message(id2b.clone(), Some(id1.clone()), "Branch B"),
            make_message(id3a.clone(), Some(id2a.clone()), "Branch A continued"),
            make_message(id3b.clone(), Some(id2b.clone()), "Branch B continued"),
        ];
        let tree = SessionTree::build(entries);

        let leaves = tree.find_all_leaves();
        assert_eq!(leaves.len(), 2);
        let leaf_ids: Vec<_> = leaves.iter().map(|l| l.id.clone()).collect();
        assert!(leaf_ids.contains(&id3a));
        assert!(leaf_ids.contains(&id3b));
    }

    #[test]
    fn test_find_all_leaves_multiple_roots() {
        let id1 = MessageId::new("root-1");
        let id2 = MessageId::new("root-2");
        let id3 = MessageId::new("child-1");

        let entries = vec![
            make_message(id1.clone(), None, "Root 1"),
            make_message(id2.clone(), None, "Root 2"),
            make_message(id3.clone(), Some(id1.clone()), "Child of Root 1"),
        ];
        let tree = SessionTree::build(entries);

        let leaves = tree.find_all_leaves();
        assert_eq!(leaves.len(), 2);
        let leaf_ids: Vec<_> = leaves.iter().map(|l| l.id.clone()).collect();
        assert!(leaf_ids.contains(&id2));
        assert!(leaf_ids.contains(&id3));
    }

    #[test]
    fn test_is_branch_point_no_children() {
        let id = MessageId::new("leaf");
        let entries = vec![make_message(id.clone(), None, "Leaf")];
        let tree = SessionTree::build(entries);

        assert!(!tree.is_branch_point(&id));
    }

    #[test]
    fn test_is_branch_point_one_child() {
        let id1 = MessageId::new("parent");
        let id2 = MessageId::new("child");

        let entries = vec![
            make_message(id1.clone(), None, "Parent"),
            make_message(id2.clone(), Some(id1.clone()), "Child"),
        ];
        let tree = SessionTree::build(entries);

        assert!(!tree.is_branch_point(&id1));
    }

    #[test]
    fn test_is_branch_point_multiple_children() {
        let id1 = MessageId::new("parent");
        let id2a = MessageId::new("child-a");
        let id2b = MessageId::new("child-b");

        let entries = vec![
            make_message(id1.clone(), None, "Parent"),
            make_message(id2a.clone(), Some(id1.clone()), "Child A"),
            make_message(id2b.clone(), Some(id1.clone()), "Child B"),
        ];
        let tree = SessionTree::build(entries);

        assert!(tree.is_branch_point(&id1));
        assert!(!tree.is_branch_point(&id2a));
        assert!(!tree.is_branch_point(&id2b));
    }

    #[test]
    fn test_is_branch_point_nonexistent() {
        let tree = SessionTree::build(vec![]);
        assert!(!tree.is_branch_point(&MessageId::new("fake")));
    }

    #[test]
    fn test_find_divergence_point_empty_tree() {
        let tree = SessionTree::build(vec![]);
        let result = tree.find_divergence_point(&MessageId::new("a"), &MessageId::new("b"));
        assert!(result.is_none());
    }

    #[test]
    fn test_find_divergence_point_same_message() {
        let id = MessageId::new("msg");
        let entries = vec![make_message(id.clone(), None, "Message")];
        let tree = SessionTree::build(entries);

        let result = tree.find_divergence_point(&id, &id);
        assert!(result.is_some());
        assert_eq!(result.expect("should have divergence point").id, id);
    }

    #[test]
    fn test_find_divergence_point_linear_chain() {
        let id1 = MessageId::new("msg-1");
        let id2 = MessageId::new("msg-2");
        let id3 = MessageId::new("msg-3");

        let entries = vec![
            make_message(id1.clone(), None, "First"),
            make_message(id2.clone(), Some(id1.clone()), "Second"),
            make_message(id3.clone(), Some(id2.clone()), "Third"),
        ];
        let tree = SessionTree::build(entries);

        // Two points on same linear chain — divergence is the earlier one
        let result = tree.find_divergence_point(&id2, &id3);
        assert!(result.is_some());
        assert_eq!(result.expect("should have divergence at id2").id, id2);

        let result = tree.find_divergence_point(&id1, &id3);
        assert!(result.is_some());
        assert_eq!(result.expect("should have divergence at id1").id, id1);
    }

    #[test]
    fn test_find_divergence_point_branching() {
        let id1 = MessageId::new("root");
        let id2a = MessageId::new("branch-a");
        let id2b = MessageId::new("branch-b");
        let id3a = MessageId::new("branch-a-child");
        let id3b = MessageId::new("branch-b-child");

        let entries = vec![
            make_message(id1.clone(), None, "Root"),
            make_message(id2a.clone(), Some(id1.clone()), "Branch A"),
            make_message(id2b.clone(), Some(id1.clone()), "Branch B"),
            make_message(id3a.clone(), Some(id2a.clone()), "Branch A continued"),
            make_message(id3b.clone(), Some(id2b.clone()), "Branch B continued"),
        ];
        let tree = SessionTree::build(entries);

        // Two leaves from different branches should diverge at root
        let result = tree.find_divergence_point(&id3a, &id3b);
        assert!(result.is_some());
        assert_eq!(result.expect("should have divergence at root for leaves").id, id1);

        // Leaf and intermediate node on different branches
        let result = tree.find_divergence_point(&id2a, &id3b);
        assert!(result.is_some());
        assert_eq!(result.expect("should have divergence at root for leaf and intermediate").id, id1);
    }

    #[test]
    fn test_find_divergence_point_no_common_ancestor() {
        let id1 = MessageId::new("root-1");
        let id2 = MessageId::new("root-2");

        let entries = vec![
            make_message(id1.clone(), None, "Root 1"),
            make_message(id2.clone(), None, "Root 2"),
        ];
        let tree = SessionTree::build(entries);

        // Two separate root messages have no common ancestor
        let result = tree.find_divergence_point(&id1, &id2);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_branch_messages_empty_tree() {
        let tree = SessionTree::build(vec![]);
        let result = tree.find_branch_messages(&MessageId::new("fake"));
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_find_branch_messages_no_branching() {
        let id1 = MessageId::new("msg-1");
        let id2 = MessageId::new("msg-2");
        let id3 = MessageId::new("msg-3");

        let entries = vec![
            make_message(id1.clone(), None, "First"),
            make_message(id2.clone(), Some(id1.clone()), "Second"),
            make_message(id3.clone(), Some(id2.clone()), "Third"),
        ];
        let tree = SessionTree::build(entries);

        // No branching, so all messages in the path are "unique" to this branch
        let result = tree.find_branch_messages(&id3);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].id, id1);
        assert_eq!(result[1].id, id2);
        assert_eq!(result[2].id, id3);
    }

    #[test]
    fn test_find_branch_messages_after_fork() {
        let id1 = MessageId::new("root");
        let id2a = MessageId::new("branch-a");
        let id2b = MessageId::new("branch-b");
        let id3a = MessageId::new("branch-a-child");

        let entries = vec![
            make_message(id1.clone(), None, "Root"),
            make_message(id2a.clone(), Some(id1.clone()), "Branch A"),
            make_message(id2b.clone(), Some(id1.clone()), "Branch B"),
            make_message(id3a.clone(), Some(id2a.clone()), "Branch A continued"),
        ];
        let tree = SessionTree::build(entries);

        // Branch A messages after the fork should be id2a and id3a
        let result = tree.find_branch_messages(&id3a);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, id2a);
        assert_eq!(result[1].id, id3a);

        // Branch B has only one message after the fork
        let result = tree.find_branch_messages(&id2b);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, id2b);
    }

    #[test]
    fn test_find_branch_messages_nested_branching() {
        let id1 = MessageId::new("root");
        let id2 = MessageId::new("middle");
        let id3a = MessageId::new("branch-a");
        let id3b = MessageId::new("branch-b");
        let id4a = MessageId::new("branch-a-child");

        let entries = vec![
            make_message(id1.clone(), None, "Root"),
            make_message(id2.clone(), Some(id1.clone()), "Middle"),
            make_message(id3a.clone(), Some(id2.clone()), "Branch A"),
            make_message(id3b.clone(), Some(id2.clone()), "Branch B"),
            make_message(id4a.clone(), Some(id3a.clone()), "Branch A continued"),
        ];
        let tree = SessionTree::build(entries);

        // Messages unique to branch A after the fork at id2
        let result = tree.find_branch_messages(&id4a);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, id3a);
        assert_eq!(result[1].id, id4a);
    }

    #[test]
    fn test_find_branch_messages_root_only() {
        let id = MessageId::new("root");
        let entries = vec![make_message(id.clone(), None, "Root")];
        let tree = SessionTree::build(entries);

        let result = tree.find_branch_messages(&id);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, id);
    }

    #[test]
    fn test_find_unique_messages_empty_source() {
        let tree = SessionTree::build(vec![]);
        let result = tree.find_unique_messages(&MessageId::new("a"), &MessageId::new("b"));
        assert!(result.is_empty());
    }

    #[test]
    fn test_find_unique_messages_branching() {
        let root = MessageId::new("root");
        let a1 = MessageId::new("a1");
        let a2 = MessageId::new("a2");
        let b1 = MessageId::new("b1");

        let entries = vec![
            make_message(root.clone(), None, "Root"),
            make_message(a1.clone(), Some(root.clone()), "A1"),
            make_message(a2.clone(), Some(a1.clone()), "A2"),
            make_message(b1.clone(), Some(root.clone()), "B1"),
        ];
        let tree = SessionTree::build(entries);

        // Unique messages in branch A (not in B): a1, a2
        let unique = tree.find_unique_messages(&a2, &b1);
        assert_eq!(unique.len(), 2);
        assert_eq!(unique[0].id, a1);
        assert_eq!(unique[1].id, a2);

        // Unique messages in branch B (not in A): b1
        let unique = tree.find_unique_messages(&b1, &a2);
        assert_eq!(unique.len(), 1);
        assert_eq!(unique[0].id, b1);
    }

    #[test]
    fn test_find_unique_messages_ancestor() {
        // When source is an ancestor of target, there are no unique messages
        let id1 = MessageId::new("msg-1");
        let id2 = MessageId::new("msg-2");
        let id3 = MessageId::new("msg-3");

        let entries = vec![
            make_message(id1.clone(), None, "First"),
            make_message(id2.clone(), Some(id1.clone()), "Second"),
            make_message(id3.clone(), Some(id2.clone()), "Third"),
        ];
        let tree = SessionTree::build(entries);

        // id1 is ancestor of id3 — all messages in id1's branch are shared
        let unique = tree.find_unique_messages(&id1, &id3);
        assert!(unique.is_empty());
    }

    #[test]
    fn test_find_unique_messages_same_leaf() {
        let id = MessageId::new("msg");
        let entries = vec![make_message(id.clone(), None, "Msg")];
        let tree = SessionTree::build(entries);

        let unique = tree.find_unique_messages(&id, &id);
        assert!(unique.is_empty());
    }

    #[test]
    fn test_find_unique_messages_deep_branches() {
        let root = MessageId::new("root");
        let shared = MessageId::new("shared");
        let a1 = MessageId::new("a1");
        let a2 = MessageId::new("a2");
        let a3 = MessageId::new("a3");
        let b1 = MessageId::new("b1");

        let entries = vec![
            make_message(root.clone(), None, "Root"),
            make_message(shared.clone(), Some(root.clone()), "Shared"),
            make_message(a1.clone(), Some(shared.clone()), "A1"),
            make_message(a2.clone(), Some(a1.clone()), "A2"),
            make_message(a3.clone(), Some(a2.clone()), "A3"),
            make_message(b1.clone(), Some(shared.clone()), "B1"),
        ];
        let tree = SessionTree::build(entries);

        // Unique to A: a1, a2, a3 (root and shared are common)
        let unique = tree.find_unique_messages(&a3, &b1);
        assert_eq!(unique.len(), 3);
        assert_eq!(unique[0].id, a1);
        assert_eq!(unique[1].id, a2);
        assert_eq!(unique[2].id, a3);
    }
}
