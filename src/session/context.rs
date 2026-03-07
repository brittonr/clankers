//! Build LLM context from session tree

use super::tree::SessionTree;
use crate::provider::message::AgentMessage;
use crate::provider::message::MessageId;

/// Build context messages for a specific branch identified by its leaf message ID.
pub fn build_messages_for_branch(tree: &SessionTree, leaf_id: Option<&MessageId>) -> Vec<AgentMessage> {
    let leaf = match leaf_id {
        Some(id) => tree.find_message_public(id),
        None => {
            // Follow the latest branch all the way down
            tree.find_latest_leaf(None).or_else(|| tree.latest_message())
        }
    };
    let leaf = match leaf {
        Some(msg) => msg,
        None => return vec![],
    };
    let branch = tree.walk_branch(&leaf.id);
    branch.into_iter().map(|entry| entry.message.clone()).collect()
}
