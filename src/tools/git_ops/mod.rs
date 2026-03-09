//! In-process git operations via libgit2.
//!
//! Replaces `tokio::process::Command::new("git")` shell-outs with native
//! `git2` calls. All public functions are async wrappers around blocking
//! `git2` operations (via `spawn_blocking`), so they're safe to call from
//! async tool handlers.

use std::path::Path;

use git2::Diff;
use git2::DiffFormat;
use git2::DiffOptions;
use git2::DiffStatsFormat;
use git2::Repository;
use git2::Sort;
use git2::StatusOptions;

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
        opts.include_untracked(true).recurse_untracked_dirs(true).include_ignored(false);
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
    .map_err(|e| GitError(format!("join error: {}", e)))?
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
    let head_tree = repo.head().ok().and_then(|r| r.peel_to_tree().ok());
    repo.diff_tree_to_index(head_tree.as_ref(), None, Some(opts)).map_err(Into::into)
}

/// Build the unstaged diff (workdir → index).
fn build_unstaged_diff<'a>(repo: &'a Repository, opts: &'a mut DiffOptions) -> Result<Diff<'a>> {
    repo.diff_index_to_workdir(None, Some(opts)).map_err(Into::into)
}

/// Build a diff between `base_ref` and HEAD.
fn build_ref_diff<'a>(repo: &'a Repository, base_ref: &str, opts: &'a mut DiffOptions) -> Result<Diff<'a>> {
    let base_obj = repo
        .revparse_single(base_ref)
        .map_err(|e| GitError(format!("Cannot resolve '{}': {}", base_ref, e)))?;
    let base_tree = base_obj
        .peel_to_tree()
        .map_err(|e| GitError(format!("Cannot get tree for '{}': {}", base_ref, e)))?;

    let head_tree = repo.head().ok().and_then(|r| r.peel_to_tree().ok());

    repo.diff_tree_to_tree(Some(&base_tree), head_tree.as_ref(), Some(opts)).map_err(Into::into)
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
    .map_err(|e| GitError(format!("join error: {}", e)))?
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
    .map_err(|e| GitError(format!("join error: {}", e)))?
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
    .map_err(|e| GitError(format!("join error: {}", e)))?
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
    .map_err(|e| GitError(format!("join error: {}", e)))?
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
    .map_err(|e| GitError(format!("join error: {}", e)))?
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
    .map_err(|e| GitError(format!("join error: {}", e)))?
}

/// List staged file names (like `git diff --cached --name-only`).
pub async fn staged_file_names() -> Result<Vec<String>> {
    tokio::task::spawn_blocking(|| {
        let repo = open_repo()?;
        let mut opts = DiffOptions::new();
        let diff = build_staged_diff(&repo, &mut opts)?;
        let mut names = Vec::new();
        for delta_idx in 0..diff.deltas().len() {
            let delta = diff.get_delta(delta_idx).expect("delta index in range from diff.deltas().len()");
            if let Some(path) = delta.new_file().path() {
                names.push(path.to_string_lossy().into_owned());
            }
        }
        Ok(names)
    })
    .await
    .map_err(|e| GitError(format!("join error: {}", e)))?
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
                let errors: Vec<String> = files.iter().map(|f| format!("{}: {}", f, e)).collect();
                return (vec![], errors);
            }
        };

        let mut index = match repo.index() {
            Ok(i) => i,
            Err(e) => {
                let errors: Vec<String> = files.iter().map(|f| format!("{}: {}", f, e)).collect();
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
            return (vec![], files.iter().map(|f| format!("{}: failed to write index: {}", f, e)).collect());
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
    .map_err(|e| GitError(format!("join error: {}", e)))?
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
    .map_err(|e| GitError(format!("join error: {}", e)))?
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
            let subject = commit.summary().unwrap_or("(no message)").to_string();
            let author = commit.author().name().unwrap_or("unknown").to_string();
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
    .map_err(|e| GitError(format!("join error: {}", e)))?
}

/// Time unit constants for relative time formatting
const SECS_PER_MINUTE: u64 = 60;
const SECS_PER_HOUR: u64 = 3600;
const SECS_PER_DAY: u64 = 86400;
const SECS_PER_WEEK: u64 = 604_800;
const SECS_PER_MONTH: u64 = 2_592_000;
const SECS_PER_YEAR: u64 = 31_536_000;

/// Format a unix timestamp as a relative time string (e.g. "2 hours ago").
fn format_relative_time(epoch_secs: i64) -> String {
    let now = chrono::Utc::now().timestamp();
    let delta = now - epoch_secs;
    if delta < 0 {
        return "in the future".to_string();
    }
    let delta = delta as u64;
    if delta < SECS_PER_MINUTE {
        format!("{} seconds ago", delta)
    } else if delta < SECS_PER_HOUR {
        let m = delta / SECS_PER_MINUTE;
        format!("{} minute{} ago", m, if m == 1 { "" } else { "s" })
    } else if delta < SECS_PER_DAY {
        let h = delta / SECS_PER_HOUR;
        format!("{} hour{} ago", h, if h == 1 { "" } else { "s" })
    } else if delta < SECS_PER_WEEK {
        let d = delta / SECS_PER_DAY;
        format!("{} day{} ago", d, if d == 1 { "" } else { "s" })
    } else if delta < SECS_PER_MONTH {
        let w = delta / SECS_PER_WEEK;
        format!("{} week{} ago", w, if w == 1 { "" } else { "s" })
    } else if delta < SECS_PER_YEAR {
        let m = delta / SECS_PER_MONTH;
        format!("{} month{} ago", m, if m == 1 { "" } else { "s" })
    } else {
        let y = delta / SECS_PER_YEAR;
        format!("{} year{} ago", y, if y == 1 { "" } else { "s" })
    }
}

// ════════════════════════════════════════════════════════════════════════
// Synchronous API — used by `src/worktree/` (GC, create, cleanup, etc.)
// ════════════════════════════════════════════════════════════════════════

pub mod sync_ops;
pub use sync_ops as sync;

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

        let tmp = tempfile::TempDir::new().expect("tempdir creation should succeed");
        let repo = tmp.path();

        // Init a real git repo
        Command::new("git").args(["init"]).current_dir(repo).output().expect("git init should succeed");
        Command::new("git")
            .args(["config", "user.email", "t@t.com"])
            .current_dir(repo)
            .output()
            .expect("git config email should succeed");
        Command::new("git")
            .args(["config", "user.name", "T"])
            .current_dir(repo)
            .output()
            .expect("git config name should succeed");
        std::fs::write(repo.join("README.md"), "hello").expect("test file write should succeed");
        Command::new("git").args(["add", "."]).current_dir(repo).output().expect("git add should succeed");
        Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(repo)
            .output()
            .expect("git commit should succeed");

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

        let tmp = tempfile::TempDir::new().expect("tempdir creation should succeed");
        let repo = tmp.path();

        Command::new("git").args(["init"]).current_dir(repo).output().expect("git init should succeed");
        Command::new("git")
            .args(["config", "user.email", "t@t.com"])
            .current_dir(repo)
            .output()
            .expect("git config email should succeed");
        Command::new("git")
            .args(["config", "user.name", "T"])
            .current_dir(repo)
            .output()
            .expect("git config name should succeed");
        std::fs::write(repo.join("f.txt"), "hello").expect("test file write should succeed");
        Command::new("git").args(["add", "."]).current_dir(repo).output().expect("git add should succeed");
        Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(repo)
            .output()
            .expect("git commit should succeed");

        // Create a branch at HEAD (so it's trivially merged)
        let r = git2::Repository::open(repo).expect("repo open should succeed");
        let head = r.head().expect("HEAD should exist").peel_to_commit().expect("HEAD should peel to commit");
        r.branch("clankers/merged-1", &head, false).expect("branch creation should succeed");

        let branches = sync::list_branches(repo, "clankers/*");
        assert_eq!(branches, vec!["clankers/merged-1"]);

        let merged = sync::list_merged_branches(repo, "clankers/*");
        assert!(merged.contains("clankers/merged-1"));
    }

    #[test]
    fn test_diff_name_only() {
        use std::process::Command;

        let tmp = tempfile::TempDir::new().expect("tempdir creation should succeed");
        let repo = tmp.path();

        Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(repo)
            .output()
            .expect("git init should succeed");
        Command::new("git")
            .args(["config", "user.email", "t@t.com"])
            .current_dir(repo)
            .output()
            .expect("git config email should succeed");
        Command::new("git")
            .args(["config", "user.name", "T"])
            .current_dir(repo)
            .output()
            .expect("git config name should succeed");
        std::fs::write(repo.join("a.txt"), "hello").expect("test file write should succeed");
        Command::new("git").args(["add", "."]).current_dir(repo).output().expect("git add should succeed");
        Command::new("git")
            .args(["commit", "-m", "first"])
            .current_dir(repo)
            .output()
            .expect("git commit should succeed");

        // Create branch, add file
        Command::new("git")
            .args(["checkout", "-b", "feature"])
            .current_dir(repo)
            .output()
            .expect("git checkout should succeed");
        std::fs::write(repo.join("b.txt"), "world").expect("test file write should succeed");
        Command::new("git").args(["add", "."]).current_dir(repo).output().expect("git add should succeed");
        Command::new("git")
            .args(["commit", "-m", "second"])
            .current_dir(repo)
            .output()
            .expect("git commit should succeed");

        let files = sync::diff_name_only(repo, "main", "feature");
        assert!(files.is_some(), "diff_name_only returned None");
        let files = files.expect("files should be present");
        assert!(files.contains(&std::path::PathBuf::from("b.txt")));
    }

    #[test]
    fn test_dir_size_approx() {
        let tmp = tempfile::TempDir::new().expect("tempdir creation should succeed");
        std::fs::write(tmp.path().join("a.txt"), "hello world").expect("test file write should succeed");
        std::fs::create_dir(tmp.path().join("sub")).expect("test dir creation should succeed");
        std::fs::write(tmp.path().join("sub/b.txt"), "12345").expect("test file write should succeed");
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
