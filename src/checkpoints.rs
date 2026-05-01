//! Working-directory checkpoint and rollback types.
//!
//! The first implementation is intentionally local and git-backed. This module
//! starts with policy and metadata helpers so CLI/tool/session callers share the
//! same safe shapes before backend wiring is added.

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;

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
}
