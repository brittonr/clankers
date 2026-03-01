//! Remove worktree + branch after merge

use super::WorktreeManager;
use crate::error::Result;

impl WorktreeManager {
    pub fn remove_worktree(&self, branch: &str) -> Result<()> {
        // Get worktree path from branch
        let worktree_path = self.repo_root.join(".git").join("clankers-worktrees").join(branch);

        // Remove worktree
        let output = std::process::Command::new("git")
            .args(["worktree", "remove", "--force"])
            .arg(&worktree_path)
            .current_dir(&self.repo_root)
            .output()
            .map_err(|e| crate::error::Error::Worktree { message: e.to_string() })?;

        if !output.status.success() {
            return Err(crate::error::Error::Worktree {
                message: format!("git worktree remove failed: {}", String::from_utf8_lossy(&output.stderr)),
            });
        }

        // Delete branch if fully merged
        let _ = std::process::Command::new("git")
            .args(["branch", "-d", branch])
            .current_dir(&self.repo_root)
            .output();

        Ok(())
    }
}
