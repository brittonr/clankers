//! Block navigation and branching — focus, collapse, copy, and branch switching.

use super::{App, BlockEntry, MessageRole};

impl App {
    /// Focus the previous block
    pub fn focus_prev_block(&mut self) {
        let conv_ids: Vec<usize> = self.conversation_block_ids();
        if conv_ids.is_empty() {
            return;
        }
        match self.conversation.focused_block {
            None => {
                self.conversation.focused_block = conv_ids.last().copied();
            }
            Some(current) => {
                if let Some(pos) = conv_ids.iter().position(|&id| id == current) {
                    if pos > 0 {
                        self.conversation.focused_block = Some(conv_ids[pos - 1]);
                    }
                    // At the first block — stay put
                } else {
                    self.conversation.focused_block = conv_ids.last().copied();
                }
            }
        }
        self.conversation.scroll.auto_scroll = false;
    }

    /// Focus the next block
    pub fn focus_next_block(&mut self) {
        let conv_ids: Vec<usize> = self.conversation_block_ids();
        if conv_ids.is_empty() {
            return;
        }
        match self.conversation.focused_block {
            None => {
                // Start from the bottom (most recent block) since the user
                // is already scrolled to the bottom when unfocused.
                self.conversation.focused_block = conv_ids.last().copied();
                self.conversation.scroll.auto_scroll = false;
            }
            Some(current) => {
                if let Some(pos) = conv_ids.iter().position(|&id| id == current) {
                    if pos + 1 < conv_ids.len() {
                        self.conversation.focused_block = Some(conv_ids[pos + 1]);
                    } else {
                        // Past the last block — unfocus and return to auto-scroll
                        self.conversation.focused_block = None;
                        self.conversation.scroll.scroll_to_bottom();
                    }
                } else {
                    self.conversation.focused_block = conv_ids.last().copied();
                    self.conversation.scroll.auto_scroll = false;
                }
            }
        }
    }

    /// Toggle collapse on the focused block
    pub fn toggle_focused_block(&mut self) {
        if let Some(id) = self.conversation.focused_block {
            for entry in &mut self.conversation.blocks {
                if let BlockEntry::Conversation(block) = entry
                    && block.id == id
                {
                    block.toggle_collapse();
                    return;
                }
            }
        }
    }

    /// Collapse all conversation blocks
    pub fn collapse_all_blocks(&mut self) {
        for entry in &mut self.conversation.blocks {
            if let BlockEntry::Conversation(block) = entry {
                block.collapsed = true;
            }
        }
    }

    /// Expand all conversation blocks
    pub fn expand_all_blocks(&mut self) {
        for entry in &mut self.conversation.blocks {
            if let BlockEntry::Conversation(block) = entry {
                block.collapsed = false;
            }
        }
    }

    /// Copy the focused block's content to the clipboard
    pub fn copy_focused_block(&self) {
        if let Some(id) = self.conversation.focused_block {
            for entry in &self.conversation.blocks {
                if let BlockEntry::Conversation(block) = entry
                    && block.id == id
                {
                    let mut text = String::new();
                    for msg in &block.responses {
                        if msg.role == MessageRole::Assistant {
                            if !text.is_empty() {
                                text.push('\n');
                            }
                            text.push_str(&msg.content);
                        }
                    }
                    if !text.is_empty() {
                        crate::tui::selection::copy_to_clipboard(&text);
                    }
                    return;
                }
            }
        }
    }

    /// Get the prompt from the focused block (for re-running)
    pub fn get_focused_block_prompt(&self) -> Option<String> {
        let id = self.conversation.focused_block?;
        for entry in &self.conversation.blocks {
            if let BlockEntry::Conversation(block) = entry
                && block.id == id
            {
                return Some(block.prompt.clone());
            }
        }
        None
    }

    /// Get IDs of all conversation blocks in order
    fn conversation_block_ids(&self) -> Vec<usize> {
        self.conversation.blocks
            .iter()
            .filter_map(|entry| match entry {
                BlockEntry::Conversation(block) => Some(block.id),
                _ => None,
            })
            .collect()
    }

    // ── Branching ────────────────────────────────────

    /// Get the sibling info for a block: (current_index, total_siblings)
    /// Siblings are blocks that share the same parent_block_id.
    pub fn block_siblings(&self, block_id: usize) -> (usize, usize) {
        let block = match self.conversation.all_blocks.iter().find(|b| b.id == block_id) {
            Some(b) => b,
            None => return (0, 1),
        };
        let parent = block.parent_block_id;
        let siblings: Vec<usize> =
            self.conversation.all_blocks.iter().filter(|b| b.parent_block_id == parent).map(|b| b.id).collect();
        let idx = siblings.iter().position(|&id| id == block_id).unwrap_or(0);
        (idx, siblings.len())
    }

    /// Count how many child blocks branch from the given block.
    /// Returns 0 for leaf blocks, >1 means this block is a branch point.
    pub fn block_children_count(&self, block_id: usize) -> usize {
        self.conversation.all_blocks.iter().filter(|b| b.parent_block_id == Some(block_id)).count()
    }

    /// Edit the focused block's prompt: pre-fill the editor and set up a
    /// pending branch operation. Returns true if a branch edit was initiated.
    pub fn edit_focused_block_prompt(&mut self) -> bool {
        let id = match self.conversation.focused_block {
            Some(id) => id,
            None => return false,
        };
        let block = match self.conversation.all_blocks.iter().find(|b| b.id == id) {
            Some(b) => b.clone(),
            None => return false,
        };
        // Pre-fill the editor with the prompt text
        self.editor.clear();
        for c in block.prompt.chars() {
            self.editor.insert_char(c);
        }
        // Store the pending branch info: we'll branch from this block's parent
        // using this block's agent_msg_checkpoint (the message count before it)
        self.branching.pending_branch = Some((id, String::new())); // prompt will be filled on submit
        self.conversation.focused_block = None;
        true
    }

    /// If there's a pending branch, finalize it with the submitted prompt.
    /// Returns Some((checkpoint, prompt)) to tell the event loop to truncate and re-prompt.
    pub fn take_pending_branch(&mut self, submitted_prompt: &str) -> Option<(usize, String)> {
        let (fork_block_id, _) = self.branching.pending_branch.take()?;
        let fork_block = self.conversation.all_blocks.iter().find(|b| b.id == fork_block_id)?;
        let checkpoint = fork_block.agent_msg_checkpoint;
        // Remove all blocks from the visible list that come at or after the fork point.
        let mut keep_up_to = self.conversation.blocks.len();
        for (i, entry) in self.conversation.blocks.iter().enumerate() {
            if let BlockEntry::Conversation(b) = entry
                && b.id == fork_block_id
            {
                keep_up_to = i;
                break;
            }
        }
        self.conversation.blocks.truncate(keep_up_to);

        // Signal the event loop to record a branch in the session file
        self.branching.last_branch_checkpoint = Some(checkpoint);

        Some((checkpoint, submitted_prompt.to_string()))
    }

    /// Navigate to the previous sibling branch at the focused block
    pub fn branch_prev(&mut self) {
        if let Some(id) = self.conversation.focused_block
            && let Some(sibling_id) = self.adjacent_sibling(id, -1)
        {
            self.switch_to_branch(sibling_id);
        }
    }

    /// Navigate to the next sibling branch at the focused block
    pub fn branch_next(&mut self) {
        if let Some(id) = self.conversation.focused_block
            && let Some(sibling_id) = self.adjacent_sibling(id, 1)
        {
            self.switch_to_branch(sibling_id);
        }
    }

    /// Find the sibling block offset positions from the given block.
    fn adjacent_sibling(&self, block_id: usize, offset: isize) -> Option<usize> {
        let block = self.conversation.all_blocks.iter().find(|b| b.id == block_id)?;
        let parent = block.parent_block_id;
        let siblings: Vec<usize> =
            self.conversation.all_blocks.iter().filter(|b| b.parent_block_id == parent).map(|b| b.id).collect();
        let idx = siblings.iter().position(|&id| id == block_id)? as isize;
        let new_idx = idx + offset;
        if new_idx >= 0 && (new_idx as usize) < siblings.len() {
            Some(siblings[new_idx as usize])
        } else {
            None
        }
    }

    /// Switch the visible block list to show the branch containing `target_block_id`.
    /// Rebuilds `self.blocks` to show the path from root through target and all its descendants.
    pub fn switch_to_branch(&mut self, target_block_id: usize) {
        // Walk up from target to root to find the full ancestor path
        let mut path_up = Vec::new();
        let mut current = Some(target_block_id);
        while let Some(id) = current {
            path_up.push(id);
            current = self.conversation.all_blocks.iter().find(|b| b.id == id).and_then(|b| b.parent_block_id);
        }
        path_up.reverse(); // now root → ... → target

        // Walk down from target following the latest child at each level
        let mut path = path_up;
        let mut leaf = target_block_id;
        loop {
            // Find children of leaf (blocks whose parent_block_id == Some(leaf))
            let children: Vec<usize> =
                self.conversation.all_blocks.iter().filter(|b| b.parent_block_id == Some(leaf)).map(|b| b.id).collect();
            if let Some(&last_child) = children.last() {
                path.push(last_child);
                leaf = last_child;
            } else {
                break;
            }
        }

        // Rebuild self.blocks: keep system messages at their positions,
        // replace conversation blocks with the path
        let system_msgs: Vec<BlockEntry> =
            self.conversation.blocks.iter().filter(|e| matches!(e, BlockEntry::System(_))).cloned().collect();

        self.conversation.blocks.clear();
        // Re-add system messages that were before the first conversation block
        // For simplicity, put system messages first, then the branch path
        for sys in system_msgs {
            self.conversation.blocks.push(sys);
        }
        for &block_id in &path {
            if let Some(block) = self.conversation.all_blocks.iter().find(|b| b.id == block_id) {
                self.conversation.blocks.push(BlockEntry::Conversation(block.clone()));
            }
        }

        self.conversation.focused_block = Some(target_block_id);
        self.conversation.scroll.auto_scroll = false;
    }
}
