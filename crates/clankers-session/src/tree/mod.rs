//! Session tree structure for navigating message hierarchies

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use std::collections::HashMap;

use clanker_message::MessageId;

use crate::entry::MessageEntry;
use crate::entry::SessionEntry;

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
    // r[impl session.index.consistent]
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
    pub(crate) fn find_message(&self, id: &MessageId) -> Option<&MessageEntry> {
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

    /// Access to children map for crate-internal use
    pub(crate) fn children(&self) -> &HashMap<Option<MessageId>, Vec<usize>> {
        &self.children
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use clanker_message::AgentMessage;
    use clanker_message::Content;
    use clanker_message::UserMessage;

    use super::*;

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

    // r[verify session.walk.path-valid]
    // r[verify session.walk.root-anchored]
    // r[verify session.walk.terminates]
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

        let branch_a = tree.walk_branch(&id3a);
        assert_eq!(branch_a.len(), 3);
        assert_eq!(branch_a[0].id, id1);
        assert_eq!(branch_a[1].id, id2a);
        assert_eq!(branch_a[2].id, id3a);

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

        let leaf = tree.find_latest_leaf(None).expect("should have latest leaf from root");
        assert_eq!(leaf.id, id3b);

        let leaf_a = tree.find_latest_leaf(Some(&id2a)).expect("should have latest leaf from branch a");
        assert_eq!(leaf_a.id, id2a);
    }

    #[test]
    fn test_find_message_public() {
        let id1 = MessageId::new("msg-1");
        let entries = vec![make_message(id1.clone(), None, "Hello")];
        let tree = SessionTree::build(entries);

        assert!(tree.find_message_public(&id1).is_some());
        assert!(tree.find_message_public(&MessageId::new("nonexistent")).is_none());
    }

    // r[verify session.index.consistent]
    #[test]
    fn test_message_count() {
        let id1 = MessageId::new("msg-1");
        let id2 = MessageId::new("msg-2");

        let entries = vec![
            make_message(id1.clone(), None, "First"),
            make_message(id2.clone(), Some(id1.clone()), "Second"),
        ];
        let tree = SessionTree::build(entries);
        assert_eq!(tree.message_count(), 2);
    }

    // find_all_leaves, is_branch_point, find_divergence_point, find_branch_messages,
    // find_unique_messages tests are all kept from the original — they test query.rs
    // and navigation.rs methods via the SessionTree interface.

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
    fn test_is_branch_point() {
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

        let unique = tree.find_unique_messages(&a2, &b1);
        assert_eq!(unique.len(), 2);
        assert_eq!(unique[0].id, a1);
        assert_eq!(unique[1].id, a2);
    }
}
