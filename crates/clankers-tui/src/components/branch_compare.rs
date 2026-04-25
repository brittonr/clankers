//! Branch comparison view — side-by-side diff of two conversation branches
//!
//! Shows the divergence point (last common ancestor) at the top, then
//! unique blocks from each branch in a split-pane layout. Provides
//! navigation and actions (switch to either branch).
//!
//! Uses rat-branches for generic tree algorithms and comparison structures.

// Re-export rat-branches types for compatibility
pub use rat_branches::BranchCompareView;
pub use rat_branches::BranchComparison;
pub use rat_branches::CompareBlock;
use rat_branches::compare::compare_branches as rb_compare_branches;
use rat_branches::compare::truncate_first_line;

use crate::components::block::ConversationBlock;

/// Compare two branches using rat-branches generic algorithm.
/// Wraps rat_branches::compare_branches with clankers-specific node conversion.
pub fn compare_branches(leaf_a: usize, leaf_b: usize, all_blocks: &[ConversationBlock]) -> Option<BranchComparison> {
    rb_compare_branches(leaf_a, leaf_b, all_blocks, block_to_compare)
}

/// Convert a ConversationBlock to a CompareBlock for display.
/// This function accesses clankers-specific fields (responses, MessageRole).
fn block_to_compare(b: &ConversationBlock) -> CompareBlock {
    use crate::app::MessageRole;
    CompareBlock::new(b.id, truncate_first_line(&b.prompt, 50), b.tokens)
        .add_detail_count("responses", b.responses.len())
        .add_detail_count("tools", b.responses.iter().filter(|m| m.role == MessageRole::ToolCall).count())
}

/// Clankers-specific extension for BranchCompareView.
pub trait BranchCompareViewExt {
    /// Open the comparison view with two branch leaf IDs (clankers-specific wrapper).
    fn open_with_blocks(&mut self, leaf_a: usize, leaf_b: usize, all_blocks: &[ConversationBlock]);
}

impl BranchCompareViewExt for BranchCompareView {
    fn open_with_blocks(&mut self, leaf_a: usize, leaf_b: usize, all_blocks: &[ConversationBlock]) {
        if let Some(comparison) = compare_branches(leaf_a, leaf_b, all_blocks) {
            self.open(comparison);
        }
    }
}
