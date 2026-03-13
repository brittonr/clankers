//! LLM-powered merge conflict resolution
//!
//! When the graggle algorithm produces conflict markers, we ask the LLM to
//! resolve them. The LLM sees the base version, each branch's version, and
//! the conflict-marked output, then returns the resolved file.
//!
//! Uses in-process git2 operations instead of shelling out to git CLI.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::mpsc;
use tracing::info;
use tracing::warn;

use crate::provider::CompletionRequest;
use crate::provider::Provider;
use crate::provider::message::AgentMessage;
use crate::provider::message::Content;
use crate::provider::message::MessageId;
use crate::provider::message::UserMessage;
use crate::provider::streaming::StreamEvent;

/// Attempt to resolve a conflicted file using the LLM.
///
/// Returns `Some(resolved_content)` if the LLM produced a clean resolution,
/// `None` if it failed or still contains conflict markers.
pub async fn resolve_conflict(
    provider: &Arc<dyn Provider>,
    model: &str,
    repo_root: &Path,
    file_path: &Path,
    parent_branch: &str,
    branches: &[String],
    conflicted_content: &str,
) -> Option<String> {
    use std::fmt::Write;
    let base = git_show(repo_root, parent_branch, file_path).unwrap_or_default();

    let mut branch_versions = String::new();
    for branch in branches {
        let content = git_show(repo_root, branch, file_path).unwrap_or_default();
        let _ = write!(branch_versions, "\n--- Branch: {} ---\n{}\n", branch, content);
    }

    let prompt = format!(
        "You are a precise merge conflict resolver. Resolve the conflicts in the file below.\n\
        \n\
        RULES:\n\
        - Output ONLY the resolved file content, nothing else\n\
        - No markdown fences, no explanations, no commentary\n\
        - Preserve the intent of ALL branches — combine changes, don't pick one side\n\
        - If changes are to different parts of the file, include both\n\
        - If changes genuinely conflict (same line, different intent), use your best judgment to combine them\n\
        - The output must be valid, compilable code (if it's code)\n\
        - Do NOT include any conflict markers (<<<<<<, ======, >>>>>>)\n\
        \n\
        === BASE VERSION (parent: {parent_branch}) ===\n\
        {base}\n\
        \n\
        === BRANCH VERSIONS ==={branch_versions}\n\
        === CONFLICTED MERGE OUTPUT ===\n\
        {conflicted_content}\n\
        \n\
        Resolved file:"
    );

    let user_msg = AgentMessage::User(UserMessage {
        id: MessageId::generate(),
        content: vec![Content::Text { text: prompt }],
        timestamp: chrono::Utc::now(),
    });

    let request = CompletionRequest {
        model: model.to_string(),
        messages: vec![user_msg],
        system_prompt: Some(
            "You resolve merge conflicts. Output only the resolved file content. \
             No markdown fences, no explanations."
                .to_string(),
        ),
        max_tokens: Some(16384),
        temperature: Some(0.0),
        tools: vec![],
        thinking: None,
        no_cache: false,
        cache_ttl: None,
    };

    let (tx, mut rx) = mpsc::channel(64);
    let provider = provider.clone();
    let complete_handle = tokio::spawn(async move { provider.complete(request, tx).await });

    // Collect the full response text
    let mut resolved = String::new();
    while let Some(event) = rx.recv().await {
        if let StreamEvent::ContentBlockDelta {
            delta: crate::provider::streaming::ContentDelta::TextDelta { text },
            ..
        } = event
        {
            resolved.push_str(&text);
        }
    }

    if let Err(e) = complete_handle.await {
        warn!("LLM conflict resolution task failed: {}", e);
        return None;
    }

    let resolved = resolved.trim().to_string();

    // Validate: no conflict markers remain
    if resolved.contains("<<<<<<<") || resolved.contains(">>>>>>>") || resolved.is_empty() {
        warn!(
            file = %file_path.display(),
            "LLM resolution still contains conflict markers or is empty"
        );
        return None;
    }

    info!(file = %file_path.display(), "LLM resolved conflict successfully");
    Some(resolved)
}

/// Resolve all conflicted files in a batch.
///
/// Returns a list of files that were successfully resolved and a list that
/// still need human intervention.
pub async fn resolve_conflicts_batch(
    provider: &Arc<dyn Provider>,
    model: &str,
    repo_root: &Path,
    conflicting_files: &[PathBuf],
    parent_branch: &str,
    branches: &[String],
) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut resolved = Vec::new();
    let mut unresolved = Vec::new();

    for file in conflicting_files {
        // Read current conflicted content from working tree
        let full_path = repo_root.join(file);
        let conflicted = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => {
                unresolved.push(file.clone());
                continue;
            }
        };

        match resolve_conflict(provider, model, repo_root, file, parent_branch, branches, &conflicted).await {
            Some(content) => {
                // Write the resolved content
                if let Err(e) = std::fs::write(&full_path, &content) {
                    warn!(file = %file.display(), error = %e, "Failed to write resolved file");
                    unresolved.push(file.clone());
                } else {
                    resolved.push(file.clone());
                }
            }
            None => {
                unresolved.push(file.clone());
            }
        }
    }

    (resolved, unresolved)
}

/// Get file content from a specific git ref using in-process git2
fn git_show(repo_root: &Path, ref_name: &str, file_path: &Path) -> Option<String> {
    let repo = git2::Repository::open(repo_root).ok()?;
    let spec = format!("{}:{}", ref_name, file_path.display());
    let obj = repo.revparse_single(&spec).ok()?;
    let blob = obj.peel_to_blob().ok()?;
    std::str::from_utf8(blob.content()).ok().map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_show_nonexistent() {
        let tmp = tempfile::TempDir::new().expect("should create temp dir");
        assert!(git_show(tmp.path(), "main", Path::new("nope.txt")).is_none());
    }
}
