//! Create worktree + branch for new session

use chrono::Utc;

use super::SessionType;
use super::WorktreeInfo;
use super::WorktreeManager;
use super::WorktreeStatus;
use crate::error::Result;
use crate::tools::git_ops;
use crate::util::id::generate_id;

impl WorktreeManager {
    /// Create a new worktree for a session
    pub fn create_worktree(&self, session_type: &SessionType, parent_branch: Option<&str>) -> Result<WorktreeInfo> {
        let short_id = generate_id();
        let branch_name = match session_type {
            SessionType::Main => format!("clankers/main-{}", short_id),
            SessionType::Subagent { agent_name } => {
                format!("clankers/sub/{}-{}", agent_name, short_id)
            }
            SessionType::Worker { worker_name } => {
                format!("clankers/worker/{}-{}", worker_name, short_id)
            }
        };

        let parent = parent_branch.unwrap_or("HEAD");

        let worktree_path = self.repo_root.join(".git").join("clankers-worktrees").join(&branch_name);

        git_ops::sync::worktree_add(&self.repo_root, &branch_name, &worktree_path, parent)
            .map_err(|e| crate::error::Error::Worktree { message: e.to_string() })?;

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
