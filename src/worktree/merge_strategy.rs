//! Merge strategy: graggle -> rerere -> LLM -> human
//!
//! Tiered merge resolution using clankers-merge's order-independent graggle algorithm.
//! Uses in-process git2 operations instead of shelling out to git CLI.

use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use crate::error::Result;

/// Result of merging a single file
#[derive(Debug, Clone)]
pub enum FileMergeResult {
    /// Clean merge, no conflicts
    Clean { content: String },
    /// Conflicts exist, content has conflict markers
    Conflict { content: String, num_conflicts: usize },
}

/// Result of merging a complete branch
#[derive(Debug)]
pub enum MergeResult {
    /// All files merged cleanly
    Clean,
    /// Some files have conflicts that need resolution
    NeedsHuman { conflicting_files: Vec<PathBuf> },
}

/// Merge a single file from multiple branches using clankers-merge's graggle algorithm.
///
/// 1. Read base content from parent branch
/// 2. Read each branch's version
/// 3. Run clankers_merge::merge() for order-independent result
/// 4. Return clean content or content with conflict markers
pub fn merge_file(
    repo_root: &Path,
    file_path: &Path,
    parent_branch: &str,
    branches: &[String],
) -> Result<FileMergeResult> {
    // Read base content from parent branch
    let base_content = git_show(repo_root, parent_branch, file_path).unwrap_or_default();

    // Read each branch's version
    let branch_contents: Vec<String> =
        branches.iter().map(|b| git_show(repo_root, b, file_path).unwrap_or_default()).collect();

    let branch_refs: Vec<&str> = branch_contents.iter().map(|s| s.as_str()).collect();

    // Use clankers-merge graggle algorithm
    let base_graggle = clankers_merge::Graggle::from_text(&base_content);
    let result = clankers_merge::merge(&base_graggle, &branch_refs);

    if result.output.has_conflicts {
        Ok(FileMergeResult::Conflict {
            content: result.output.content.clone(),
            num_conflicts: result.output.content.matches("<<<<<<<").count(),
        })
    } else {
        Ok(FileMergeResult::Clean {
            content: result.output.content,
        })
    }
}

/// Apply a trivial branch merge (no overlapping files with other branches).
/// Uses in-process git2 merge operations.
pub fn apply_trivial_merge(repo_root: &Path, branch: &str, _target: &str) -> Result<MergeResult> {
    let repo = git2::Repository::open(repo_root).map_err(|e| crate::error::Error::Worktree {
        message: format!("Failed to open repository: {}", e),
    })?;

    let (analysis, branch_commit, annotated) = setup_merge(&repo, branch)?;

    if analysis.is_up_to_date() {
        return Ok(MergeResult::Clean);
    }

    if analysis.is_fast_forward() {
        return execute_fast_forward(&repo, branch, &branch_commit);
    }

    execute_normal_merge(&repo, branch, &annotated, &branch_commit)
}

/// Resolve branch and perform merge analysis
fn setup_merge<'a>(
    repo: &'a git2::Repository,
    branch: &str,
) -> Result<(git2::MergeAnalysis, git2::Commit<'a>, git2::AnnotatedCommit<'a>)> {
    let branch_ref = repo
        .find_branch(branch, git2::BranchType::Local)
        .or_else(|_| {
            // Try as remote branch or raw ref
            repo.find_reference(&format!("refs/heads/{}", branch))
                .or_else(|_| repo.find_reference(branch))
                .map(git2::Branch::wrap)
        })
        .map_err(|e| crate::error::Error::Worktree {
            message: format!("Branch '{}' not found: {}", branch, e),
        })?;

    let branch_commit = branch_ref
        .get()
        .peel_to_commit()
        .map_err(|e| crate::error::Error::Worktree {
            message: format!("Failed to get commit for branch '{}': {}", branch, e),
        })?;

    let annotated = repo
        .reference_to_annotated_commit(branch_ref.get())
        .map_err(|e| crate::error::Error::Worktree {
            message: format!("Failed to create annotated commit: {}", e),
        })?;

    let (analysis, _pref) = repo
        .merge_analysis(&[&annotated])
        .map_err(|e| crate::error::Error::Worktree {
            message: format!("Merge analysis failed: {}", e),
        })?;

    Ok((analysis, branch_commit, annotated))
}

/// Execute a fast-forward merge
fn execute_fast_forward(
    repo: &git2::Repository,
    branch: &str,
    branch_commit: &git2::Commit,
) -> Result<MergeResult> {
    let mut head_ref = repo.head().map_err(|e| crate::error::Error::Worktree {
        message: format!("Failed to get HEAD: {}", e),
    })?;

    head_ref
        .set_target(branch_commit.id(), &format!("merge {}: Fast-forward", branch))
        .map_err(|e| crate::error::Error::Worktree {
            message: format!("Fast-forward failed: {}", e),
        })?;

    repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
        .map_err(|e| crate::error::Error::Worktree {
            message: format!("Checkout after fast-forward failed: {}", e),
        })?;

    Ok(MergeResult::Clean)
}

/// Execute a normal merge and create merge commit
fn execute_normal_merge(
    repo: &git2::Repository,
    branch: &str,
    annotated: &git2::AnnotatedCommit,
    branch_commit: &git2::Commit,
) -> Result<MergeResult> {
    let mut merge_opts = git2::MergeOptions::new();
    merge_opts.file_favor(git2::FileFavor::Normal);

    repo.merge(&[annotated], Some(&mut merge_opts), None)
        .map_err(|e| {
            let _ = repo.cleanup_state();
            crate::error::Error::Worktree {
                message: format!("Merge failed: {}", e),
            }
        })?;

    let mut index = repo.index().map_err(|e| crate::error::Error::Worktree {
        message: format!("Failed to get index: {}", e),
    })?;

    if index.has_conflicts() {
        abort_merge(repo)?;
        return Ok(MergeResult::NeedsHuman {
            conflicting_files: vec![],
        });
    }

    create_merge_commit(repo, branch, &mut index, branch_commit)?;

    Ok(MergeResult::Clean)
}

/// Abort merge and restore working tree
fn abort_merge(repo: &git2::Repository) -> Result<()> {
    repo.cleanup_state().map_err(|e| crate::error::Error::Worktree {
        message: format!("Failed to cleanup merge state: {}", e),
    })?;

    repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
        .map_err(|e| crate::error::Error::Worktree {
            message: format!("Failed to checkout HEAD after conflict: {}", e),
        })?;

    Ok(())
}

/// Create a merge commit
fn create_merge_commit(
    repo: &git2::Repository,
    branch: &str,
    index: &mut git2::Index,
    branch_commit: &git2::Commit,
) -> Result<()> {
    let sig = repo.signature().map_err(|e| crate::error::Error::Worktree {
        message: format!("Failed to get signature: {}", e),
    })?;

    let tree_id = index.write_tree().map_err(|e| crate::error::Error::Worktree {
        message: format!("Failed to write tree: {}", e),
    })?;

    let tree = repo.find_tree(tree_id).map_err(|e| crate::error::Error::Worktree {
        message: format!("Failed to find tree: {}", e),
    })?;

    let head_commit = repo
        .head()
        .and_then(|h| h.peel_to_commit())
        .map_err(|e| crate::error::Error::Worktree {
            message: format!("Failed to get HEAD commit: {}", e),
        })?;

    let message = format!("Merge branch '{}'", branch);
    repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        &message,
        &tree,
        &[&head_commit, branch_commit],
    )
    .map_err(|e| crate::error::Error::Worktree {
        message: format!("Failed to create merge commit: {}", e),
    })?;

    repo.cleanup_state().map_err(|e| crate::error::Error::Worktree {
        message: format!("Failed to cleanup merge state: {}", e),
    })?;

    Ok(())
}

/// Apply an overlapping merge using graggle algorithm for conflicting files.
pub fn apply_graggle_merge(
    repo_root: &Path,
    branches: &[String],
    conflicting_files: &HashSet<PathBuf>,
    target: &str,
) -> Result<MergeResult> {
    let parent = target;
    let mut all_clean = true;
    let mut conflict_files = Vec::new();

    for file in conflicting_files {
        let result = merge_file(repo_root, file, parent, branches)?;
        match result {
            FileMergeResult::Clean { content } => {
                // Write merged content to working tree
                let full_path = repo_root.join(file);
                if let Some(dir) = full_path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                std::fs::write(&full_path, &content).map_err(|e| crate::error::Error::Worktree {
                    message: format!("Failed to write merged file {}: {}", file.display(), e),
                })?;
            }
            FileMergeResult::Conflict { content, .. } => {
                all_clean = false;
                conflict_files.push(file.clone());
                // Write content with conflict markers
                let full_path = repo_root.join(file);
                if let Some(dir) = full_path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                std::fs::write(&full_path, &content).map_err(|e| crate::error::Error::Worktree {
                    message: format!("Failed to write conflict file {}: {}", file.display(), e),
                })?;
            }
        }
    }

    if all_clean {
        Ok(MergeResult::Clean)
    } else {
        Ok(MergeResult::NeedsHuman {
            conflicting_files: conflict_files,
        })
    }
}

/// Ensure git rerere is enabled for the repo using in-process git2
pub fn ensure_rerere_enabled(repo_root: &Path) -> Result<()> {
    let repo = git2::Repository::open(repo_root).map_err(|e| crate::error::Error::Worktree {
        message: format!("Failed to open repository: {}", e),
    })?;
    let mut config = repo.config().map_err(|e| crate::error::Error::Worktree {
        message: format!("Failed to get config: {}", e),
    })?;
    config.set_bool("rerere.enabled", true).map_err(|e| crate::error::Error::Worktree {
        message: format!("Failed to enable rerere: {}", e),
    })?;
    Ok(())
}

/// Get file content from a specific git ref using in-process git2
fn git_show(repo_root: &Path, ref_name: &str, file_path: &Path) -> Option<String> {
    let repo = git2::Repository::open(repo_root).ok()?;
    let spec = format!("{}:{}", ref_name, file_path.display());
    let obj = repo.revparse_single(&spec).ok()?;
    let blob = obj.peel_to_blob().ok()?;
    std::str::from_utf8(blob.content()).ok().map(|s| s.to_string())
}
