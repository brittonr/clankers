//! File-level conflict graph analysis + merge ordering

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

/// Files changed by a single branch
#[derive(Debug, Clone)]
pub struct BranchChangeset {
    pub branch: String,
    pub parent_branch: String,
    pub changed_files: HashSet<PathBuf>,
}

impl BranchChangeset {
    /// Get changed files for a branch relative to its parent via `git diff --name-only`
    pub fn from_git(repo_root: &Path, branch: &str, parent: &str) -> Option<Self> {
        let output = std::process::Command::new("git")
            .args(["diff", "--name-only", parent, branch])
            .current_dir(repo_root)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let files: HashSet<PathBuf> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|l| !l.is_empty())
            .map(PathBuf::from)
            .collect();
        Some(Self {
            branch: branch.to_string(),
            parent_branch: parent.to_string(),
            changed_files: files,
        })
    }

    /// Check if this branch has zero commits ahead of parent
    pub fn is_empty(&self) -> bool {
        self.changed_files.is_empty()
    }
}

/// Plan for merging multiple branches
#[derive(Debug)]
pub struct MergePlan {
    /// Branches that touch unique files (can merge in any order, no conflicts possible)
    pub trivial: Vec<String>,
    /// Groups of branches that touch overlapping files (need graggle merge)
    pub overlapping: Vec<OverlapGroup>,
    /// Branches with no changes (skip)
    pub empty: Vec<String>,
}

/// A group of branches that share at least one changed file
#[derive(Debug)]
pub struct OverlapGroup {
    pub branches: Vec<String>,
    /// Files touched by more than one branch in this group
    pub conflicting_files: HashSet<PathBuf>,
}

/// Compute a merge plan from a set of branch changesets.
///
/// 1. Identify branches with no changes -> skip
/// 2. Build file -> branches index
/// 3. Find connected components (branches sharing files)
/// 4. Single-branch components -> trivial
/// 5. Multi-branch components -> overlapping (need graggle merge per file)
pub fn compute_merge_plan(changesets: &[BranchChangeset]) -> MergePlan {
    let mut empty = Vec::new();
    let mut active_changesets = Vec::new();

    // Separate empty branches
    for cs in changesets {
        if cs.is_empty() {
            empty.push(cs.branch.clone());
        } else {
            active_changesets.push(cs);
        }
    }

    // Build file -> branch index
    let mut file_to_branches: HashMap<&PathBuf, Vec<usize>> = HashMap::new();
    for (i, cs) in active_changesets.iter().enumerate() {
        for file in &cs.changed_files {
            file_to_branches.entry(file).or_default().push(i);
        }
    }

    // Union-Find to group overlapping branches
    let n = active_changesets.len();
    let mut parent: Vec<usize> = (0..n).collect();

    fn find(parent: &mut [usize], i: usize) -> usize {
        if parent[i] != i {
            parent[i] = find(parent, parent[i]);
        }
        parent[i]
    }
    fn union(parent: &mut [usize], a: usize, b: usize) {
        let ra = find(parent, a);
        let rb = find(parent, b);
        if ra != rb {
            parent[ra] = rb;
        }
    }

    // Union branches that share files
    for indices in file_to_branches.values() {
        if indices.len() > 1 {
            for i in 1..indices.len() {
                union(&mut parent, indices[0], indices[i]);
            }
        }
    }

    // Group by root
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..n {
        let root = find(&mut parent, i);
        groups.entry(root).or_default().push(i);
    }

    let mut trivial = Vec::new();
    let mut overlapping = Vec::new();

    for members in groups.values() {
        if members.len() == 1 {
            trivial.push(active_changesets[members[0]].branch.clone());
        } else {
            // Find conflicting files (touched by >1 branch in group)
            let branch_names: Vec<String> = members.iter().map(|&i| active_changesets[i].branch.clone()).collect();
            let all_files: HashSet<&PathBuf> =
                members.iter().flat_map(|&i| &active_changesets[i].changed_files).collect();
            let mut conflicting = HashSet::new();
            for file in all_files {
                let count = members.iter().filter(|&&i| active_changesets[i].changed_files.contains(file)).count();
                if count > 1 {
                    conflicting.insert((*file).clone());
                }
            }
            overlapping.push(OverlapGroup {
                branches: branch_names,
                conflicting_files: conflicting,
            });
        }
    }

    MergePlan {
        trivial,
        overlapping,
        empty,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_changesets() {
        let plan = compute_merge_plan(&[]);
        assert!(plan.trivial.is_empty());
        assert!(plan.overlapping.is_empty());
        assert!(plan.empty.is_empty());
    }

    #[test]
    fn test_all_trivial() {
        let changesets = vec![
            BranchChangeset {
                branch: "a".to_string(),
                parent_branch: "main".to_string(),
                changed_files: [PathBuf::from("file1.rs")].into_iter().collect(),
            },
            BranchChangeset {
                branch: "b".to_string(),
                parent_branch: "main".to_string(),
                changed_files: [PathBuf::from("file2.rs")].into_iter().collect(),
            },
        ];
        let plan = compute_merge_plan(&changesets);
        assert_eq!(plan.trivial.len(), 2);
        assert!(plan.overlapping.is_empty());
    }

    #[test]
    fn test_overlapping_branches() {
        let changesets = vec![
            BranchChangeset {
                branch: "a".to_string(),
                parent_branch: "main".to_string(),
                changed_files: [PathBuf::from("shared.rs")].into_iter().collect(),
            },
            BranchChangeset {
                branch: "b".to_string(),
                parent_branch: "main".to_string(),
                changed_files: [PathBuf::from("shared.rs")].into_iter().collect(),
            },
        ];
        let plan = compute_merge_plan(&changesets);
        assert!(plan.trivial.is_empty());
        assert_eq!(plan.overlapping.len(), 1);
        assert_eq!(plan.overlapping[0].branches.len(), 2);
        assert!(plan.overlapping[0].conflicting_files.contains(&PathBuf::from("shared.rs")));
    }

    #[test]
    fn test_mixed_trivial_and_overlapping() {
        let changesets = vec![
            BranchChangeset {
                branch: "a".to_string(),
                parent_branch: "main".to_string(),
                changed_files: [PathBuf::from("shared.rs")].into_iter().collect(),
            },
            BranchChangeset {
                branch: "b".to_string(),
                parent_branch: "main".to_string(),
                changed_files: [PathBuf::from("shared.rs"), PathBuf::from("b_only.rs")].into_iter().collect(),
            },
            BranchChangeset {
                branch: "c".to_string(),
                parent_branch: "main".to_string(),
                changed_files: [PathBuf::from("c_only.rs")].into_iter().collect(),
            },
        ];
        let plan = compute_merge_plan(&changesets);
        assert_eq!(plan.trivial.len(), 1); // c
        assert_eq!(plan.overlapping.len(), 1); // a + b
    }

    #[test]
    fn test_empty_branches() {
        let changesets = vec![BranchChangeset {
            branch: "empty".to_string(),
            parent_branch: "main".to_string(),
            changed_files: HashSet::new(),
        }];
        let plan = compute_merge_plan(&changesets);
        assert_eq!(plan.empty.len(), 1);
    }
}
