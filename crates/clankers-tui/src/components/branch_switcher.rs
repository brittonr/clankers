//! Branch switcher — floating overlay for quick branch switching with fuzzy filter
//!
//! Triggered by a keyboard shortcut, renders as a centered floating popup.
//! Provides type-ahead filtering to quickly find and switch to a branch.
//!
//! Uses rat-branches NodeSwitcher for the underlying implementation.

use rat_branches::TreeNode;
use rat_branches::compare::truncate_first_line;
// Re-export rat-branches types for compatibility
pub use rat_branches::{NodeSwitcher, SwitcherItem};
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::components::block::ConversationBlock;

/// Wrapper around rat-branches NodeSwitcher for clankers-specific branch switching.
#[derive(Debug, Default)]
pub struct BranchSwitcher {
    inner: NodeSwitcher,
}

impl BranchSwitcher {
    pub fn new() -> Self {
        Self::default()
    }

    /// Open the switcher with branches from the block tree.
    pub fn open(&mut self, all_blocks: &[ConversationBlock], active_block_ids: &std::collections::HashSet<usize>) {
        self.inner.open(all_blocks, |leaf, path| {
            let tokens: usize =
                path.iter().filter_map(|&id| all_blocks.iter().find(|b| b.id == id)).map(|b| b.tokens).sum();

            // Generate branch name using path index
            let leaf_ids = rat_branches::tree::find_leaves(all_blocks);
            let branch_index = leaf_ids.iter().position(|&id| id == leaf.id()).unwrap_or(0) + 1;

            SwitcherItem::new(
                leaf.id(),
                format!("branch-{}", branch_index),
                truncate_first_line(&leaf.prompt, 50),
                active_block_ids.contains(&leaf.id()),
            )
            .add_metadata("msgs", path.len())
            .add_metadata("tok", tokens)
        });
    }

    /// Close the switcher
    pub fn close(&mut self) {
        self.inner.close();
    }

    /// Get the selected item's leaf block ID
    pub fn selected_leaf_id(&self) -> Option<usize> {
        self.inner.selected_node_id()
    }

    /// Render the switcher as a floating overlay
    pub fn render(&self, frame: &mut Frame, area: Rect) {
        self.inner.render(frame, area);
    }

    // Delegate input methods to inner NodeSwitcher
    pub fn move_up(&mut self) {
        self.inner.move_up();
    }

    pub fn move_down(&mut self) {
        self.inner.move_down();
    }

    pub fn type_char(&mut self, c: char) {
        self.inner.type_char(c);
    }

    pub fn backspace(&mut self) {
        self.inner.backspace();
    }

    // Provide access to properties for compatibility
    pub fn visible(&self) -> bool {
        self.inner.model.visible
    }

    pub fn filter(&self) -> &str {
        &self.inner.model.filter
    }
}
