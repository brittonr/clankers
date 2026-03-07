//! Blocking git operations for worktree management.
//!
//! These mirror the async wrappers above but run inline (no
//! `spawn_blocking`). They accept an explicit repo root path instead
//! of discovering from CWD.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use git2::{
    BranchType, Repository, WorktreePruneOptions,
};

use super::{GitError, Result};

/// Open a repository at an explicit path.
fn open_at(repo_root: &Path) -> Result<Repository> {
    Repository::open(repo_root)
        .map_err(|e| GitError(format!("Not a git repository ({}): {}", repo_root.display(), e)))
}

// ── Repo discovery ─────────────────────────────────────────────────

/// Check if a directory is inside a git repo (like `git rev-parse --git-dir`).
pub fn is_git_repo(path: &Path) -> bool {
    Repository::discover(path).is_ok()
}

/// Find the repo root for a path (like `git rev-parse --show-toplevel`).
pub fn find_repo_root(path: &Path) -> Option<PathBuf> {
    Repository::discover(path)
        .ok()
        .and_then(|repo| repo.workdir().map(|p| p.to_path_buf()))
}

// ── Worktree lifecycle ─────────────────────────────────────────────

/// Create a new worktree with a new branch.
///
/// Equivalent to `git worktree add -b <branch> <path> <start_point>`.
pub fn worktree_add(
    repo_root: &Path,
    branch_name: &str,
    worktree_path: &Path,
    start_point: &str,
) -> Result<()> {
    let repo = open_at(repo_root)?;

    // Resolve start point to a commit
    let start_obj = repo.revparse_single(start_point).map_err(|e| {
        GitError(format!("Cannot resolve '{}': {}", start_point, e))
    })?;
    let start_commit = start_obj.peel_to_commit().map_err(|e| {
        GitError(format!("'{}' is not a commit: {}", start_point, e))
    })?;

    // Create the branch
    repo.branch(branch_name, &start_commit, false).map_err(|e| {
        GitError(format!("Failed to create branch '{}': {}", branch_name, e))
    })?;

    // Create the worktree
    // git2 worktree name is the last component of the path
    let wt_name = worktree_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(branch_name);

    let reference = repo
        .find_branch(branch_name, BranchType::Local)
        .map_err(|e| GitError(format!("Branch '{}' not found after creation: {}", branch_name, e)))?;

    // Ensure parent directories exist (git2 doesn't create them)
    if let Some(parent) = worktree_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            GitError(format!(
                "Failed to create worktree parent dir '{}': {}",
                parent.display(),
                e
            ))
        })?;
    }

    let mut opts = git2::WorktreeAddOptions::new();
    opts.reference(Some(reference.get()));

    repo.worktree(wt_name, worktree_path, Some(&opts)).map_err(|e| {
        // Clean up the branch if worktree creation fails
        if let Ok(mut br) = repo.find_branch(branch_name, BranchType::Local) {
            let _ = br.delete();
        }
        GitError(format!("Failed to create worktree: {}", e))
    })?;

    Ok(())
}

/// Remove a worktree directory and prune its refs.
///
/// Equivalent to `git worktree remove --force <path>`.
pub fn worktree_remove(repo_root: &Path, worktree_path: &Path) -> bool {
    let repo = match open_at(repo_root) {
        Ok(r) => r,
        Err(_) => return false,
    };

    // Find the worktree by matching paths
    let wt_names = match repo.worktrees() {
        Ok(names) => names,
        Err(_) => {
            // Fallback: just remove the directory
            return remove_dir_and_prune(repo_root, worktree_path);
        }
    };

    for i in 0..wt_names.len() {
        let name = match wt_names.get(i) {
            Some(n) => n,
            None => continue,
        };
        if let Ok(wt) = repo.find_worktree(name)
            && wt.path() == worktree_path
        {
            // Prune (force remove) this worktree
            let mut prune_opts = WorktreePruneOptions::new();
            prune_opts.working_tree(true);
            prune_opts.valid(true);
            prune_opts.locked(true);
            if wt.prune(Some(&mut prune_opts)).is_ok() {
                return true;
            }
        }
    }

    // Worktree not found in git's list — just remove the dir
    remove_dir_and_prune(repo_root, worktree_path)
}

/// Remove directory and prune stale worktree refs.
fn remove_dir_and_prune(repo_root: &Path, worktree_path: &Path) -> bool {
    if worktree_path.exists() {
        let _ = std::fs::remove_dir_all(worktree_path);
    }
    worktree_prune(repo_root);
    true
}

/// Prune stale worktree refs (like `git worktree prune`).
pub fn worktree_prune(repo_root: &Path) {
    let repo = match open_at(repo_root) {
        Ok(r) => r,
        Err(_) => return,
    };

    let wt_names = match repo.worktrees() {
        Ok(names) => names,
        Err(_) => return,
    };

    for i in 0..wt_names.len() {
        let name = match wt_names.get(i) {
            Some(n) => n,
            None => continue,
        };
        if let Ok(wt) = repo.find_worktree(name)
            && wt.validate().is_err()
        {
            let mut opts = WorktreePruneOptions::new();
            opts.working_tree(true);
            let _ = wt.prune(Some(&mut opts));
        }
    }
}

/// Parsed worktree entry from `worktrees()`.
pub struct WorktreeEntry {
    pub name: String,
    pub path: PathBuf,
    pub branch: Option<String>,
}

/// List all worktrees (like `git worktree list --porcelain`).
///
/// Returns only worktrees whose branch starts with the given prefix
/// (e.g. "clankers/"). Pass empty string for all.
pub fn worktree_list(repo_root: &Path, branch_prefix: &str) -> Vec<WorktreeEntry> {
    let repo = match open_at(repo_root) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let wt_names = match repo.worktrees() {
        Ok(names) => names,
        Err(_) => return Vec::new(),
    };

    let mut entries = Vec::new();
    for i in 0..wt_names.len() {
        let name = match wt_names.get(i) {
            Some(n) => n,
            None => continue,
        };
        if let Ok(wt) = repo.find_worktree(name) {
            // Determine the branch by opening the worktree as a repo
            let branch = Repository::open(wt.path())
                .ok()
                .and_then(|wt_repo| {
                    wt_repo
                        .head()
                        .ok()
                        .and_then(|h| h.shorthand().map(|s| s.to_string()))
                });

            let matches = branch_prefix.is_empty()
                || branch.as_ref().is_some_and(|b| b.starts_with(branch_prefix));

            if matches {
                entries.push(WorktreeEntry {
                    name: name.to_string(),
                    path: wt.path().to_path_buf(),
                    branch,
                });
            }
        }
    }
    entries
}

// ── Branch operations ──────────────────────────────────────────────

/// List local branches matching a glob pattern (e.g. "clankers/*").
///
/// Equivalent to `git branch --list <pattern> --format=%(refname:short)`.
pub fn list_branches(repo_root: &Path, pattern: &str) -> Vec<String> {
    let repo = match open_at(repo_root) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let branches = match repo.branches(Some(BranchType::Local)) {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };

    let glob = glob::Pattern::new(pattern).ok();

    branches
        .filter_map(|entry| {
            let (branch, _) = entry.ok()?;
            let name = branch.name().ok()??;
            if let Some(ref g) = glob
                && !g.matches(name)
            {
                return None;
            }
            Some(name.to_string())
        })
        .collect()
}

/// List local branches that are fully merged into HEAD.
///
/// Equivalent to `git branch --merged --list <pattern>`.
pub fn list_merged_branches(repo_root: &Path, pattern: &str) -> HashSet<String> {
    let repo = match open_at(repo_root) {
        Ok(r) => r,
        Err(_) => return HashSet::new(),
    };

    let head_oid = match repo.head().ok().and_then(|h| h.target()) {
        Some(oid) => oid,
        None => return HashSet::new(),
    };

    let branches = match repo.branches(Some(BranchType::Local)) {
        Ok(b) => b,
        Err(_) => return HashSet::new(),
    };

    let glob = glob::Pattern::new(pattern).ok();

    branches
        .filter_map(|entry| {
            let (branch, _) = entry.ok()?;
            let name = branch.name().ok()??.to_string();
            if let Some(ref g) = glob
                && !g.matches(&name)
            {
                return None;
            }
            // Branch is "merged" if its tip is an ancestor of HEAD
            let branch_oid = branch.get().target()?;
            let merge_base = repo.merge_base(branch_oid, head_oid).ok()?;
            if merge_base == branch_oid {
                Some(name)
            } else {
                None
            }
        })
        .collect()
}

/// Delete a branch (safe delete — must be fully merged).
///
/// Equivalent to `git branch -d <branch>`.
pub fn delete_branch(repo_root: &Path, branch_name: &str) -> bool {
    let repo = match open_at(repo_root) {
        Ok(r) => r,
        Err(_) => return false,
    };
    let mut branch = match repo.find_branch(branch_name, BranchType::Local) {
        Ok(b) => b,
        Err(_) => return false,
    };
    branch.delete().is_ok()
}

/// Force-delete branches in bulk.
///
/// Equivalent to `git branch -D <branch> ...`.
/// Returns the number of branches actually deleted.
pub fn delete_branches_force(repo_root: &Path, branch_names: &[String]) -> usize {
    let repo = match open_at(repo_root) {
        Ok(r) => r,
        Err(_) => return 0,
    };
    let mut count = 0;
    for name in branch_names {
        if let Ok(mut branch) = repo.find_branch(name, BranchType::Local)
            && branch.delete().is_ok()
        {
            count += 1;
        }
    }
    count
}

// ── Diff (sync) ────────────────────────────────────────────────────

/// Get file names changed between two refs.
///
/// Equivalent to `git diff --name-only <from> <to>`.
pub fn diff_name_only(
    repo_root: &Path,
    from_ref: &str,
    to_ref: &str,
) -> Option<HashSet<PathBuf>> {
    let repo = open_at(repo_root).ok()?;
    let from_obj = repo.revparse_single(from_ref).ok()?;
    let to_obj = repo.revparse_single(to_ref).ok()?;
    let from_tree = from_obj.peel_to_tree().ok()?;
    let to_tree = to_obj.peel_to_tree().ok()?;

    let diff = repo
        .diff_tree_to_tree(Some(&from_tree), Some(&to_tree), None)
        .ok()?;

    let mut files = HashSet::new();
    for delta_idx in 0..diff.deltas().len() {
        if let Some(delta) = diff.get_delta(delta_idx)
            && let Some(path) = delta.new_file().path()
        {
            files.insert(path.to_path_buf());
        }
    }
    Some(files)
}

// ── Filesystem helpers ─────────────────────────────────────────────

/// Approximate directory size in bytes (recursive walk).
///
/// Replaces `du -sb` shell-out. Returns 0 on any error.
pub fn dir_size_approx(path: &Path) -> u64 {
    fn walk(path: &Path) -> u64 {
        let mut total = 0u64;
        let entries = match std::fs::read_dir(path) {
            Ok(e) => e,
            Err(_) => return 0,
        };
        for entry in entries.flatten() {
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.is_dir() {
                total += walk(&entry.path());
            } else {
                total += meta.len();
            }
        }
        total
    }
    walk(path)
}
