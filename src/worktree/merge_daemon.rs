//! Background merge daemon
//!
//! Watches the worktree registry for completed sessions and merges their
//! branches back to the parent. Uses conflict_graph for smart ordering,
//! merge_strategy for graggle-based resolution, and LLM for conflict
//! resolution when graggle can't auto-merge.
//!
//! Uses in-process git2 operations instead of shelling out to git CLI.

use std::path::PathBuf;
use std::sync::Arc;

use tracing::info;
use tracing::warn;

use super::WorktreeManager;
use super::WorktreeStatus;
use super::conflict_graph::BranchChangeset;
use super::conflict_graph::compute_merge_plan;
use super::merge_strategy;
use crate::db::Db;
use crate::error::Result;
use crate::provider::Provider;

/// Merge daemon state
pub struct MergeDaemon {
    repo_root: PathBuf,
    /// Optional LLM provider for conflict resolution
    provider: Option<Arc<dyn Provider>>,
    /// Model to use for conflict resolution
    model: String,
}

impl MergeDaemon {
    /// Create a merge daemon without LLM support (graggle + rerere only)
    pub fn new(repo_root: PathBuf) -> Self {
        Self {
            repo_root,
            provider: None,
            model: String::new(),
        }
    }

    /// Create a merge daemon with LLM-powered conflict resolution
    pub fn with_llm(repo_root: PathBuf, provider: Arc<dyn Provider>, model: String) -> Self {
        Self {
            repo_root,
            provider: Some(provider),
            model,
        }
    }

    /// Run one merge cycle: check registry, merge completed branches.
    ///
    /// Returns the number of branches successfully merged.
    pub async fn run_cycle(&self, db: &Db) -> Result<usize> {
        let reg = db.worktrees();
        let completed = reg.completed()?;

        if completed.is_empty() {
            return Ok(0);
        }

        info!(count = completed.len(), "merge daemon: processing completed branches");

        // Ensure rerere is enabled
        merge_strategy::ensure_rerere_enabled(&self.repo_root)?;

        // Build merge plan
        let plan = self.build_merge_plan(&completed)?;

        let mut merged_count = 0;

        // Process each category of branches
        merged_count += self.process_empty_branches(db, &plan.empty)?;
        merged_count += self.process_trivial_branches(db, &completed, &plan.trivial)?;
        merged_count += self.process_overlapping_groups(db, &completed, &plan.overlapping).await?;

        Ok(merged_count)
    }

    /// Build changesets and compute merge plan from completed worktrees
    fn build_merge_plan(
        &self,
        completed: &[super::WorktreeInfo],
    ) -> Result<super::conflict_graph::MergePlan> {
        let changesets: Vec<BranchChangeset> = completed
            .iter()
            .filter_map(|w| BranchChangeset::from_git(&self.repo_root, &w.branch, &w.parent_branch))
            .collect();

        Ok(compute_merge_plan(&changesets))
    }

    /// Process empty branches (no commits ahead of parent)
    fn process_empty_branches(&self, db: &Db, empty_branches: &[String]) -> Result<usize> {
        let reg = db.worktrees();
        let mut count = 0;

        for branch in empty_branches {
            info!(branch, "skipping empty branch (no commits ahead)");
            let manager = WorktreeManager::new(self.repo_root.clone());
            if let Err(e) = manager.remove_worktree(branch) {
                warn!(branch, error = %e, "failed to clean up empty branch");
            }
            let _ = reg.remove(branch);
            count += 1;
        }

        Ok(count)
    }

    /// Process trivial branches (non-overlapping files with other branches)
    fn process_trivial_branches(
        &self,
        db: &Db,
        completed: &[super::WorktreeInfo],
        trivial_branches: &[String],
    ) -> Result<usize> {
        let reg = db.worktrees();
        let mut count = 0;

        for branch in trivial_branches {
            info!(branch, "merging trivial branch (no file overlaps)");
            let target = completed
                .iter()
                .find(|w| w.branch == *branch)
                .map(|w| w.parent_branch.as_str())
                .unwrap_or("main");

            match merge_strategy::apply_trivial_merge(&self.repo_root, branch, target) {
                Ok(merge_strategy::MergeResult::Clean) => {
                    info!(branch, "trivial merge successful");
                    let manager = WorktreeManager::new(self.repo_root.clone());
                    if let Err(e) = manager.remove_worktree(branch) {
                        warn!(branch, error = %e, "failed to clean up after merge");
                    }
                    let _ = reg.remove(branch);
                    count += 1;
                }
                Ok(merge_strategy::MergeResult::NeedsHuman { .. }) => {
                    warn!(branch, "trivial merge has conflicts — keeping worktree for manual resolution");
                }
                Err(e) => {
                    warn!(branch, error = %e, "merge failed");
                }
            }
        }

        Ok(count)
    }

    /// Process overlapping groups with graggle merge + LLM fallback
    async fn process_overlapping_groups(
        &self,
        db: &Db,
        completed: &[super::WorktreeInfo],
        overlapping_groups: &[super::conflict_graph::OverlapGroup],
    ) -> Result<usize> {
        let mut count = 0;

        for group in overlapping_groups {
            info!(
                branches = ?group.branches,
                files = ?group.conflicting_files,
                "merging overlapping group with graggle algorithm"
            );

            let target = completed
                .iter()
                .find(|w| group.branches.contains(&w.branch))
                .map(|w| w.parent_branch.as_str())
                .unwrap_or("main");

            match merge_strategy::apply_graggle_merge(
                &self.repo_root,
                &group.branches,
                &group.conflicting_files,
                target,
            ) {
                Ok(merge_strategy::MergeResult::Clean) => {
                    self.commit_and_cleanup(db, &group.branches, "graggle merge clean")?;
                    count += group.branches.len();
                }
                Ok(merge_strategy::MergeResult::NeedsHuman { conflicting_files }) => {
                    count += self.try_llm_resolution(db, &group.branches, &conflicting_files, target).await?;
                }
                Err(e) => {
                    warn!(error = %e, "graggle merge failed");
                }
            }
        }

        Ok(count)
    }

    /// Try LLM-based conflict resolution, fall back to marking for human review
    async fn try_llm_resolution(
        &self,
        db: &Db,
        branches: &[String],
        conflicting_files: &[PathBuf],
        target: &str,
    ) -> Result<usize> {
        if let Some(ref provider) = self.provider {
            info!(
                files = ?conflicting_files,
                "attempting LLM conflict resolution"
            );

            let (resolved, unresolved) = super::llm_resolver::resolve_conflicts_batch(
                provider,
                &self.model,
                &self.repo_root,
                conflicting_files,
                target,
                branches,
            )
            .await;

            if !resolved.is_empty() {
                info!(count = resolved.len(), "LLM resolved conflicts");
            }

            if unresolved.is_empty() {
                // All conflicts resolved by LLM
                self.commit_and_cleanup(db, branches, "graggle + LLM merge clean")?;
                return Ok(branches.len());
            }
            warn!(?unresolved, "LLM could not resolve all conflicts — needs human review");
            self.mark_needs_review(db, branches)?;
        } else {
            warn!(
                ?conflicting_files,
                "graggle merge has conflicts and no LLM available — needs human review"
            );
            self.mark_needs_review(db, branches)?;
        }

        Ok(0)
    }

    /// Commit merged files and clean up worktrees using in-process git2
    fn commit_and_cleanup(&self, db: &Db, branches: &[String], label: &str) -> Result<()> {
        info!(label, "committing merged files");

        let repo = git2::Repository::open(&self.repo_root).map_err(|e| crate::error::Error::Worktree {
            message: format!("Failed to open repository: {}", e),
        })?;

        // Stage all changes (git add -A)
        let mut index = repo.index().map_err(|e| crate::error::Error::Worktree {
            message: format!("Failed to get index: {}", e),
        })?;

        index
            .add_all(["."].iter(), git2::IndexAddOption::DEFAULT, None)
            .map_err(|e| crate::error::Error::Worktree {
                message: format!("Failed to add files: {}", e),
            })?;

        index.write().map_err(|e| crate::error::Error::Worktree {
            message: format!("Failed to write index: {}", e),
        })?;

        // Create commit
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

        let message = format!("Merge branches: {}", branches.join(", "));
        repo.commit(Some("HEAD"), &sig, &sig, &message, &tree, &[&head_commit])
            .map_err(|e| crate::error::Error::Worktree {
                message: format!("Failed to create commit: {}", e),
            })?;

        // Clean up worktrees
        let manager = WorktreeManager::new(self.repo_root.clone());
        let reg = db.worktrees();
        for branch in branches {
            if let Err(e) = manager.remove_worktree(branch) {
                warn!(branch, error = %e, "failed to clean up after merge");
            }
            let _ = reg.remove(branch);
        }
        Ok(())
    }

    /// Mark branches as stale (needs human review) in the registry
    fn mark_needs_review(&self, db: &Db, branches: &[String]) -> Result<()> {
        let reg = db.worktrees();
        for branch in branches {
            let _ = reg.set_status(branch, WorktreeStatus::Stale);
        }
        Ok(())
    }
}

/// Spawn a polling merge daemon that runs continuously.
///
/// Used by the `clankers merge-daemon` CLI command.
pub async fn run_polling(
    db: Db,
    repo_root: PathBuf,
    interval_secs: u64,
    once: bool,
    provider: Option<Arc<dyn Provider>>,
    model: String,
) {
    let daemon = match provider {
        Some(p) => MergeDaemon::with_llm(repo_root, p, model),
        None => MergeDaemon::new(repo_root),
    };

    if once {
        match daemon.run_cycle(&db).await {
            Ok(n) => println!("Merge cycle complete: {} branches merged.", n),
            Err(e) => {
                eprintln!("Merge cycle failed: {}", e);
            }
        }
        return;
    }

    println!("Merge daemon started (polling every {}s). Press Ctrl+C to stop.", interval_secs);
    loop {
        match daemon.run_cycle(&db).await {
            Ok(0) => {} // Nothing to merge
            Ok(n) => println!("[merge-daemon] Merged {} branch(es).", n),
            Err(e) => eprintln!("[merge-daemon] Cycle error: {}", e),
        }
        tokio::select! {
            () = tokio::time::sleep(std::time::Duration::from_secs(interval_secs)) => {}
            _ = tokio::signal::ctrl_c() => {
                println!("\nMerge daemon stopped.");
                break;
            }
        }
    }
}
