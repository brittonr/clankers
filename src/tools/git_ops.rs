//! In-process git operations via libgit2.
//!
//! Replaces `tokio::process::Command::new("git")` shell-outs with native
//! `git2` calls. All public functions are async wrappers around blocking
//! `git2` operations (via `spawn_blocking`), so they're safe to call from
//! async tool handlers.

use std::path::Path;

use git2::{Diff, DiffFormat, DiffOptions, DiffStatsFormat, Repository, Sort, StatusOptions};

// ── Error ──────────────────────────────────────────────────────────────

/// Lightweight error type for git operations.
#[derive(Debug)]
pub struct GitError(pub String);

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<git2::Error> for GitError {
    fn from(e: git2::Error) -> Self {
        Self(e.message().to_string())
    }
}

type Result<T> = std::result::Result<T, GitError>;

// ── Repository discovery ───────────────────────────────────────────────

/// Open the git repository that contains the current directory.
fn open_repo() -> Result<Repository> {
    Repository::discover(".").map_err(|e| GitError(format!("Not a git repository: {}", e)))
}

// ── Status ─────────────────────────────────────────────────────────────

/// Equivalent to `git status --porcelain`.
///
/// Returns one line per changed entry in the standard two-char format
/// (e.g. ` M src/main.rs`, `?? new.txt`).
pub async fn status_porcelain() -> Result<String> {
    tokio::task::spawn_blocking(|| {
        let repo = open_repo()?;
        let mut opts = StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true)
            .include_ignored(false);
        let statuses = repo.statuses(Some(&mut opts))?;

        let mut out = String::new();
        for entry in statuses.iter() {
            let path = entry.path().unwrap_or("(non-utf8)");
            let (x, y) = status_chars(entry.status());
            out.push_str(&format!("{}{} {}\n", x, y, path));
        }
        Ok(out)
    })
    .await
    .unwrap_or_else(|e| Err(GitError(format!("join error: {}", e))))
}

/// Map git2 status flags to the two-char porcelain codes.
fn status_chars(s: git2::Status) -> (char, char) {
    let index = if s.is_index_new() {
        'A'
    } else if s.is_index_modified() {
        'M'
    } else if s.is_index_deleted() {
        'D'
    } else if s.is_index_renamed() {
        'R'
    } else if s.is_index_typechange() {
        'T'
    } else {
        ' '
    };

    let wt = if s.is_wt_new() {
        '?'
    } else if s.is_wt_modified() {
        'M'
    } else if s.is_wt_deleted() {
        'D'
    } else if s.is_wt_renamed() {
        'R'
    } else if s.is_wt_typechange() {
        'T'
    } else {
        ' '
    };

    // Untracked shows as ?? in porcelain
    if s.is_wt_new() && !s.is_index_new() {
        return ('?', '?');
    }

    (index, wt)
}

// ── Diff helpers ───────────────────────────────────────────────────────

/// What to diff.
#[derive(Debug, Clone, Copy)]
pub enum DiffTarget {
    /// Staged changes (index vs HEAD) — like `git diff --cached`
    Staged,
    /// Unstaged changes (workdir vs index) — like `git diff`
    Unstaged,
    /// Both staged + unstaged combined
    Both,
    /// Diff between a ref and HEAD — like `git diff <base>`
    Ref(/* inline handled by string param */),
}

/// Build the staged diff (index → HEAD tree).
fn build_staged_diff<'a>(repo: &'a Repository, opts: &'a mut DiffOptions) -> Result<Diff<'a>> {
    let head_tree = repo
        .head()
        .ok()
        .and_then(|r| r.peel_to_tree().ok());
    repo.diff_tree_to_index(head_tree.as_ref(), None, Some(opts))
        .map_err(Into::into)
}

/// Build the unstaged diff (workdir → index).
fn build_unstaged_diff<'a>(repo: &'a Repository, opts: &'a mut DiffOptions) -> Result<Diff<'a>> {
    repo.diff_index_to_workdir(None, Some(opts))
        .map_err(Into::into)
}

/// Build a diff between `base_ref` and HEAD.
fn build_ref_diff<'a>(
    repo: &'a Repository,
    base_ref: &str,
    opts: &'a mut DiffOptions,
) -> Result<Diff<'a>> {
    let base_obj = repo
        .revparse_single(base_ref)
        .map_err(|e| GitError(format!("Cannot resolve '{}': {}", base_ref, e)))?;
    let base_tree = base_obj
        .peel_to_tree()
        .map_err(|e| GitError(format!("Cannot get tree for '{}': {}", base_ref, e)))?;

    let head_tree = repo
        .head()
        .ok()
        .and_then(|r| r.peel_to_tree().ok());

    repo.diff_tree_to_tree(Some(&base_tree), head_tree.as_ref(), Some(opts))
        .map_err(Into::into)
}

/// Apply pathspec filters to diff options.
fn apply_pathspecs(opts: &mut DiffOptions, files: &[String]) {
    for f in files {
        opts.pathspec(f);
    }
}

/// Render a diff to its patch text (like `git diff` output).
fn diff_to_patch(diff: &Diff<'_>) -> Result<String> {
    let mut buf = Vec::new();
    diff.print(DiffFormat::Patch, |_delta, _hunk, line| {
        let origin = line.origin();
        match origin {
            '+' | '-' | ' ' => buf.push(origin as u8),
            _ => {}
        }
        buf.extend_from_slice(line.content());
        true
    })?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// Render diff stats (like `git diff --stat`).
fn diff_to_stat(diff: &Diff<'_>) -> Result<String> {
    let stats = diff.stats()?;
    let buf = stats.to_buf(DiffStatsFormat::FULL, 80)?;
    Ok(buf.as_str().unwrap_or("").to_string())
}

// ── Public diff API ────────────────────────────────────────────────────

/// Get diff patch text for staged changes (like `git diff --cached`).
/// Optionally filter to specific files.
pub async fn diff_staged(files: Vec<String>) -> Result<String> {
    tokio::task::spawn_blocking(move || {
        let repo = open_repo()?;
        let mut opts = DiffOptions::new();
        apply_pathspecs(&mut opts, &files);
        let diff = build_staged_diff(&repo, &mut opts)?;
        diff_to_patch(&diff)
    })
    .await
    .unwrap_or_else(|e| Err(GitError(format!("join error: {}", e))))
}

/// Get diff patch text for unstaged changes (like `git diff`).
/// Optionally filter to specific files.
pub async fn diff_unstaged(files: Vec<String>) -> Result<String> {
    tokio::task::spawn_blocking(move || {
        let repo = open_repo()?;
        let mut opts = DiffOptions::new();
        apply_pathspecs(&mut opts, &files);
        let diff = build_unstaged_diff(&repo, &mut opts)?;
        diff_to_patch(&diff)
    })
    .await
    .unwrap_or_else(|e| Err(GitError(format!("join error: {}", e))))
}

/// Get diff stat text for staged changes (like `git diff --cached --stat`).
pub async fn diff_staged_stat(files: Vec<String>) -> Result<String> {
    tokio::task::spawn_blocking(move || {
        let repo = open_repo()?;
        let mut opts = DiffOptions::new();
        apply_pathspecs(&mut opts, &files);
        let diff = build_staged_diff(&repo, &mut opts)?;
        diff_to_stat(&diff)
    })
    .await
    .unwrap_or_else(|e| Err(GitError(format!("join error: {}", e))))
}

/// Get diff stat text for unstaged changes (like `git diff --stat`).
pub async fn diff_unstaged_stat(files: Vec<String>) -> Result<String> {
    tokio::task::spawn_blocking(move || {
        let repo = open_repo()?;
        let mut opts = DiffOptions::new();
        apply_pathspecs(&mut opts, &files);
        let diff = build_unstaged_diff(&repo, &mut opts)?;
        diff_to_stat(&diff)
    })
    .await
    .unwrap_or_else(|e| Err(GitError(format!("join error: {}", e))))
}

/// Get diff patch between a base ref and HEAD (like `git diff <base>`).
/// Optionally filter to specific files.
pub async fn diff_ref(base: String, files: Vec<String>) -> Result<String> {
    tokio::task::spawn_blocking(move || {
        let repo = open_repo()?;
        let mut opts = DiffOptions::new();
        apply_pathspecs(&mut opts, &files);
        let diff = build_ref_diff(&repo, &base, &mut opts)?;
        diff_to_patch(&diff)
    })
    .await
    .unwrap_or_else(|e| Err(GitError(format!("join error: {}", e))))
}

/// Get diff stat between a base ref and HEAD (like `git diff <base> --stat`).
pub async fn diff_ref_stat(base: String, files: Vec<String>) -> Result<String> {
    tokio::task::spawn_blocking(move || {
        let repo = open_repo()?;
        let mut opts = DiffOptions::new();
        apply_pathspecs(&mut opts, &files);
        let diff = build_ref_diff(&repo, &base, &mut opts)?;
        diff_to_stat(&diff)
    })
    .await
    .unwrap_or_else(|e| Err(GitError(format!("join error: {}", e))))
}

/// List staged file names (like `git diff --cached --name-only`).
pub async fn staged_file_names() -> Result<Vec<String>> {
    tokio::task::spawn_blocking(|| {
        let repo = open_repo()?;
        let mut opts = DiffOptions::new();
        let diff = build_staged_diff(&repo, &mut opts)?;
        let mut names = Vec::new();
        for delta_idx in 0..diff.deltas().len() {
            let delta = diff.get_delta(delta_idx).expect("delta in range");
            if let Some(path) = delta.new_file().path() {
                names.push(path.to_string_lossy().into_owned());
            }
        }
        Ok(names)
    })
    .await
    .unwrap_or_else(|e| Err(GitError(format!("join error: {}", e))))
}

// ── Staging ────────────────────────────────────────────────────────────

/// Stage files (like `git add <files>`).
///
/// Returns `(staged, errors)` — files that were staged successfully and
/// files that failed with error messages.
pub async fn stage_files(files: Vec<String>) -> (Vec<String>, Vec<String>) {
    tokio::task::spawn_blocking(move || {
        let repo = match open_repo() {
            Ok(r) => r,
            Err(e) => {
                let errors: Vec<String> = files
                    .iter()
                    .map(|f| format!("{}: {}", f, e))
                    .collect();
                return (vec![], errors);
            }
        };

        let mut index = match repo.index() {
            Ok(i) => i,
            Err(e) => {
                let errors: Vec<String> = files
                    .iter()
                    .map(|f| format!("{}: {}", f, e))
                    .collect();
                return (vec![], errors);
            }
        };

        let mut staged = Vec::new();
        let mut errors = Vec::new();

        for file in &files {
            let path = Path::new(file);
            match index.add_path(path) {
                Ok(()) => staged.push(file.clone()),
                Err(e) => errors.push(format!("{}: {}", file, e.message())),
            }
        }

        if let Err(e) = index.write() {
            // If we can't write the index, report all as errors
            return (
                vec![],
                files
                    .iter()
                    .map(|f| format!("{}: failed to write index: {}", f, e))
                    .collect(),
            );
        }

        (staged, errors)
    })
    .await
    .unwrap_or_else(|_| (vec![], vec!["join error".to_string()]))
}

// ── Commit ─────────────────────────────────────────────────────────────

/// Create a commit with the current index (like `git commit -m <msg>`).
///
/// Returns the commit output summary on success.
pub async fn commit(message: String) -> Result<CommitResult> {
    tokio::task::spawn_blocking(move || {
        let repo = open_repo()?;
        let sig = repo.signature()?;
        let mut index = repo.index()?;
        let tree_oid = index.write_tree()?;
        let tree = repo.find_tree(tree_oid)?;

        // Find parent commit (HEAD)
        let parents = if let Ok(head) = repo.head() {
            vec![head.peel_to_commit()?]
        } else {
            vec![] // Initial commit
        };

        let parent_refs: Vec<&git2::Commit<'_>> = parents.iter().collect();

        let oid = repo.commit(Some("HEAD"), &sig, &sig, &message, &tree, &parent_refs)?;

        let short_hash = oid.to_string()[..7].to_string();
        let files_changed = {
            let stats = if parent_refs.is_empty() {
                // Initial commit — diff tree to empty
                let diff = repo.diff_tree_to_tree(None, Some(&tree), None)?;
                diff.stats()?
            } else {
                let parent_tree = parent_refs[0].tree()?;
                let diff = repo.diff_tree_to_tree(Some(&parent_tree), Some(&tree), None)?;
                diff.stats()?
            };
            format!(
                "{} file(s) changed, {} insertions(+), {} deletions(-)",
                stats.files_changed(),
                stats.insertions(),
                stats.deletions()
            )
        };

        Ok(CommitResult {
            short_hash,
            full_hash: oid.to_string(),
            summary: files_changed,
        })
    })
    .await
    .unwrap_or_else(|e| Err(GitError(format!("join error: {}", e))))
}

/// Result of a successful commit.
pub struct CommitResult {
    pub short_hash: String,
    pub full_hash: String,
    pub summary: String,
}

// ── Ref resolution ─────────────────────────────────────────────────────

/// Check if a ref (branch, tag, commit) exists.
pub async fn ref_exists(refname: String) -> bool {
    tokio::task::spawn_blocking(move || {
        let repo = match open_repo() {
            Ok(r) => r,
            Err(_) => return false,
        };
        repo.revparse_single(&refname).is_ok()
    })
    .await
    .unwrap_or(false)
}

/// Get the short hash of HEAD.
pub async fn head_short_hash() -> Result<String> {
    tokio::task::spawn_blocking(|| {
        let repo = open_repo()?;
        let head = repo.head()?;
        let oid = head.target().ok_or_else(|| GitError("HEAD has no target".into()))?;
        Ok(oid.to_string()[..7].to_string())
    })
    .await
    .unwrap_or_else(|e| Err(GitError(format!("join error: {}", e))))
}

// ── Log ────────────────────────────────────────────────────────────────

/// A single log entry.
pub struct LogEntry {
    pub short_hash: String,
    pub subject: String,
    pub author: String,
    pub relative_time: String,
}

/// Walk recent commits (like `git log -N`).
///
/// Returns up to `count` log entries from HEAD.
pub async fn log(count: usize) -> Result<Vec<LogEntry>> {
    tokio::task::spawn_blocking(move || {
        let repo = open_repo()?;
        let mut revwalk = repo.revwalk()?;
        revwalk.push_head()?;
        revwalk.set_sorting(Sort::TIME)?;

        let mut entries = Vec::with_capacity(count);
        for (i, oid_result) in revwalk.enumerate() {
            if i >= count {
                break;
            }
            let oid = oid_result?;
            let commit = repo.find_commit(oid)?;
            let short = oid.to_string()[..7].to_string();
            let subject = commit
                .summary()
                .unwrap_or("(no message)")
                .to_string();
            let author = commit
                .author()
                .name()
                .unwrap_or("unknown")
                .to_string();
            let time = commit.time();
            let relative = format_relative_time(time.seconds());

            entries.push(LogEntry {
                short_hash: short,
                subject,
                author,
                relative_time: relative,
            });
        }

        Ok(entries)
    })
    .await
    .unwrap_or_else(|e| Err(GitError(format!("join error: {}", e))))
}

/// Format a unix timestamp as a relative time string (e.g. "2 hours ago").
fn format_relative_time(epoch_secs: i64) -> String {
    let now = chrono::Utc::now().timestamp();
    let delta = now - epoch_secs;
    if delta < 0 {
        return "in the future".to_string();
    }
    let delta = delta as u64;
    if delta < 60 {
        format!("{} seconds ago", delta)
    } else if delta < 3600 {
        let m = delta / 60;
        format!("{} minute{} ago", m, if m == 1 { "" } else { "s" })
    } else if delta < 86400 {
        let h = delta / 3600;
        format!("{} hour{} ago", h, if h == 1 { "" } else { "s" })
    } else if delta < 604_800 {
        let d = delta / 86400;
        format!("{} day{} ago", d, if d == 1 { "" } else { "s" })
    } else if delta < 2_592_000 {
        let w = delta / 604_800;
        format!("{} week{} ago", w, if w == 1 { "" } else { "s" })
    } else if delta < 31_536_000 {
        let m = delta / 2_592_000;
        format!("{} month{} ago", m, if m == 1 { "" } else { "s" })
    } else {
        let y = delta / 31_536_000;
        format!("{} year{} ago", y, if y == 1 { "" } else { "s" })
    }
}

// ════════════════════════════════════════════════════════════════════════
// Synchronous API — used by `src/worktree/` (GC, create, cleanup, etc.)
// ════════════════════════════════════════════════════════════════════════

pub mod sync {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_chars_untracked() {
        let s = git2::Status::WT_NEW;
        assert_eq!(status_chars(s), ('?', '?'));
    }

    #[test]
    fn test_status_chars_modified_in_index() {
        let s = git2::Status::INDEX_MODIFIED;
        assert_eq!(status_chars(s), ('M', ' '));
    }

    #[test]
    fn test_status_chars_modified_in_workdir() {
        let s = git2::Status::WT_MODIFIED;
        assert_eq!(status_chars(s), (' ', 'M'));
    }

    #[test]
    fn test_status_chars_added_to_index() {
        let s = git2::Status::INDEX_NEW;
        assert_eq!(status_chars(s), ('A', ' '));
    }

    #[test]
    fn test_worktree_add_and_remove() {
        use std::process::Command;

        let tmp = tempfile::TempDir::new().unwrap();
        let repo = tmp.path();

        // Init a real git repo
        Command::new("git").args(["init"]).current_dir(repo).output().unwrap();
        Command::new("git").args(["config", "user.email", "t@t.com"]).current_dir(repo).output().unwrap();
        Command::new("git").args(["config", "user.name", "T"]).current_dir(repo).output().unwrap();
        std::fs::write(repo.join("README.md"), "hello").unwrap();
        Command::new("git").args(["add", "."]).current_dir(repo).output().unwrap();
        Command::new("git").args(["commit", "-m", "init"]).current_dir(repo).output().unwrap();

        // Test worktree_add
        let wt_path = repo.join(".git").join("clankers-worktrees").join("clankers").join("test-1");
        let result = sync::worktree_add(repo, "clankers/test-1", &wt_path, "HEAD");
        assert!(result.is_ok(), "worktree_add failed: {:?}", result.err());

        // Verify the worktree directory exists
        assert!(wt_path.exists(), "worktree path should exist");

        // Verify the branch exists
        let branches = sync::list_branches(repo, "clankers/*");
        assert!(branches.contains(&"clankers/test-1".to_string()), "branch should exist: {:?}", branches);

        // Test worktree_remove
        assert!(sync::worktree_remove(repo, &wt_path));

        // Test delete_branch
        assert!(sync::delete_branch(repo, "clankers/test-1"));
    }

    #[test]
    fn test_list_branches_and_merged() {
        use std::process::Command;

        let tmp = tempfile::TempDir::new().unwrap();
        let repo = tmp.path();

        Command::new("git").args(["init"]).current_dir(repo).output().unwrap();
        Command::new("git").args(["config", "user.email", "t@t.com"]).current_dir(repo).output().unwrap();
        Command::new("git").args(["config", "user.name", "T"]).current_dir(repo).output().unwrap();
        std::fs::write(repo.join("f.txt"), "hello").unwrap();
        Command::new("git").args(["add", "."]).current_dir(repo).output().unwrap();
        Command::new("git").args(["commit", "-m", "init"]).current_dir(repo).output().unwrap();

        // Create a branch at HEAD (so it's trivially merged)
        let r = git2::Repository::open(repo).unwrap();
        let head = r.head().unwrap().peel_to_commit().unwrap();
        r.branch("clankers/merged-1", &head, false).unwrap();

        let branches = sync::list_branches(repo, "clankers/*");
        assert_eq!(branches, vec!["clankers/merged-1"]);

        let merged = sync::list_merged_branches(repo, "clankers/*");
        assert!(merged.contains("clankers/merged-1"));
    }

    #[test]
    fn test_diff_name_only() {
        use std::process::Command;

        let tmp = tempfile::TempDir::new().unwrap();
        let repo = tmp.path();

        Command::new("git").args(["init", "-b", "main"]).current_dir(repo).output().unwrap();
        Command::new("git").args(["config", "user.email", "t@t.com"]).current_dir(repo).output().unwrap();
        Command::new("git").args(["config", "user.name", "T"]).current_dir(repo).output().unwrap();
        std::fs::write(repo.join("a.txt"), "hello").unwrap();
        Command::new("git").args(["add", "."]).current_dir(repo).output().unwrap();
        Command::new("git").args(["commit", "-m", "first"]).current_dir(repo).output().unwrap();

        // Create branch, add file
        Command::new("git").args(["checkout", "-b", "feature"]).current_dir(repo).output().unwrap();
        std::fs::write(repo.join("b.txt"), "world").unwrap();
        Command::new("git").args(["add", "."]).current_dir(repo).output().unwrap();
        Command::new("git").args(["commit", "-m", "second"]).current_dir(repo).output().unwrap();

        let files = sync::diff_name_only(repo, "main", "feature");
        assert!(files.is_some(), "diff_name_only returned None");
        let files = files.unwrap();
        assert!(files.contains(&std::path::PathBuf::from("b.txt")));
    }

    #[test]
    fn test_dir_size_approx() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("a.txt"), "hello world").unwrap();
        std::fs::create_dir(tmp.path().join("sub")).unwrap();
        std::fs::write(tmp.path().join("sub/b.txt"), "12345").unwrap();
        let size = sync::dir_size_approx(tmp.path());
        assert!(size >= 16, "Expected at least 16 bytes, got {}", size);
    }

    #[test]
    fn test_format_relative_time() {
        let now = chrono::Utc::now().timestamp();
        assert!(format_relative_time(now - 30).contains("seconds"));
        assert!(format_relative_time(now - 120).contains("minute"));
        assert!(format_relative_time(now - 7200).contains("hour"));
        assert!(format_relative_time(now - 86400).contains("day"));
    }
}
