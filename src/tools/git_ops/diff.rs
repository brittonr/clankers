//! Git diff operations via libgit2.

use git2::Diff;
use git2::DiffFormat;
use git2::DiffOptions;
use git2::DiffStatsFormat;
use git2::Repository;

use super::GitError;
use super::Result;
use super::open_repo;

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
pub fn build_staged_diff<'a>(repo: &'a Repository, opts: &'a mut DiffOptions) -> Result<Diff<'a>> {
    let head_tree = repo.head().ok().and_then(|r| r.peel_to_tree().ok());
    repo.diff_tree_to_index(head_tree.as_ref(), None, Some(opts)).map_err(Into::into)
}

/// Build the unstaged diff (workdir → index).
pub fn build_unstaged_diff<'a>(repo: &'a Repository, opts: &'a mut DiffOptions) -> Result<Diff<'a>> {
    repo.diff_index_to_workdir(None, Some(opts)).map_err(Into::into)
}

/// Build a diff between `base_ref` and HEAD.
pub fn build_ref_diff<'a>(repo: &'a Repository, base_ref: &str, opts: &'a mut DiffOptions) -> Result<Diff<'a>> {
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
pub fn apply_pathspecs(opts: &mut DiffOptions, files: &[String]) {
    for f in files {
        opts.pathspec(f);
    }
}

/// Render a diff to its patch text (like `git diff` output).
pub fn diff_to_patch(diff: &Diff<'_>) -> Result<String> {
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
pub fn diff_to_stat(diff: &Diff<'_>) -> Result<String> {
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
