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
    assert!(log::format_relative_time(now - 30).contains("seconds"));
    assert!(log::format_relative_time(now - 120).contains("minute"));
    assert!(log::format_relative_time(now - 7200).contains("hour"));
    assert!(log::format_relative_time(now - 86400).contains("day"));
}
