//! Create worktree + branch for new session

use chrono::Utc;

use super::SessionType;
use super::WorktreeInfo;
use super::WorktreeManager;
use super::WorktreeStatus;
use crate::error::Result;
use crate::util::id::generate_id;

impl WorktreeManager {
    /// Create a new worktree for a session
    pub fn create_worktree(&self, session_type: &SessionType, parent_branch: Option<&str>) -> Result<WorktreeInfo> {
        let short_id = generate_id();
        let branch_name = match session_type {
            SessionType::Main => format!("clankers/main-{}", short_id),
            SessionType::Subagent { agent_name } => format!("clankers/sub/{}-{}", agent_name, short_id),
            SessionType::Worker { worker_name } => format!("clankers/worker/{}-{}", worker_name, short_id),
        };

        let parent = parent_branch.unwrap_or("HEAD");

        let worktree_path = self.repo_root.join(".git").join("clankers-worktrees").join(&branch_name);

        // Create branch from parent
        let output = std::process::Command::new("git")
            .args(["worktree", "add", "-b", &branch_name])
            .arg(&worktree_path)
            .arg(parent)
            .current_dir(&self.repo_root)
            .output()
            .map_err(|e| crate::error::Error::Worktree {
                message: format!("Failed to create worktree: {}", e),
            })?;

        if !output.status.success() {
            return Err(crate::error::Error::Worktree {
                message: format!("git worktree add failed: {}", String::from_utf8_lossy(&output.stderr)),
            });
        }

        let agent_name = match session_type {
            SessionType::Main => "main".to_string(),
            SessionType::Subagent { agent_name } => agent_name.clone(),
            SessionType::Worker { worker_name } => worker_name.clone(),
        };

        Ok(WorktreeInfo {
            branch: branch_name,
            path: worktree_path,
            session_id: short_id,
            agent: agent_name,
            status: WorktreeStatus::Active,
            created_at: Utc::now(),
            parent_branch: parent.to_string(),
        })
    }
}
