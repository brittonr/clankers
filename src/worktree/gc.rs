//! Automatic worktree garbage collection.
//!
//! Runs on startup and after session completion to:
//!
//! 1. Remove worktree directories for branches that are fully merged.
//! 2. Delete fully-merged `clankers/*` git branches.
//! 3. Clean up legacy `pirs/*` worktrees and branches from the rename.
//! 4. Reconcile the redb registry against actual git state.
//! 5. Run `git gc` when enough garbage has accumulated.
//!
//! This prevents the 17 GB orphaned-worktree problem by design: every
//! session exit path calls `gc_after_session`, and every startup calls
//! `gc_on_startup`.
//!
//! Legacy cleanup: the project was renamed from `pirs` to `clankers`.
//! Any leftover `pirs/*` branches and `.git/pirs-worktrees/` directories
//! are cleaned up automatically on startup.
//!
//! All git operations are in-process via libgit2 (see `git_ops::sync`),
//! except `git gc` which requires the git CLI.

use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use tracing::debug;
use tracing::info;
use tracing::warn;

use crate::db::Db;
use crate::tools::git_ops;

/// Summary of what GC did.
#[derive(Debug, Default)]
pub struct GcReport {
    /// Worktree directories removed from disk.
    pub worktrees_removed: usize,
    /// Git branches deleted.
    pub branches_deleted: usize,
    /// Registry entries cleaned from redb.
    pub registry_pruned: usize,
    /// Whether `git gc` was run.
    pub git_gc_ran: bool,
    /// How long GC took.
    pub elapsed: Duration,
}

impl std::fmt::Display for GcReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_empty() {
            write!(f, "gc: nothing to clean")
        } else {
            write!(
                f,
                "gc: {} worktrees, {} branches, {} registry entries cleaned ({:.1}s{})",
                self.worktrees_removed,
                self.branches_deleted,
                self.registry_pruned,
                self.elapsed.as_secs_f64(),
                if self.git_gc_ran { ", ran git gc" } else { "" },
            )
        }
    }
}

impl GcReport {
    fn is_empty(&self) -> bool {
        self.worktrees_removed == 0 && self.branches_deleted == 0 && self.registry_pruned == 0
    }
}

// ── Public entry points ─────────────────────────────────────────────────

/// Lightweight GC after a session completes.
///
/// Only cleans up the single branch that just finished plus any stale
/// entries discovered in the registry. Fast enough to run synchronously
/// before the process exits.
pub fn gc_after_session(db: &Db, repo_root: &Path, branch: &str) -> GcReport {
    let start = Instant::now();
    let mut report = GcReport::default();

    // 1. Remove the specific worktree + branch that just completed
    if remove_worktree(repo_root, branch) {
        report.worktrees_removed += 1;
    }
    if git_ops::sync::delete_branch(repo_root, branch) {
        report.branches_deleted += 1;
    }
    if let Ok(true) = db.worktrees().remove(branch) {
        report.registry_pruned += 1;
    }

    // 2. Quick sweep: prune registry entries whose branches no longer exist
    report.registry_pruned += prune_stale_registry(db, repo_root);

    report.elapsed = start.elapsed();
    if !report.is_empty() {
        info!("{report}");
    }
    report
}

/// Full GC on startup.
///
/// Scans for all orphaned `clankers/*` worktrees and branches, cleans up
/// any legacy `pirs/*` leftovers, reconciles the registry, and optionally
/// runs `git gc`.
pub fn gc_on_startup(db: &Db, repo_root: &Path) -> GcReport {
    let start = Instant::now();
    let mut report = GcReport::default();

    info!("gc: startup scan for {}", repo_root.display());

    // 0. Clean up legacy pirs/* worktrees and branches from the rename
    let legacy = cleanup_legacy_worktrees(repo_root);
    report.worktrees_removed += legacy.0;
    report.branches_deleted += legacy.1;

    // 1. Discover all clankers/* worktrees on disk
    let live_worktrees = list_clankers_worktrees(repo_root);

    // 2. Discover all clankers/* branches (merged ones are candidates for deletion)
    let merged_branches = git_ops::sync::list_merged_branches(repo_root, "clankers/*");

    // 3. Get the set of branches the registry considers active
    let active_branches: HashSet<String> = db
        .worktrees()
        .active()
        .unwrap_or_default()
        .into_iter()
        .map(|w| w.branch)
        .collect();

    // 4. Remove worktrees for fully-merged branches that aren't active
    for entry in &live_worktrees {
        if let Some(ref branch) = entry.branch {
            if merged_branches.contains(branch) && !active_branches.contains(branch.as_str()) {
                debug!("gc: removing merged worktree {branch}");
                if git_ops::sync::worktree_remove(repo_root, &entry.path) {
                    report.worktrees_removed += 1;
                }
            }
        }
    }

    // 5. Delete fully-merged clankers/* branches that aren't active
    for branch in &merged_branches {
        if !active_branches.contains(branch.as_str())
            && git_ops::sync::delete_branch(repo_root, branch)
        {
            report.branches_deleted += 1;
        }
    }

    // 6. Prune git's internal worktree tracking for any that lost their directory
    git_ops::sync::worktree_prune(repo_root);

    // 7. Reconcile registry: remove entries for branches that no longer exist
    let remaining_branches: HashSet<String> =
        git_ops::sync::list_branches(repo_root, "clankers/*")
            .into_iter()
            .collect();
    report.registry_pruned += prune_against_git(db, &remaining_branches);

    // 8. Run git gc if we cleaned up enough to warrant it
    if report.branches_deleted >= 10 || report.worktrees_removed >= 10 {
        info!(
            "gc: running git gc (cleaned {} branches, {} worktrees)",
            report.branches_deleted, report.worktrees_removed
        );
        run_git_gc(repo_root);
        report.git_gc_ran = true;
    }

    report.elapsed = start.elapsed();
    if !report.is_empty() {
        info!("{report}");
    } else {
        debug!(
            "gc: startup scan clean ({} active worktrees)",
            active_branches.len()
        );
    }
    report
}

/// Non-blocking startup GC — spawns on a background thread so it doesn't
/// delay the TUI appearing.
pub fn spawn_startup_gc(db: Db, repo_root: PathBuf) -> tokio::task::JoinHandle<GcReport> {
    tokio::task::spawn_blocking(move || gc_on_startup(&db, &repo_root))
}

// ── Legacy cleanup ──────────────────────────────────────────────────────

/// Legacy project name prefixes to clean up. The project was renamed from
/// `pirs` to `clankers` — any leftover worktrees/branches use these.
const LEGACY_PREFIXES: &[&str] = &["pirs"];

/// Remove legacy worktree directories and branches from before the rename.
///
/// Returns (worktrees_removed, branches_deleted).
fn cleanup_legacy_worktrees(repo_root: &Path) -> (usize, usize) {
    let mut wt_removed = 0;
    let mut br_deleted = 0;

    for prefix in LEGACY_PREFIXES {
        // Remove the legacy worktree directory tree (e.g. .git/pirs-worktrees/)
        let legacy_wt_dir = repo_root
            .join(".git")
            .join(format!("{prefix}-worktrees"));
        if legacy_wt_dir.is_dir() {
            let size = git_ops::sync::dir_size_approx(&legacy_wt_dir);
            info!(
                "gc: removing legacy {prefix}-worktrees/ ({:.1} MB)",
                size as f64 / 1_048_576.0
            );
            match std::fs::remove_dir_all(&legacy_wt_dir) {
                Ok(()) => {
                    wt_removed += 1; // counted as one bulk removal
                    // Prune git's worktree refs now that the dirs are gone
                    git_ops::sync::worktree_prune(repo_root);
                }
                Err(e) => warn!("gc: failed to remove {}: {e}", legacy_wt_dir.display()),
            }
        }

        // Delete all legacy branches in bulk
        let pattern = format!("{prefix}/*");
        let branches = git_ops::sync::list_branches(repo_root, &pattern);
        if !branches.is_empty() {
            info!("gc: deleting {} legacy {prefix}/* branches", branches.len());
            let deleted = git_ops::sync::delete_branches_force(repo_root, &branches);
            br_deleted += deleted;
            if deleted < branches.len() {
                debug!(
                    "gc: {}/{} legacy branches deleted (some may have failed)",
                    deleted,
                    branches.len()
                );
            }
        }
    }

    (wt_removed, br_deleted)
}

// ── Git operations (low level) ──────────────────────────────────────────

/// List all worktrees with clankers/* branches.
fn list_clankers_worktrees(repo_root: &Path) -> Vec<git_ops::sync::WorktreeEntry> {
    git_ops::sync::worktree_list(repo_root, "clankers/")
}

/// Remove a git worktree by branch name (looks up path from `.git/clankers-worktrees/`).
fn remove_worktree(repo_root: &Path, branch: &str) -> bool {
    let wt_path = repo_root
        .join(".git")
        .join("clankers-worktrees")
        .join(branch);
    if !wt_path.exists() {
        return false;
    }
    git_ops::sync::worktree_remove(repo_root, &wt_path)
}

/// Run `git gc` in the background. Non-fatal if it fails.
///
/// NOTE: libgit2 does not have a `git gc` equivalent, so this remains
/// a shell-out. It's an optimization (not correctness-critical) and runs
/// rarely (only after cleaning 10+ branches/worktrees).
fn run_git_gc(repo_root: &Path) {
    let result = std::process::Command::new("git")
        .args(["gc", "--auto", "--quiet"])
        .current_dir(repo_root)
        .output();
    match result {
        Ok(o) if o.status.success() => debug!("gc: git gc completed"),
        Ok(o) => debug!("gc: git gc exited with {}", o.status),
        Err(e) => warn!("gc: failed to run git gc: {e}"),
    }
}

// ── Registry reconciliation ─────────────────────────────────────────────

/// Remove registry entries whose branches no longer exist in git.
fn prune_against_git(db: &Db, live_branches: &HashSet<String>) -> usize {
    let all = match db.worktrees().list_all() {
        Ok(v) => v,
        Err(e) => {
            warn!("gc: failed to read registry: {e}");
            return 0;
        }
    };
    let stale: Vec<String> = all
        .into_iter()
        .filter(|w| !live_branches.contains(&w.branch))
        .map(|w| w.branch)
        .collect();
    if stale.is_empty() {
        return 0;
    }
    match db.worktrees().remove_batch(&stale) {
        Ok(n) => n,
        Err(e) => {
            warn!("gc: failed to prune registry: {e}");
            0
        }
    }
}

/// Quick prune: remove registry entries for branches that aren't in git.
fn prune_stale_registry(db: &Db, repo_root: &Path) -> usize {
    let live: HashSet<String> = git_ops::sync::list_branches(repo_root, "clankers/*")
        .into_iter()
        .collect();
    prune_against_git(db, &live)
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use super::*;
    use crate::worktree::WorktreeInfo;
    use crate::worktree::WorktreeStatus;

    fn test_db() -> Db {
        Db::in_memory().unwrap()
    }

    fn make_worktree(branch: &str, status: WorktreeStatus) -> WorktreeInfo {
        WorktreeInfo {
            branch: branch.to_string(),
            path: PathBuf::from(format!("/tmp/{branch}")),
            session_id: "test-sess".to_string(),
            agent: "main".to_string(),
            status,
            created_at: Utc::now(),
            parent_branch: "main".to_string(),
        }
    }

    #[test]
    fn test_gc_report_display_empty() {
        let r = GcReport::default();
        assert_eq!(format!("{r}"), "gc: nothing to clean");
    }

    #[test]
    fn test_gc_report_display_nonempty() {
        let r = GcReport {
            worktrees_removed: 3,
            branches_deleted: 5,
            registry_pruned: 2,
            git_gc_ran: true,
            elapsed: Duration::from_millis(1234),
        };
        let s = format!("{r}");
        assert!(s.contains("3 worktrees"));
        assert!(s.contains("5 branches"));
        assert!(s.contains("2 registry"));
        assert!(s.contains("ran git gc"));
    }

    #[test]
    fn test_prune_against_git_removes_stale() {
        let db = test_db();
        let reg = db.worktrees();
        reg.upsert(&make_worktree("clankers/main-aaa", WorktreeStatus::Active))
            .unwrap();
        reg.upsert(&make_worktree(
            "clankers/main-bbb",
            WorktreeStatus::Completed,
        ))
        .unwrap();
        reg.upsert(&make_worktree("clankers/main-ccc", WorktreeStatus::Active))
            .unwrap();

        // Simulate: only "aaa" still exists in git
        let live: HashSet<String> = ["clankers/main-aaa".to_string()].into();
        let pruned = prune_against_git(&db, &live);
        assert_eq!(pruned, 2);
        assert_eq!(reg.count().unwrap(), 1);
        assert!(reg.get("clankers/main-aaa").unwrap().is_some());
    }

    #[test]
    fn test_prune_against_git_empty() {
        let db = test_db();
        let live = HashSet::new();
        let pruned = prune_against_git(&db, &live);
        assert_eq!(pruned, 0);
    }

    #[test]
    fn test_gc_after_session_prunes_registry() {
        let db = test_db();
        let reg = db.worktrees();
        reg.upsert(&make_worktree("clankers/main-aaa", WorktreeStatus::Active))
            .unwrap();

        // This won't find the branch in a real git repo (we're in a temp dir),
        // but it should still clean the registry entry since the branch doesn't exist.
        let tmp = tempfile::TempDir::new().unwrap();
        let report = gc_after_session(&db, tmp.path(), "clankers/main-aaa");
        // Registry entry pruned because the branch doesn't exist in git
        assert!(report.registry_pruned >= 1);
    }

    #[test]
    fn test_gc_on_startup_in_non_git_dir() {
        let db = test_db();
        let tmp = tempfile::TempDir::new().unwrap();
        let report = gc_on_startup(&db, tmp.path());
        assert!(report.is_empty());
    }
}
