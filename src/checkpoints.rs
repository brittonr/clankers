//! Working-directory checkpoint and rollback types.
//!
//! The first implementation is intentionally local and git-backed. It snapshots
//! git-tracked and non-ignored untracked files into `.git/clankers-checkpoints`
//! and restores from that local snapshot on rollback.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;

use crate::error::Error;
use crate::error::Result;

/// Local checkpoint backend supported by the first production slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointBackend {
    Git,
}

impl CheckpointBackend {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Git => "git",
        }
    }
}

/// User-facing checkpoint operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckpointOperation {
    Create { label: Option<String> },
    List,
    Rollback { checkpoint_id: String, confirmed: bool },
}

/// Durable checkpoint record stored beside the local git metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointRecord {
    pub id: String,
    pub label: Option<String>,
    pub repo_root: String,
    pub backend: String,
    pub created_at: String,
    pub changed_file_count: usize,
    pub files: Vec<String>,
}

/// Normalized operation output shared by CLI/tool/session callers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointOutcome {
    pub action: String,
    pub status: String,
    pub record: Option<CheckpointRecord>,
    pub records: Vec<CheckpointRecord>,
    pub details: CheckpointMetadata,
}

/// Normalized, replay-safe checkpoint metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointMetadata {
    pub action: String,
    pub status: String,
    pub backend: String,
    pub repo_root: String,
    pub checkpoint_id: Option<String>,
    pub changed_file_count: usize,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

impl CheckpointMetadata {
    pub fn success(
        action: &str,
        repo_root: impl Into<String>,
        checkpoint_id: Option<String>,
        changed_file_count: usize,
    ) -> Self {
        Self {
            action: action.to_string(),
            status: "success".to_string(),
            backend: CheckpointBackend::Git.as_str().to_string(),
            repo_root: repo_root.into(),
            checkpoint_id,
            changed_file_count,
            error_code: None,
            error_message: None,
        }
    }

    pub fn error(action: &str, repo_root: impl Into<String>, error_code: &str, error_message: &str) -> Self {
        Self {
            action: action.to_string(),
            status: "error".to_string(),
            backend: CheckpointBackend::Git.as_str().to_string(),
            repo_root: repo_root.into(),
            checkpoint_id: None,
            changed_file_count: 0,
            error_code: Some(error_code.to_string()),
            error_message: Some(sanitize_error_message(error_message)),
        }
    }

    pub fn to_details(&self) -> Value {
        json!(self)
    }
}

/// Create a local git-backed checkpoint for the repository containing `cwd`.
pub fn create_checkpoint(cwd: &Path, label: Option<String>) -> Result<CheckpointOutcome> {
    let repo_root = discover_repo_root(cwd)?;
    let files = snapshot_file_list(&repo_root)?;
    let id = format!("clankers-checkpoint-{}", Utc::now().timestamp_millis());
    let checkpoint_dir = checkpoint_dir(&repo_root, &id)?;
    let snapshot_dir = checkpoint_dir.join("snapshot");
    fs::create_dir_all(&snapshot_dir).map_err(|source| Error::Io { source })?;

    for file in &files {
        let source = repo_root.join(file);
        if source.is_file() {
            let destination = snapshot_dir.join(file);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|source| Error::Io { source })?;
            }
            fs::copy(&source, &destination).map_err(|source| Error::Io { source })?;
        }
    }

    let record = CheckpointRecord {
        id: id.clone(),
        label,
        repo_root: repo_root.display().to_string(),
        backend: CheckpointBackend::Git.as_str().to_string(),
        created_at: Utc::now().to_rfc3339(),
        changed_file_count: files.len(),
        files,
    };
    write_record(&checkpoint_dir, &record)?;

    Ok(CheckpointOutcome {
        action: "create".to_string(),
        status: "success".to_string(),
        record: Some(record.clone()),
        records: Vec::new(),
        details: CheckpointMetadata::success("create", &record.repo_root, Some(id), record.changed_file_count),
    })
}

/// List local clankers checkpoints for the repository containing `cwd`.
pub fn list_checkpoints(cwd: &Path) -> Result<CheckpointOutcome> {
    let repo_root = discover_repo_root(cwd)?;
    let root = checkpoints_root(&repo_root)?;
    let mut records = Vec::new();
    if root.exists() {
        for entry in fs::read_dir(root).map_err(|source| Error::Io { source })? {
            let entry = entry.map_err(|source| Error::Io { source })?;
            let metadata = entry.path().join("metadata.json");
            if metadata.is_file() {
                let text = fs::read_to_string(metadata).map_err(|source| Error::Io { source })?;
                records.push(serde_json::from_str(&text).map_err(|source| Error::Json { source })?);
            }
        }
    }
    records.sort_by(|a: &CheckpointRecord, b| b.created_at.cmp(&a.created_at));
    Ok(CheckpointOutcome {
        action: "list".to_string(),
        status: "success".to_string(),
        record: None,
        records,
        details: CheckpointMetadata::success("list", repo_root.display().to_string(), None, 0),
    })
}

/// Restore files from a local clankers checkpoint.
pub fn rollback_checkpoint(cwd: &Path, checkpoint_id: &str, confirmed: bool) -> Result<CheckpointOutcome> {
    if !is_clankers_checkpoint_id(checkpoint_id) {
        return Err(Error::Worktree {
            message: "checkpoint id is outside the clankers checkpoint namespace".to_string(),
        });
    }
    if !confirmed {
        return Err(Error::Worktree {
            message: "rollback requires explicit confirmation with --yes".to_string(),
        });
    }

    let repo_root = discover_repo_root(cwd)?;
    let local_id = checkpoint_id.rsplit('/').next().unwrap_or(checkpoint_id);
    let checkpoint_dir = checkpoint_dir(&repo_root, local_id)?;
    let record = read_record(&checkpoint_dir)?;
    let snapshot_dir = checkpoint_dir.join("snapshot");
    for file in &record.files {
        let source = snapshot_dir.join(file);
        if source.is_file() {
            let destination = repo_root.join(file);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|source| Error::Io { source })?;
            }
            fs::copy(&source, &destination).map_err(|source| Error::Io { source })?;
        }
    }

    Ok(CheckpointOutcome {
        action: "rollback".to_string(),
        status: "success".to_string(),
        record: Some(record.clone()),
        records: Vec::new(),
        details: CheckpointMetadata::success(
            "rollback",
            &record.repo_root,
            Some(record.id.clone()),
            record.changed_file_count,
        ),
    })
}

/// Return true only for checkpoint identifiers owned by clankers.
pub fn is_clankers_checkpoint_id(id: &str) -> bool {
    id.starts_with("refs/clankers/checkpoints/") || id.starts_with("clankers-checkpoint-")
}

/// Remove content-bearing fragments from errors before persistence/replay.
pub fn sanitize_error_message(message: &str) -> String {
    let flattened = message.replace('\n', " ");
    let mut chars = flattened.chars();
    let mut sanitized: String = chars.by_ref().take(240).collect();
    if chars.next().is_some() {
        sanitized.push('…');
    }
    sanitized
}

fn discover_repo_root(cwd: &Path) -> Result<PathBuf> {
    let output = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|source| Error::Io { source })?;
    if !output.status.success() {
        return Err(Error::Worktree {
            message: "not a git repository; run from a git checkout or pass --cwd".to_string(),
        });
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(PathBuf::from(stdout.trim()))
}

fn snapshot_file_list(repo_root: &Path) -> Result<Vec<String>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["ls-files", "--cached", "--others", "--exclude-standard", "-z"])
        .output()
        .map_err(|source| Error::Io { source })?;
    if !output.status.success() {
        return Err(Error::Worktree {
            message: "failed to list repository files for checkpoint".to_string(),
        });
    }
    Ok(output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|bytes| !bytes.is_empty())
        .filter_map(|bytes| String::from_utf8(bytes.to_vec()).ok())
        .collect())
}

fn git_dir(repo_root: &Path) -> Result<PathBuf> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["rev-parse", "--git-dir"])
        .output()
        .map_err(|source| Error::Io { source })?;
    if !output.status.success() {
        return Err(Error::Worktree {
            message: "failed to locate git metadata directory".to_string(),
        });
    }
    let git_dir = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim().to_string());
    if git_dir.is_absolute() {
        Ok(git_dir)
    } else {
        Ok(repo_root.join(git_dir))
    }
}

fn checkpoints_root(repo_root: &Path) -> Result<PathBuf> {
    Ok(git_dir(repo_root)?.join("clankers-checkpoints"))
}

fn checkpoint_dir(repo_root: &Path, checkpoint_id: &str) -> Result<PathBuf> {
    Ok(checkpoints_root(repo_root)?.join(checkpoint_id))
}

fn write_record(checkpoint_dir: &Path, record: &CheckpointRecord) -> Result<()> {
    let text = serde_json::to_string_pretty(record).map_err(|source| Error::Json { source })?;
    fs::write(checkpoint_dir.join("metadata.json"), text).map_err(|source| Error::Io { source })
}

fn read_record(checkpoint_dir: &Path) -> Result<CheckpointRecord> {
    let text = fs::read_to_string(checkpoint_dir.join("metadata.json")).map_err(|source| Error::Io { source })?;
    serde_json::from_str(&text).map_err(|source| Error::Json { source })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_clankers_checkpoint_namespace() {
        assert!(is_clankers_checkpoint_id("refs/clankers/checkpoints/abc123"));
        assert!(is_clankers_checkpoint_id("clankers-checkpoint-abc123"));
        assert!(!is_clankers_checkpoint_id("refs/heads/main"));
        assert!(!is_clankers_checkpoint_id("HEAD"));
    }

    #[test]
    fn details_use_safe_metadata_shape() {
        let details =
            CheckpointMetadata::success("create", "/repo", Some("refs/clankers/checkpoints/abc123".to_string()), 2)
                .to_details();

        assert_eq!(details["action"], "create");
        assert_eq!(details["status"], "success");
        assert_eq!(details["backend"], "git");
        assert_eq!(details["changed_file_count"], 2);
        assert!(details.get("diff").is_none());
        assert!(details.get("content").is_none());
        assert!(details.get("env").is_none());
    }

    #[test]
    fn sanitizes_multiline_and_long_errors() {
        let error = CheckpointMetadata::error("rollback", "/repo", "unsupported", &"secret\n".repeat(80));
        let message = error.error_message.expect("error message");
        assert!(!message.contains('\n'));
        assert!(message.chars().count() <= 241);
    }

    #[test]
    fn create_list_and_rollback_restore_file_contents() {
        let tmp = tempfile::tempdir().expect("tempdir");
        run_git(tmp.path(), &["init"]);
        fs::write(tmp.path().join("note.txt"), "before").expect("write fixture");
        run_git(tmp.path(), &["add", "note.txt"]);

        let created = create_checkpoint(tmp.path(), Some("before edit".to_string())).expect("create checkpoint");
        let record = created.record.expect("record");
        assert_eq!(record.changed_file_count, 1);
        assert_eq!(record.files, vec!["note.txt"]);

        fs::write(tmp.path().join("note.txt"), "after").expect("mutate fixture");
        let listed = list_checkpoints(tmp.path()).expect("list checkpoints");
        assert_eq!(listed.records.len(), 1);
        assert_eq!(listed.records[0].id, record.id);

        rollback_checkpoint(tmp.path(), &record.id, true).expect("rollback checkpoint");
        assert_eq!(fs::read_to_string(tmp.path().join("note.txt")).expect("read restored"), "before");
    }

    #[test]
    fn non_git_directory_returns_actionable_error() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let error = create_checkpoint(tmp.path(), None).expect_err("non-git should fail");
        assert!(error.to_string().contains("not a git repository"));
    }

    fn run_git(cwd: &Path, args: &[&str]) {
        let output = Command::new("git").arg("-C").arg(cwd).args(args).output().expect("run git");
        assert!(output.status.success(), "git failed: {}", String::from_utf8_lossy(&output.stderr));
    }
}
