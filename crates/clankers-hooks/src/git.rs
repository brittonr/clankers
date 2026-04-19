//! Git hook handler — runs standard .git/hooks/ scripts and manages shims.

use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use tracing;

use crate::dispatcher::HookHandler;
use crate::dispatcher::PRIORITY_GIT_HOOKS;
use crate::payload::HookPayload;
use crate::point::HookPoint;
use crate::verdict::HookVerdict;

/// Default timeout for git hooks.
const GIT_HOOK_TIMEOUT: Duration = Duration::from_secs(30);

/// Runs standard git hooks from .git/hooks/.
pub struct GitHookHandler {
    git_hooks_dir: PathBuf,
}

impl GitHookHandler {
    pub fn new(repo_root: PathBuf) -> Self {
        Self {
            git_hooks_dir: repo_root.join(".git").join("hooks"),
        }
    }

    /// Map a HookPoint to a git hook filename (if applicable).
    #[cfg_attr(
        dylint_lib = "tigerstyle",
        allow(catch_all_on_enum, reason = "default handler covers many variants uniformly")
    )]
    fn git_hook_name(point: HookPoint) -> Option<&'static str> {
        match point {
            HookPoint::PreCommit => Some("pre-commit"),
            HookPoint::PostCommit => Some("post-commit"),
            _ => None,
        }
    }

    fn hook_path(&self, point: HookPoint) -> Option<PathBuf> {
        Self::git_hook_name(point).map(|name| self.git_hooks_dir.join(name))
    }

    fn hook_exists(&self, point: HookPoint) -> bool {
        self.hook_path(point).map(|p| p.is_file() && is_executable(&p)).unwrap_or(false)
    }
}

#[async_trait]
impl HookHandler for GitHookHandler {
    fn name(&self) -> &str {
        "git"
    }
    fn priority(&self) -> u32 {
        PRIORITY_GIT_HOOKS
    }

    fn subscribes_to(&self, point: HookPoint) -> bool {
        self.hook_exists(point)
    }

    async fn handle(&self, point: HookPoint, _payload: &HookPayload) -> HookVerdict {
        let Some(hook_path) = self.hook_path(point) else {
            return HookVerdict::Continue;
        };

        if !hook_path.is_file() {
            return HookVerdict::Continue;
        }

        match run_git_hook(&hook_path).await {
            Ok(0) => HookVerdict::Continue,
            Ok(code) => {
                if point.is_pre_hook() {
                    HookVerdict::Deny {
                        reason: format!("git {} hook exited with code {code}", point.to_filename()),
                    }
                } else {
                    HookVerdict::Continue
                }
            }
            Err(e) => {
                tracing::warn!(hook = %point, error = %e, "git hook execution failed");
                if point.is_pre_hook() {
                    HookVerdict::Deny {
                        reason: format!("git hook error: {e}"),
                    }
                } else {
                    HookVerdict::Continue
                }
            }
        }
    }
}

/// Run a git hook script and return its exit code.
async fn run_git_hook(path: &Path) -> Result<i32, String> {
    use tokio::process::Command;

    let output = tokio::time::timeout(
        GIT_HOOK_TIMEOUT,
        Command::new(path)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output(),
    )
    .await
    .map_err(|_| format!("git hook timed out after {}s", GIT_HOOK_TIMEOUT.as_secs()))?
    .map_err(|e| format!("spawn: {e}"))?;

    if !output.stderr.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::debug!(path = %path.display(), stderr = %stderr.trim(), "git hook stderr");
    }

    Ok(output.status.code().unwrap_or(-1))
}

/// Install a clankers shim as a git hook.
///
/// Backs up existing hooks. The shim delegates to `clankers hook run <name>`.
pub fn install_hook_shim(repo_root: &Path, hook_name: &str) -> Result<(), String> {
    let hooks_dir = repo_root.join(".git").join("hooks");
    assert!(repo_root.is_dir());
    assert!(!hook_name.is_empty());
    std::fs::create_dir_all(&hooks_dir).map_err(|e| format!("create hooks dir: {e}"))?;

    let hook_path = hooks_dir.join(hook_name);

    // Back up existing hook
    if hook_path.exists() {
        let backup = hooks_dir.join(format!("{hook_name}.clankers-backup"));
        std::fs::rename(&hook_path, &backup).map_err(|e| format!("backup existing hook: {e}"))?;
        tracing::info!(hook = hook_name, backup = %backup.display(), "backed up existing git hook");
    }

    let shim = format!(
        "#!/bin/sh\n\
         # Managed by clankers — do not edit\n\
         # Original hook backed up to {hook_name}.clankers-backup (if it existed)\n\
         exec clankers hook run {hook_name} \"$@\"\n"
    );

    std::fs::write(&hook_path, &shim).map_err(|e| format!("write shim: {e}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook_path, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("chmod: {e}"))?;
    }

    assert!(hook_path.is_file());
    assert!(hook_path.starts_with(&hooks_dir));
    Ok(())
}

/// Remove a clankers shim and restore the backup if present.
pub fn uninstall_hook_shim(repo_root: &Path, hook_name: &str) -> Result<(), String> {
    let hooks_dir = repo_root.join(".git").join("hooks");
    let hook_path = hooks_dir.join(hook_name);
    let backup = hooks_dir.join(format!("{hook_name}.clankers-backup"));

    // Only remove if it's our shim
    if hook_path.is_file() {
        let content = std::fs::read_to_string(&hook_path).unwrap_or_default();
        if content.contains("Managed by clankers") {
            std::fs::remove_file(&hook_path).map_err(|e| format!("remove shim: {e}"))?;

            // Restore backup
            if backup.exists() {
                std::fs::rename(&backup, &hook_path).map_err(|e| format!("restore backup: {e}"))?;
                tracing::info!(hook = hook_name, "restored original git hook from backup");
            }
        }
    }

    Ok(())
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).map(|m| m.permissions().mode() & 0o111 != 0).unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn make_git_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join(".git/hooks")).unwrap();
        dir
    }

    fn make_hook(dir: &Path, name: &str, content: &str) {
        let path = dir.join(".git/hooks").join(name);
        fs::write(&path, content).ok();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    #[tokio::test]
    async fn pre_commit_exit_0() {
        let repo = make_git_repo();
        make_hook(repo.path(), "pre-commit", "#!/bin/sh\nexit 0\n");
        let handler = GitHookHandler::new(repo.path().to_path_buf());
        let payload = HookPayload::empty("pre-commit", "s1");
        let v = handler.handle(HookPoint::PreCommit, &payload).await;
        assert!(matches!(v, HookVerdict::Continue));
    }

    #[tokio::test]
    async fn pre_commit_exit_1_denies() {
        let repo = make_git_repo();
        make_hook(repo.path(), "pre-commit", "#!/bin/sh\nexit 1\n");
        let handler = GitHookHandler::new(repo.path().to_path_buf());
        let payload = HookPayload::empty("pre-commit", "s1");
        let v = handler.handle(HookPoint::PreCommit, &payload).await;
        assert!(matches!(v, HookVerdict::Deny { .. }));
    }

    #[tokio::test]
    async fn post_commit_exit_1_continues() {
        let repo = make_git_repo();
        make_hook(repo.path(), "post-commit", "#!/bin/sh\nexit 1\n");
        let handler = GitHookHandler::new(repo.path().to_path_buf());
        let payload = HookPayload::empty("post-commit", "s1");
        let v = handler.handle(HookPoint::PostCommit, &payload).await;
        assert!(matches!(v, HookVerdict::Continue));
    }

    #[test]
    fn install_and_uninstall_shim() {
        let repo = make_git_repo();
        install_hook_shim(repo.path(), "pre-commit").unwrap();

        let shim_path = repo.path().join(".git/hooks/pre-commit");
        assert!(shim_path.exists());
        let content = fs::read_to_string(&shim_path).unwrap();
        assert!(content.contains("Managed by clankers"));
        assert!(content.contains("clankers hook run pre-commit"));

        uninstall_hook_shim(repo.path(), "pre-commit").unwrap();
        assert!(!shim_path.exists());
    }

    #[test]
    fn install_backs_up_existing() {
        let repo = make_git_repo();
        make_hook(repo.path(), "pre-commit", "#!/bin/sh\necho original\n");

        install_hook_shim(repo.path(), "pre-commit").unwrap();

        let backup = repo.path().join(".git/hooks/pre-commit.clankers-backup");
        assert!(backup.exists());
        let backup_content = fs::read_to_string(&backup).unwrap();
        assert!(backup_content.contains("echo original"));

        // Uninstall restores backup
        uninstall_hook_shim(repo.path(), "pre-commit").unwrap();
        let restored = repo.path().join(".git/hooks/pre-commit");
        assert!(restored.exists());
        let restored_content = fs::read_to_string(&restored).unwrap();
        assert!(restored_content.contains("echo original"));
    }

    #[tokio::test]
    async fn no_hook_returns_continue() {
        let repo = make_git_repo();
        let handler = GitHookHandler::new(repo.path().to_path_buf());
        let payload = HookPayload::empty("pre-commit", "s1");
        let v = handler.handle(HookPoint::PreCommit, &payload).await;
        assert!(matches!(v, HookVerdict::Continue));
    }

    #[test]
    fn subscribes_only_for_git_hooks() {
        let repo = make_git_repo();
        let handler = GitHookHandler::new(repo.path().to_path_buf());
        // No hooks installed
        assert!(!handler.subscribes_to(HookPoint::PreCommit));
        assert!(!handler.subscribes_to(HookPoint::PreTool)); // never for non-git hooks
    }
}
