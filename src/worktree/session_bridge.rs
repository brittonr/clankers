//! Bridge between session lifecycle and git worktrees.
//!
//! When `use_worktrees` is enabled and the cwd is a git repo, new sessions
//! get their own worktree so each agent works in isolation. On session end
//! the worktree is marked completed for the merge daemon to pick up,
//! then GC runs to clean up merged branches.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use tracing::info;
use tracing::warn;

use super::DbWorktreeExt;
use super::SessionType;
use super::WorktreeManager;
use super::WorktreeStatus;
use crate::db::Db;
use crate::error::Result;
use crate::provider::Provider;

/// Result of setting up a worktree for a session.
#[derive(Debug, Clone)]
pub struct WorktreeSetup {
    /// The working directory to use (worktree path).
    pub working_dir: PathBuf,
    /// The branch name created for this worktree.
    pub branch: String,
    /// The original repo root (for cleanup later).
    pub repo_root: PathBuf,
}

/// Attempt to create a worktree for a new session.
///
/// Returns `Some(WorktreeSetup)` if a worktree was created, `None` if
/// worktrees are disabled, the cwd isn't a git repo, or creation failed
/// (in which case we log a warning and fall back to the normal cwd).
pub fn setup_worktree_for_session(db: &Db, cwd: &str, use_worktrees: bool) -> Option<WorktreeSetup> {
    if !use_worktrees {
        return None;
    }

    let cwd_path = Path::new(cwd);
    let repo_root = WorktreeManager::find_repo_root(cwd_path)?;
    let manager = WorktreeManager::new(repo_root.clone());

    let worktree_info = match manager.create_worktree(&SessionType::Main, None) {
        Ok(info) => info,
        Err(e) => {
            warn!("Failed to create worktree, falling back to cwd: {}", e);
            return None;
        }
    };

    // Register in redb
    if let Err(e) = db.worktrees().upsert(&worktree_info) {
        warn!("Failed to register worktree in db: {}", e);
        // Non-fatal — the worktree was still created
    }

    info!(
        branch = %worktree_info.branch,
        path = %worktree_info.path.display(),
        "Created worktree for session"
    );

    Some(WorktreeSetup {
        working_dir: worktree_info.path,
        branch: worktree_info.branch,
        repo_root,
    })
}

/// Re-enter an existing worktree for a resumed session.
///
/// Looks up the worktree by branch name from the session header. Returns the
/// worktree path if it still exists on disk, `None` otherwise.
pub fn resume_worktree(worktree_path: Option<&str>, worktree_branch: Option<&str>) -> Option<WorktreeSetup> {
    let wt_path_str = worktree_path?;
    let branch = worktree_branch?;
    let wt_path = PathBuf::from(wt_path_str);

    if !wt_path.exists() {
        warn!(
            path = %wt_path.display(),
            "Worktree path no longer exists, falling back to original cwd"
        );
        return None;
    }

    // Find repo root from the worktree
    let repo_root = WorktreeManager::find_repo_root(&wt_path)?;

    info!(
        branch = %branch,
        path = %wt_path.display(),
        "Resuming session in existing worktree"
    );

    Some(WorktreeSetup {
        working_dir: wt_path,
        branch: branch.to_string(),
        repo_root,
    })
}

/// Mark a worktree as completed so the merge daemon will pick it up.
///
/// Called when a session ends normally (user quits).
pub fn complete_worktree(db: &Db, setup: &WorktreeSetup) -> Result<()> {
    if db.worktrees().set_status(&setup.branch, WorktreeStatus::Completed)? {
        info!(branch = %setup.branch, "Marked worktree as completed");
    } else {
        warn!(branch = %setup.branch, "Worktree not found in registry");
    }
    Ok(())
}

/// Mark a worktree as completed, run background merge, then GC.
///
/// This is the primary entry point — marks the worktree completed then
/// kicks off a single merge cycle in the background, followed by GC to
/// clean up merged branches and worktree directories.
pub fn complete_and_merge(
    db: &Db,
    setup: &WorktreeSetup,
    provider: Option<Arc<dyn Provider>>,
    model: String,
) -> Result<tokio::task::JoinHandle<()>> {
    complete_worktree(db, setup)?;

    let db = db.clone();
    let repo_root = setup.repo_root.clone();
    let branch = setup.branch.clone();

    let handle = tokio::spawn(async move {
        // 1. Run merge cycle
        let daemon = match provider {
            Some(p) => super::merge_daemon::MergeDaemon::with_llm(repo_root.clone(), p, model),
            None => super::merge_daemon::MergeDaemon::new(repo_root.clone()),
        };
        match daemon.run_cycle(&db).await {
            Ok(0) => {}
            Ok(n) => info!(count = n, "background merge: merged branches"),
            Err(e) => warn!(error = %e, "background merge cycle failed"),
        }

        // 2. GC the branch that just completed + stale entries
        super::gc::gc_after_session(&db, &repo_root, &branch);
    });

    Ok(handle)
}

/// Change the process working directory to the worktree.
pub fn enter_worktree(setup: &WorktreeSetup) -> std::io::Result<()> {
    std::env::set_current_dir(&setup.working_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Db {
        Db::in_memory().expect("test: failed to create in-memory db")
    }

    #[test]
    fn test_setup_worktree_disabled() {
        let db = test_db();
        assert!(setup_worktree_for_session(&db, "/tmp", false).is_none());
    }

    #[test]
    fn test_setup_worktree_not_git_repo() {
        let db = test_db();
        let tmp = tempfile::TempDir::new().expect("test: failed to create temp dir");
        assert!(
            setup_worktree_for_session(&db, tmp.path().to_str().expect("test: failed to convert path to str"), true)
                .is_none()
        );
    }

    #[test]
    fn test_setup_worktree_in_git_repo() {
        let db = test_db();
        let tmp = tempfile::TempDir::new().expect("test: failed to create temp dir");
        let repo = tmp.path();

        // Initialize a git repo with an initial commit
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(repo)
            .output()
            .expect("test: git init failed");
        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(repo)
            .output()
            .expect("test: git config email failed");
        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(repo)
            .output()
            .expect("test: git config name failed");
        std::fs::write(repo.join("README.md"), "hello").expect("test: failed to write README");
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(repo)
            .output()
            .expect("test: git add failed");
        std::process::Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(repo)
            .output()
            .expect("test: git commit failed");

        let setup = setup_worktree_for_session(&db, repo.to_str().expect("test: failed to convert path to str"), true);
        assert!(setup.is_some());

        let setup = setup.expect("test: setup should succeed");
        assert!(setup.working_dir.exists());
        assert!(setup.branch.starts_with("clankers/main-"));
        assert_eq!(setup.repo_root, repo);

        // Registry should have the worktree in redb
        let info = db.worktrees().get(&setup.branch).expect("test: failed to get worktree from db");
        assert!(info.is_some());
        assert_eq!(info.expect("test: worktree should exist").status, WorktreeStatus::Active);

        // Cleanup
        let manager = WorktreeManager::new(repo.to_path_buf());
        manager.remove_worktree(&setup.branch).ok();
    }

    #[test]
    fn test_complete_worktree() {
        let db = test_db();
        let tmp = tempfile::TempDir::new().expect("test: failed to create temp dir");
        let repo = tmp.path();

        // Initialize git repo
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(repo)
            .output()
            .expect("test: git init failed");
        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(repo)
            .output()
            .expect("test: git config email failed");
        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(repo)
            .output()
            .expect("test: git config name failed");
        std::fs::write(repo.join("README.md"), "hello").expect("test: failed to write README");
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(repo)
            .output()
            .expect("test: git add failed");
        std::process::Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(repo)
            .output()
            .expect("test: git commit failed");

        let setup = setup_worktree_for_session(&db, repo.to_str().expect("test: failed to convert path to str"), true)
            .expect("test: setup should succeed");

        // Mark completed
        complete_worktree(&db, &setup).expect("test: failed to complete worktree");

        let info = db
            .worktrees()
            .get(&setup.branch)
            .expect("test: failed to get worktree")
            .expect("test: worktree should exist");
        assert_eq!(info.status, WorktreeStatus::Completed);

        // Cleanup
        let manager = WorktreeManager::new(repo.to_path_buf());
        manager.remove_worktree(&setup.branch).ok();
    }

    #[test]
    fn test_resume_worktree_missing_path() {
        assert!(resume_worktree(None, Some("branch")).is_none());
        assert!(resume_worktree(Some("/nonexistent"), Some("branch")).is_none());
    }

    #[test]
    fn test_resume_worktree_exists() {
        let db = test_db();
        let tmp = tempfile::TempDir::new().expect("test: failed to create temp dir");
        let repo = tmp.path();

        // Initialize git repo
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(repo)
            .output()
            .expect("test: git init failed");
        std::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(repo)
            .output()
            .expect("test: git config email failed");
        std::process::Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(repo)
            .output()
            .expect("test: git config name failed");
        std::fs::write(repo.join("README.md"), "hello").expect("test: failed to write README");
        std::process::Command::new("git")
            .args(["add", "."])
            .current_dir(repo)
            .output()
            .expect("test: git add failed");
        std::process::Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(repo)
            .output()
            .expect("test: git commit failed");

        let setup = setup_worktree_for_session(&db, repo.to_str().expect("test: failed to convert path to str"), true)
            .expect("test: setup should succeed");

        // Resume should find the worktree
        let resumed = resume_worktree(
            Some(setup.working_dir.to_str().expect("test: failed to convert path to str")),
            Some(&setup.branch),
        );
        assert!(resumed.is_some());
        let resumed = resumed.expect("test: resume should succeed");
        assert_eq!(resumed.working_dir, setup.working_dir);
        assert_eq!(resumed.branch, setup.branch);

        // Cleanup
        let manager = WorktreeManager::new(repo.to_path_buf());
        manager.remove_worktree(&setup.branch).ok();
    }
}
