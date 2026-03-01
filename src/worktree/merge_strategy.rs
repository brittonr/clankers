//! Merge strategy: graggle -> rerere -> LLM -> human
//!
//! Tiered merge resolution using clankers-merge's order-independent graggle algorithm.

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
/// Uses git merge --ff-only or git merge --no-ff.
pub fn apply_trivial_merge(repo_root: &Path, branch: &str, _target: &str) -> Result<MergeResult> {
    // Try fast-forward first
    let output = std::process::Command::new("git")
        .args(["merge", "--ff-only", branch])
        .current_dir(repo_root)
        .output()
        .map_err(|e| crate::error::Error::Worktree {
            message: format!("git merge failed: {}", e),
        })?;

    if output.status.success() {
        return Ok(MergeResult::Clean);
    }

    // If ff fails, try regular merge
    let output = std::process::Command::new("git")
        .args(["merge", "--no-ff", "-m", &format!("Merge branch '{}'", branch), branch])
        .current_dir(repo_root)
        .output()
        .map_err(|e| crate::error::Error::Worktree {
            message: format!("git merge failed: {}", e),
        })?;

    if output.status.success() {
        Ok(MergeResult::Clean)
    } else {
        // Abort the failed merge
        let _ = std::process::Command::new("git").args(["merge", "--abort"]).current_dir(repo_root).output();
        Ok(MergeResult::NeedsHuman {
            conflicting_files: vec![],
        })
    }
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

/// Ensure git rerere is enabled for the repo
pub fn ensure_rerere_enabled(repo_root: &Path) -> Result<()> {
    let output = std::process::Command::new("git")
        .args(["config", "rerere.enabled", "true"])
        .current_dir(repo_root)
        .output()
        .map_err(|e| crate::error::Error::Worktree {
            message: format!("Failed to enable rerere: {}", e),
        })?;
    if !output.status.success() {
        return Err(crate::error::Error::Worktree {
            message: "Failed to enable git rerere".to_string(),
        });
    }
    Ok(())
}

/// Get file content from a specific git ref
fn git_show(repo_root: &Path, ref_name: &str, file_path: &Path) -> Option<String> {
    let spec = format!("{}:{}", ref_name, file_path.display());
    let output = std::process::Command::new("git").args(["show", &spec]).current_dir(repo_root).output().ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        None
    }
}
