//! Git operations for experiment lifecycle.

use std::path::Path;
use std::process::Command;

pub fn create_branch(cwd: &Path, tag: &str) -> std::io::Result<()> {
    let branch = format!("autoresearch/{tag}");
    let status = Command::new("git").args(["checkout", "-b", &branch]).current_dir(cwd).status()?;
    if !status.success() {
        return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("git checkout -b {branch} failed")));
    }
    Ok(())
}

pub fn commit(cwd: &Path, message: &str) -> std::io::Result<String> {
    let status = Command::new("git").args(["add", "-A"]).current_dir(cwd).status()?;
    if !status.success() {
        return Err(std::io::Error::new(std::io::ErrorKind::Other, "git add -A failed"));
    }
    let status = Command::new("git").args(["commit", "-m", message, "--allow-empty"]).current_dir(cwd).status()?;
    if !status.success() {
        return Err(std::io::Error::new(std::io::ErrorKind::Other, "git commit failed"));
    }
    short_hash(cwd)
}

pub fn revert_preserving(cwd: &Path, preserve: &[&str]) -> std::io::Result<()> {
    // Stash preserved files
    for file in preserve {
        let path = cwd.join(file);
        if path.exists() {
            let backup = cwd.join(format!(".autoresearch-backup-{}", file.replace('/', "_")));
            std::fs::copy(&path, &backup)?;
        }
    }

    // Revert tracked changes.
    let status = Command::new("git").args(["checkout", "--", "."]).current_dir(cwd).status()?;
    if !status.success() {
        return Err(std::io::Error::new(std::io::ErrorKind::Other, "git checkout -- . failed"));
    }

    // Remove untracked experiment artifacts while keeping backups.
    let status = Command::new("git")
        .args(["clean", "-fd", "-e", ".autoresearch-backup-*"])
        .current_dir(cwd)
        .status()?;
    if !status.success() {
        return Err(std::io::Error::new(std::io::ErrorKind::Other, "git clean -fd failed"));
    }

    // Restore preserved files
    for file in preserve {
        let backup = cwd.join(format!(".autoresearch-backup-{}", file.replace('/', "_")));
        if backup.exists() {
            let target = cwd.join(file);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::rename(&backup, &target)?;
        }
    }

    Ok(())
}

pub fn short_hash(cwd: &Path) -> std::io::Result<String> {
    let output = Command::new("git").args(["rev-parse", "--short=7", "HEAD"]).current_dir(cwd).output()?;
    if !output.status.success() {
        return Err(std::io::Error::new(std::io::ErrorKind::Other, "git rev-parse failed"));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    fn init_git_repo(path: &Path) {
        Command::new("git").args(["init"]).current_dir(path).status().unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(path)
            .status()
            .unwrap();
        Command::new("git").args(["config", "user.name", "Test"]).current_dir(path).status().unwrap();
        std::fs::write(path.join("init.txt"), "init").unwrap();
        Command::new("git").args(["add", "-A"]).current_dir(path).status().unwrap();
        Command::new("git").args(["commit", "-m", "initial"]).current_dir(path).status().unwrap();
    }

    #[test]
    fn short_hash_returns_7_chars() {
        let tmp = tempfile::TempDir::new().unwrap();
        init_git_repo(tmp.path());
        let hash = short_hash(tmp.path()).unwrap();
        assert_eq!(hash.len(), 7);
    }

    #[test]
    fn commit_and_hash() {
        let tmp = tempfile::TempDir::new().unwrap();
        init_git_repo(tmp.path());

        std::fs::write(tmp.path().join("test.txt"), "data").unwrap();
        let hash = commit(tmp.path(), "test commit").unwrap();
        assert_eq!(hash.len(), 7);
    }

    #[test]
    fn revert_preserves_files() {
        let tmp = tempfile::TempDir::new().unwrap();
        init_git_repo(tmp.path());

        std::fs::write(tmp.path().join("code.rs"), "modified").unwrap();
        std::fs::write(tmp.path().join("autoresearch.jsonl"), "preserved").unwrap();

        revert_preserving(tmp.path(), &["autoresearch.jsonl"]).unwrap();

        // code.rs should be reverted (doesn't exist since it wasn't in initial commit)
        assert!(!tmp.path().join("code.rs").exists());
        // autoresearch.jsonl should be preserved
        assert_eq!(std::fs::read_to_string(tmp.path().join("autoresearch.jsonl")).unwrap(), "preserved");
    }
}
