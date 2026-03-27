use std::fs;

use git2::Oid;
use git2::Repository;
use git2::Signature;
use tempfile::TempDir;

use super::*;

/// Helper to create a test repository with initial commit
fn setup_test_repo() -> (TempDir, Repository) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo = Repository::init(temp_dir.path()).expect("Failed to init repo");

    // Create initial commit on main branch
    let sig = Signature::now("Test User", "test@example.com").unwrap();
    let tree_id = {
        let mut index = repo.index().unwrap();
        index.write_tree().unwrap()
    };

    {
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[]).unwrap();
    }

    (temp_dir, repo)
}

/// Helper to create a commit with a file change
fn create_commit(repo: &Repository, filename: &str, content: &str, message: &str) -> Oid {
    let sig = Signature::now("Test User", "test@example.com").unwrap();

    // Write file
    let file_path = repo.workdir().unwrap().join(filename);
    fs::write(&file_path, content).ok();

    // Stage file
    let mut index = repo.index().unwrap();
    index.add_path(Path::new(filename)).unwrap();
    index.write().ok();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();

    // Get parent commit
    let parent = repo.head().unwrap().peel_to_commit().unwrap();

    repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent]).unwrap()
}

#[test]
fn test_is_git_repo() {
    // Test with a git repository
    let (temp_dir, _repo) = setup_test_repo();
    assert!(is_git_repo(temp_dir.path()), "Should detect git repo");

    // Test with a plain directory
    let plain_dir = TempDir::new().unwrap();
    assert!(!is_git_repo(plain_dir.path()), "Should not detect non-repo");
}

#[test]
fn test_find_repo_root() {
    // Test from repo root
    let (temp_dir, _repo) = setup_test_repo();
    let root = find_repo_root(temp_dir.path());
    assert_eq!(root, Some(temp_dir.path().to_path_buf()), "Should find repo root");

    // Test from subdirectory
    let subdir = temp_dir.path().join("subdir");
    fs::create_dir(&subdir).unwrap();
    let root_from_subdir = find_repo_root(&subdir);
    assert_eq!(root_from_subdir, Some(temp_dir.path().to_path_buf()), "Should find repo root from subdirectory");

    // Test with non-repo directory
    let plain_dir = TempDir::new().unwrap();
    assert_eq!(find_repo_root(plain_dir.path()), None, "Should return None for non-repo");
}

#[test]
fn test_list_branches_empty_pattern() {
    let (temp_dir, repo) = setup_test_repo();

    // Fresh repo should have one branch (main or master)
    let branches = list_branches(temp_dir.path(), "*");
    assert_eq!(branches.len(), 1, "Fresh repo should have one branch");

    // Create additional branches
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    repo.branch("feature/test", &head, false).unwrap();
    repo.branch("bugfix/issue-123", &head, false).unwrap();

    // List all branches
    let all_branches = list_branches(temp_dir.path(), "*");
    assert_eq!(all_branches.len(), 3, "Should have 3 branches total");
    assert!(all_branches.iter().any(|b| b.starts_with("main") || b.starts_with("master")));
    assert!(all_branches.contains(&"feature/test".to_string()));
    assert!(all_branches.contains(&"bugfix/issue-123".to_string()));
}

#[test]
fn test_list_branches_with_pattern() {
    let (temp_dir, repo) = setup_test_repo();
    let head = repo.head().unwrap().peel_to_commit().unwrap();

    repo.branch("feature/test-1", &head, false).unwrap();
    repo.branch("feature/test-2", &head, false).unwrap();
    repo.branch("bugfix/issue-123", &head, false).unwrap();

    // List only feature branches
    let feature_branches = list_branches(temp_dir.path(), "feature/*");
    assert_eq!(feature_branches.len(), 2, "Should have 2 feature branches");
    assert!(feature_branches.contains(&"feature/test-1".to_string()));
    assert!(feature_branches.contains(&"feature/test-2".to_string()));

    // List only bugfix branches
    let bugfix_branches = list_branches(temp_dir.path(), "bugfix/*");
    assert_eq!(bugfix_branches.len(), 1, "Should have 1 bugfix branch");
    assert!(bugfix_branches.contains(&"bugfix/issue-123".to_string()));
}

#[test]
fn test_diff_name_only() {
    let (temp_dir, repo) = setup_test_repo();

    // Get first commit
    let first_commit = repo.head().unwrap().target().unwrap();

    // Create second commit with file changes
    create_commit(&repo, "file1.txt", "content1", "Add file1");
    let second_commit = repo.head().unwrap().target().unwrap();

    // Create third commit with more changes
    create_commit(&repo, "file2.txt", "content2", "Add file2");
    create_commit(&repo, "file3.txt", "content3", "Add file3");
    let third_commit = repo.head().unwrap().target().unwrap();

    // Test diff between first and second commit
    let diff1 = diff_name_only(temp_dir.path(), &first_commit.to_string(), &second_commit.to_string());
    assert!(diff1.is_some(), "Diff should succeed");
    let files1 = diff1.unwrap();
    assert_eq!(files1.len(), 1, "Should have 1 changed file");
    assert!(files1.contains(Path::new("file1.txt")));

    // Test diff between second and third commit
    let diff2 = diff_name_only(temp_dir.path(), &second_commit.to_string(), &third_commit.to_string());
    let files2 = diff2.unwrap();
    assert_eq!(files2.len(), 2, "Should have 2 changed files");
    assert!(files2.contains(Path::new("file2.txt")));
    assert!(files2.contains(Path::new("file3.txt")));

    // Test diff between first and third commit
    let diff3 = diff_name_only(temp_dir.path(), &first_commit.to_string(), &third_commit.to_string());
    let files3 = diff3.unwrap();
    assert_eq!(files3.len(), 3, "Should have 3 changed files total");
}

#[test]
fn test_diff_name_only_with_refs() {
    let (temp_dir, repo) = setup_test_repo();

    // Create commits on main
    create_commit(&repo, "main-file.txt", "main content", "Main commit");

    // Create a branch
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    let branch = repo.branch("test-branch", &head, false).unwrap();
    repo.set_head(branch.get().name().unwrap()).unwrap();
    repo.checkout_head(None).unwrap();

    // Create commits on branch
    create_commit(&repo, "branch-file.txt", "branch content", "Branch commit");

    // Test diff using branch names
    let diff = diff_name_only(temp_dir.path(), "main", "test-branch");
    assert!(diff.is_some(), "Diff should succeed with branch refs");
    let files = diff.unwrap();
    assert!(files.contains(Path::new("branch-file.txt")));
}

#[test]
fn test_list_merged_branches() {
    let (temp_dir, repo) = setup_test_repo();

    // Save the current HEAD
    let initial_head = repo.head().unwrap().peel_to_commit().unwrap();

    // Create a branch that will be merged (at current HEAD)
    repo.branch("merged-branch", &initial_head, false).unwrap();

    // Advance HEAD with a new commit
    create_commit(&repo, "main-file.txt", "main content", "Main commit");

    // Create an unmerged branch with divergent commits
    let unmerged_branch = repo.branch("unmerged-branch", &initial_head, false).unwrap();
    repo.set_head(unmerged_branch.get().name().unwrap()).unwrap();
    repo.checkout_head(None).unwrap();
    create_commit(&repo, "unmerged-file.txt", "unmerged content", "Unmerged commit");

    // Switch back to main
    repo.set_head("refs/heads/main").or_else(|_| repo.set_head("refs/heads/master")).unwrap();
    repo.checkout_head(None).unwrap();

    // List merged branches
    let merged = list_merged_branches(temp_dir.path(), "*");
    assert!(merged.contains("merged-branch"), "Should find merged branch");
    assert!(!merged.contains("unmerged-branch"), "Should not find unmerged branch");
}

#[test]
fn test_delete_branch() {
    let (temp_dir, repo) = setup_test_repo();

    // Create a branch
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    repo.branch("test-branch", &head, false).unwrap();

    // Verify branch exists
    let branches_before = list_branches(temp_dir.path(), "*");
    assert!(branches_before.contains(&"test-branch".to_string()));

    // Delete the branch
    let deleted = delete_branch(temp_dir.path(), "test-branch");
    assert!(deleted, "Branch deletion should succeed");

    // Verify branch is gone
    let branches_after = list_branches(temp_dir.path(), "*");
    assert!(!branches_after.contains(&"test-branch".to_string()));
}

#[test]
fn test_delete_branches_force() {
    let (temp_dir, repo) = setup_test_repo();

    // Create multiple branches
    let head = repo.head().unwrap().peel_to_commit().unwrap();
    repo.branch("branch-1", &head, false).unwrap();
    repo.branch("branch-2", &head, false).unwrap();
    repo.branch("branch-3", &head, false).unwrap();

    // Delete multiple branches
    let branches_to_delete = vec![
        "branch-1".to_string(),
        "branch-2".to_string(),
        "nonexistent".to_string(),
    ];

    let deleted_count = delete_branches_force(temp_dir.path(), &branches_to_delete);
    assert_eq!(deleted_count, 2, "Should delete 2 existing branches");

    // Verify branches are gone
    let remaining = list_branches(temp_dir.path(), "*");
    assert!(!remaining.contains(&"branch-1".to_string()));
    assert!(!remaining.contains(&"branch-2".to_string()));
    assert!(remaining.contains(&"branch-3".to_string()), "branch-3 should remain");
}

#[test]
fn test_dir_size_approx() {
    let temp_dir = TempDir::new().unwrap();

    // Empty directory should have size 0
    let empty_size = dir_size_approx(temp_dir.path());
    assert_eq!(empty_size, 0, "Empty directory should have size 0");

    // Create files
    let file1 = temp_dir.path().join("file1.txt");
    fs::write(&file1, "12345").unwrap(); // 5 bytes

    let subdir = temp_dir.path().join("subdir");
    fs::create_dir(&subdir).unwrap();
    let file2 = subdir.join("file2.txt");
    fs::write(&file2, "1234567890").unwrap(); // 10 bytes

    // Total should be 15 bytes
    let total_size = dir_size_approx(temp_dir.path());
    assert_eq!(total_size, 15, "Directory size should be 15 bytes");
}

#[test]
fn test_is_git_repo_with_nested_dirs() {
    let (temp_dir, _repo) = setup_test_repo();

    // Create nested subdirectories
    let level1 = temp_dir.path().join("level1");
    let level2 = level1.join("level2");
    fs::create_dir_all(&level2).unwrap();

    // All levels should detect the repo
    assert!(is_git_repo(&level1), "level1 should detect git repo");
    assert!(is_git_repo(&level2), "level2 should detect git repo");
}

#[test]
fn test_diff_name_only_invalid_refs() {
    let (temp_dir, _repo) = setup_test_repo();

    // Test with invalid refs
    let diff = diff_name_only(temp_dir.path(), "nonexistent", "HEAD");
    assert!(diff.is_none(), "Should return None for invalid ref");

    let diff2 = diff_name_only(temp_dir.path(), "HEAD", "invalid-ref");
    assert!(diff2.is_none(), "Should return None for invalid ref");
}
