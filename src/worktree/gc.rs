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

use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use tracing::debug;
use tracing::info;
use tracing::warn;

use crate::db::Db;

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
    if delete_merged_branch(repo_root, branch) {
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
    let merged_branches = list_merged_clankers_branches(repo_root);

    // 3. Get the set of branches the registry considers active
    let active_branches: HashSet<String> =
        db.worktrees().active().unwrap_or_default().into_iter().map(|w| w.branch).collect();

    // 4. Remove worktrees for fully-merged branches that aren't active
    for (branch, wt_path) in &live_worktrees {
        if merged_branches.contains(branch) && !active_branches.contains(branch.as_str()) {
            debug!("gc: removing merged worktree {branch}");
            if remove_worktree_at(repo_root, wt_path) {
                report.worktrees_removed += 1;
            }
        }
    }

    // 5. Delete fully-merged clankers/* branches that aren't active
    for branch in &merged_branches {
        if !active_branches.contains(branch.as_str()) && delete_merged_branch(repo_root, branch) {
            report.branches_deleted += 1;
        }
    }

    // 6. Prune git's internal worktree tracking for any that lost their directory
    prune_worktree_refs(repo_root);

    // 7. Reconcile registry: remove entries for branches that no longer exist
    let remaining_branches: HashSet<String> = list_clankers_branches(repo_root).into_iter().collect();
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
        debug!("gc: startup scan clean ({} active worktrees)", active_branches.len());
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
        let legacy_wt_dir = repo_root.join(".git").join(format!("{prefix}-worktrees"));
        if legacy_wt_dir.is_dir() {
            let size = dir_size_approx(&legacy_wt_dir);
            info!("gc: removing legacy {prefix}-worktrees/ ({:.1} MB)", size as f64 / 1_048_576.0);
            match std::fs::remove_dir_all(&legacy_wt_dir) {
                Ok(()) => {
                    wt_removed += 1; // counted as one bulk removal
                    // Prune git's worktree refs now that the dirs are gone
                    prune_worktree_refs(repo_root);
                }
                Err(e) => warn!("gc: failed to remove {}: {e}", legacy_wt_dir.display()),
            }
        }

        // Delete all legacy branches in bulk
        let branches = list_branches_with_prefix(repo_root, &format!("{prefix}/"));
        if !branches.is_empty() {
            info!("gc: deleting {} legacy {prefix}/* branches", branches.len());
            for chunk in branches.chunks(100) {
                let mut cmd = std::process::Command::new("git");
                cmd.arg("branch").arg("-D");
                for b in chunk {
                    cmd.arg(b);
                }
                match cmd.current_dir(repo_root).output() {
                    Ok(o) if o.status.success() => br_deleted += chunk.len(),
                    Ok(o) => {
                        // Some may have failed; count successes from output
                        let out = String::from_utf8_lossy(&o.stdout);
                        br_deleted += out.lines().filter(|l| l.starts_with("Deleted")).count();
                        debug!("gc: some branch deletes failed: {}", String::from_utf8_lossy(&o.stderr).trim());
                    }
                    Err(e) => warn!("gc: git branch -D failed: {e}"),
                }
            }
        }
    }

    (wt_removed, br_deleted)
}

/// List all branch names matching a prefix (e.g. "pirs/").
fn list_branches_with_prefix(repo_root: &Path, prefix: &str) -> Vec<String> {
    let pattern = format!("{prefix}*");
    let output = std::process::Command::new("git")
        .args(["branch", "--list", &pattern, "--format=%(refname:short)"])
        .current_dir(repo_root)
        .output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter(|l| !l.is_empty())
            .map(|s| s.to_string())
            .collect(),
        _ => Vec::new(),
    }
}

/// Quick approximate directory size via `du -sb`. Returns 0 on failure.
fn dir_size_approx(path: &Path) -> u64 {
    std::process::Command::new("du")
        .args(["-sb"])
        .arg(path)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8_lossy(&o.stdout).split_whitespace().next().and_then(|s| s.parse().ok()))
        .unwrap_or(0)
}

/// Run `git worktree prune` to clean stale worktree tracking refs.
fn prune_worktree_refs(repo_root: &Path) {
    match std::process::Command::new("git").args(["worktree", "prune"]).current_dir(repo_root).output() {
        Ok(o) if o.status.success() => debug!("gc: pruned worktree refs"),
        Ok(o) => debug!("gc: git worktree prune: {}", String::from_utf8_lossy(&o.stderr).trim()),
        Err(e) => warn!("gc: failed to run git worktree prune: {e}"),
    }
}

// ── Git operations (low level) ──────────────────────────────────────────

/// List all worktrees under `.git/clankers-worktrees/` with their branch names.
fn list_clankers_worktrees(repo_root: &Path) -> Vec<(String, PathBuf)> {
    let output = std::process::Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(repo_root)
        .output();
    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };
    let text = String::from_utf8_lossy(&output.stdout);

    let mut results = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    for line in text.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            current_path = Some(PathBuf::from(path));
        } else if let Some(branch) = line.strip_prefix("branch refs/heads/") {
            if let Some(ref path) = current_path
                && branch.starts_with("clankers/")
            {
                results.push((branch.to_string(), path.clone()));
            }
        } else if line.is_empty() {
            current_path = None;
        }
    }
    results
}

/// List all `clankers/*` branch names.
fn list_clankers_branches(repo_root: &Path) -> Vec<String> {
    let output = std::process::Command::new("git")
        .args(["branch", "--list", "clankers/*", "--format=%(refname:short)"])
        .current_dir(repo_root)
        .output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter(|l| !l.is_empty())
            .map(|s| s.to_string())
            .collect(),
        _ => Vec::new(),
    }
}

/// List `clankers/*` branches that are fully merged into HEAD.
fn list_merged_clankers_branches(repo_root: &Path) -> HashSet<String> {
    let output = std::process::Command::new("git")
        .args([
            "branch",
            "--merged",
            "--list",
            "clankers/*",
            "--format=%(refname:short)",
        ])
        .current_dir(repo_root)
        .output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter(|l| !l.is_empty())
            .map(|s| s.to_string())
            .collect(),
        _ => HashSet::new(),
    }
}

/// Remove a git worktree by branch name (looks up path from `.git/clankers-worktrees/`).
fn remove_worktree(repo_root: &Path, branch: &str) -> bool {
    let wt_path = repo_root.join(".git").join("clankers-worktrees").join(branch);
    if !wt_path.exists() {
        return false;
    }
    remove_worktree_at(repo_root, &wt_path)
}

/// Remove a git worktree at a specific path.
fn remove_worktree_at(repo_root: &Path, wt_path: &Path) -> bool {
    let result = std::process::Command::new("git")
        .args(["worktree", "remove", "--force"])
        .arg(wt_path)
        .current_dir(repo_root)
        .output();
    match result {
        Ok(o) if o.status.success() => {
            debug!("gc: removed worktree {}", wt_path.display());
            true
        }
        Ok(o) => {
            // If git worktree remove fails, try direct cleanup
            debug!(
                "gc: git worktree remove failed ({}), trying direct cleanup",
                String::from_utf8_lossy(&o.stderr).trim()
            );
            if wt_path.exists() {
                std::fs::remove_dir_all(wt_path).ok();
            }
            // Also prune stale worktree references
            let _ = std::process::Command::new("git").args(["worktree", "prune"]).current_dir(repo_root).output();
            true
        }
        Err(e) => {
            warn!("gc: failed to run git worktree remove: {e}");
            false
        }
    }
}

/// Delete a fully-merged branch. Uses `-d` (safe delete).
fn delete_merged_branch(repo_root: &Path, branch: &str) -> bool {
    let result = std::process::Command::new("git").args(["branch", "-d", branch]).current_dir(repo_root).output();
    match result {
        Ok(o) if o.status.success() => {
            debug!("gc: deleted branch {branch}");
            true
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            // Not an error if the branch just doesn't exist
            if !stderr.contains("not found") {
                debug!("gc: branch -d {branch} failed: {}", stderr.trim());
            }
            false
        }
        Err(e) => {
            warn!("gc: failed to run git branch -d: {e}");
            false
        }
    }
}

/// Run `git gc` in the background. Non-fatal if it fails.
fn run_git_gc(repo_root: &Path) {
    let result = std::process::Command::new("git").args(["gc", "--auto", "--quiet"]).current_dir(repo_root).output();
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
    let stale: Vec<String> = all.into_iter().filter(|w| !live_branches.contains(&w.branch)).map(|w| w.branch).collect();
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
    let live: HashSet<String> = list_clankers_branches(repo_root).into_iter().collect();
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
        reg.upsert(&make_worktree("clankers/main-aaa", WorktreeStatus::Active)).unwrap();
        reg.upsert(&make_worktree("clankers/main-bbb", WorktreeStatus::Completed)).unwrap();
        reg.upsert(&make_worktree("clankers/main-ccc", WorktreeStatus::Active)).unwrap();

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
        reg.upsert(&make_worktree("clankers/main-aaa", WorktreeStatus::Active)).unwrap();

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
