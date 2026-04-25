//! Create worktree + branch for new session

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use chrono::Utc;

use super::SessionType;
use super::WorktreeInfo;
use super::WorktreeManager;
use super::WorktreeStatus;
use crate::error::Result;
use crate::tools::git_ops;
use crate::util::id::generate_id;

impl WorktreeManager {
    /// Create a new worktree for a session
    pub fn create_worktree(&self, session_type: &SessionType, parent_branch: Option<&str>) -> Result<WorktreeInfo> {
        let short_id = generate_id();
        let branch_name = match session_type {
            SessionType::Main => format!("clankers/main-{}", short_id),
            SessionType::Subagent { agent_name } => {
                format!("clankers/sub/{}-{}", agent_name, short_id)
            }
            SessionType::Worker { worker_name } => {
                format!("clankers/worker/{}-{}", worker_name, short_id)
            }
        };

        let parent = parent_branch.unwrap_or("HEAD");

        let worktree_path = self.repo_root.join(".git").join("clankers-worktrees").join(&branch_name);

        git_ops::sync::worktree_add(&self.repo_root, &branch_name, &worktree_path, parent)
            .map_err(|e| crate::error::Error::Worktree { message: e.to_string() })?;

        let agent_name = match session_type {
            SessionType::Main => "main".to_string(),
            SessionType::Subagent { agent_name } => agent_name.clone(),
            SessionType::Worker { worker_name } => worker_name.clone(),
        };

        Ok(WorktreeInfo {
            branch: branch_name,
            path: worktree_path,
            session_id: short_id,
            agent: agent_name,
            status: WorktreeStatus::Active,
            created_at: Utc::now(),
            parent_branch: parent.to_string(),
        })
    }
}
