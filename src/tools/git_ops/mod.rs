//! In-process git operations via libgit2.
//!
//! Replaces `tokio::process::Command::new("git")` shell-outs with native
//! `git2` calls. All public functions are async wrappers around blocking
//! `git2` operations (via `spawn_blocking`), so they're safe to call from
//! async tool handlers.

use std::path::Path;

use git2::Repository;
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
        use std::fmt::Write;
        let repo = open_repo()?;
        let mut opts = StatusOptions::new();
        opts.include_untracked(true).recurse_untracked_dirs(true).include_ignored(false);
        let statuses = repo.statuses(Some(&mut opts))?;

        let mut out = String::new();
        for entry in statuses.iter() {
            let path = entry.path().unwrap_or("(non-utf8)");
            let (x, y) = status_chars(entry.status());
            writeln!(out, "{}{} {}", x, y, path).ok();
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

// ── Modules ────────────────────────────────────────────────────────────

pub mod diff;
pub mod log;

// Re-export public items from modules
pub use diff::DiffTarget;
pub use diff::diff_ref;
pub use diff::diff_ref_stat;
pub use diff::diff_staged;
pub use diff::diff_staged_stat;
pub use diff::diff_unstaged;
pub use diff::diff_unstaged_stat;
pub use diff::staged_file_names;
pub use log::LogEntry;
pub use log::log;

#[cfg(test)]
mod tests;

// ════════════════════════════════════════════════════════════════════════
// Synchronous API — used by `src/worktree/` (GC, create, cleanup, etc.)
// ════════════════════════════════════════════════════════════════════════

pub mod sync_ops;
pub use sync_ops as sync;
