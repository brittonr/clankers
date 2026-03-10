//! Build LLM context from session tree

use clankers_message::AgentMessage;
use clankers_message::MessageId;

use super::tree::SessionTree;

/// Build context messages for a specific branch identified by its leaf message ID.
pub fn build_messages_for_branch(tree: &SessionTree, leaf_id: Option<&MessageId>) -> Vec<AgentMessage> {
    let leaf = match leaf_id {
        Some(id) => tree.find_message_public(id),
        None => tree.find_latest_leaf(None).or_else(|| tree.latest_message()),
    };
    let leaf = match leaf {
        Some(msg) => msg,
        None => return vec![],
    };
    let branch = tree.walk_branch(&leaf.id);
    branch.into_iter().map(|entry| entry.message.clone()).collect()
}
