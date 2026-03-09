//! Remove worktree + branch after merge

use super::WorktreeManager;
use crate::error::Result;
use crate::tools::git_ops;

impl WorktreeManager {
    pub fn remove_worktree(&self, branch: &str) -> Result<()> {
        // Get worktree path from branch
        let worktree_path = self.repo_root.join(".git").join("clankers-worktrees").join(branch);

        // Remove worktree
        if !git_ops::sync::worktree_remove(&self.repo_root, &worktree_path) {
            return Err(crate::error::Error::Worktree {
                message: format!("Failed to remove worktree at {}", worktree_path.display()),
            });
        }

        // Delete branch if fully merged
        let _ = git_ops::sync::delete_branch(&self.repo_root, branch);

        Ok(())
    }
}
