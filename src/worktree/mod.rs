//! Git worktree isolation

pub mod cleanup;
pub mod conflict_graph;
pub mod create;
pub mod gc;
pub mod llm_resolver;
pub mod merge_daemon;
pub mod merge_strategy;
pub mod registry;
pub mod session_bridge;

use std::path::Path;
use std::path::PathBuf;

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeInfo {
    pub branch: String,
    pub path: PathBuf,
    pub session_id: String,
    pub agent: String,
    pub status: WorktreeStatus,
    pub created_at: DateTime<Utc>,
    pub parent_branch: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorktreeStatus {
    Active,
    Completed,
    Merging,
    Stale,
}

#[derive(Debug, Clone)]
pub enum SessionType {
    Main,
    Subagent { agent_name: String },
    Worker { worker_name: String },
}

pub struct WorktreeManager {
    repo_root: PathBuf,
}

impl WorktreeManager {
    pub fn new(repo_root: PathBuf) -> Self {
        Self { repo_root }
    }

    pub fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    /// Check if a directory is inside a git repo
    pub fn is_git_repo(path: &Path) -> bool {
        let output = std::process::Command::new("git").args(["rev-parse", "--git-dir"]).current_dir(path).output();
        matches!(output, Ok(o) if o.status.success())
    }

    /// Get the git repo root for a path
    pub fn find_repo_root(path: &Path) -> Option<PathBuf> {
        let output = std::process::Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(path)
            .output()
            .ok()?;
        if output.status.success() {
            let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Some(PathBuf::from(root))
        } else {
            None
        }
    }
}
