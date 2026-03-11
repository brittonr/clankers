//! Script-based hook handler — runs user shell scripts from .clankers/hooks/

use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use tracing;

use crate::dispatcher::{HookHandler, PRIORITY_SCRIPT_HOOKS};
use crate::payload::HookPayload;
use crate::point::HookPoint;
use crate::verdict::HookVerdict;

/// Executes user scripts from a hooks directory.
pub struct ScriptHookHandler {
    hooks_dir: PathBuf,
    timeout: Duration,
}

impl ScriptHookHandler {
    pub fn new(hooks_dir: PathBuf, timeout: Duration) -> Self {
        Self { hooks_dir, timeout }
    }

    fn script_path(&self, point: HookPoint) -> PathBuf {
        self.hooks_dir.join(point.to_filename())
    }

    fn script_exists(&self, point: HookPoint) -> bool {
        let path = self.script_path(point);
        path.is_file() && is_executable(&path)
    }
}

#[async_trait]
impl HookHandler for ScriptHookHandler {
    fn name(&self) -> &str { "script" }
    fn priority(&self) -> u32 { PRIORITY_SCRIPT_HOOKS }

    fn subscribes_to(&self, point: HookPoint) -> bool {
        self.script_exists(point)
    }

    async fn handle(&self, point: HookPoint, payload: &HookPayload) -> HookVerdict {
        let script_path = self.script_path(point);
        if !script_path.is_file() {
            return HookVerdict::Continue;
        }

        let payload_json = match serde_json::to_string(payload) {
            Ok(j) => j,
            Err(e) => {
                tracing::warn!(hook = %point, error = %e, "failed to serialize hook payload");
                return HookVerdict::Continue;
            }
        };

        match run_script(&script_path, &payload_json, payload, self.timeout).await {
            Ok(output) => parse_script_output(point, &output),
            Err(e) => {
                tracing::warn!(hook = %point, error = %e, "hook script failed");
                if point.is_pre_hook() {
                    HookVerdict::Deny { reason: format!("hook script error: {e}") }
                } else {
                    HookVerdict::Continue
                }
            }
        }
    }
}

/// Output from a hook script.
struct ScriptOutput {
    exit_code: i32,
    stdout: String,
    stderr: String,
}

/// Run a hook script with the given payload on stdin.
async fn run_script(
    path: &Path,
    payload_json: &str,
    payload: &HookPayload,
    timeout: Duration,
) -> Result<ScriptOutput, String> {
    use tokio::process::Command;
    use tokio::io::AsyncWriteExt;

    let mut cmd = Command::new(path);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    // Set env vars
    cmd.env("CLANKERS_HOOK", payload.hook.as_str());
    cmd.env("CLANKERS_SESSION_ID", &payload.session_id);

    // Hook-specific env vars
    match &payload.data {
        crate::payload::HookData::Tool { tool_name, call_id, .. } => {
            cmd.env("CLANKERS_TOOL_NAME", tool_name);
            cmd.env("CLANKERS_CALL_ID", call_id);
        }
        crate::payload::HookData::Git { action, hash, message, .. } => {
            cmd.env("CLANKERS_GIT_ACTION", action);
            if let Some(h) = hash { cmd.env("CLANKERS_COMMIT_HASH", h); }
            if let Some(m) = message { cmd.env("CLANKERS_COMMIT_MESSAGE", m); }
        }
        crate::payload::HookData::Error { message, .. } => {
            cmd.env("CLANKERS_ERROR_MESSAGE", message);
        }
        crate::payload::HookData::ModelChange { from, to, reason } => {
            cmd.env("CLANKERS_MODEL_FROM", from);
            cmd.env("CLANKERS_MODEL_TO", to);
            cmd.env("CLANKERS_MODEL_CHANGE_REASON", reason);
        }
        _ => {}
    }

    let mut child = cmd.spawn().map_err(|e| format!("spawn: {e}"))?;

    // Write payload to stdin
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(payload_json.as_bytes()).await;
        drop(stdin);
    }

    // Wait with timeout
    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| format!("hook timed out after {}s", timeout.as_secs()))?
        .map_err(|e| format!("wait: {e}"))?;

    Ok(ScriptOutput {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

/// Parse script output into a verdict.
fn parse_script_output(point: HookPoint, output: &ScriptOutput) -> HookVerdict {
    if !output.stderr.is_empty() {
        tracing::debug!(hook = %point, stderr = %output.stderr.trim(), "hook stderr");
    }

    if output.exit_code != 0 {
        if point.is_pre_hook() {
            let reason = if output.stderr.trim().is_empty() {
                format!("hook exited with code {}", output.exit_code)
            } else {
                output.stderr.trim().to_string()
            };
            return HookVerdict::Deny { reason };
        }
        return HookVerdict::Continue;
    }

    // For pre-hooks, check if stdout contains JSON modifications
    if point.is_pre_hook() && !output.stdout.trim().is_empty()
        && let Ok(modified) = serde_json::from_str::<serde_json::Value>(output.stdout.trim()) {
            return HookVerdict::Modify(modified);
        }

    HookVerdict::Continue
}

/// Check if a path is executable (Unix).
#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    fn make_script(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }

    fn test_payload(hook: &str) -> HookPayload {
        HookPayload::empty(hook, "test-session")
    }

    #[tokio::test]
    async fn script_exit_0_continues() {
        let dir = TempDir::new().unwrap();
        make_script(dir.path(), "pre-tool", "#!/bin/sh\nexit 0\n");
        let handler = ScriptHookHandler::new(dir.path().to_path_buf(), Duration::from_secs(5));
        let v = handler.handle(HookPoint::PreTool, &test_payload("pre-tool")).await;
        assert!(matches!(v, HookVerdict::Continue));
    }

    #[tokio::test]
    async fn script_exit_1_denies_pre_hook() {
        let dir = TempDir::new().unwrap();
        make_script(dir.path(), "pre-tool", "#!/bin/sh\necho 'blocked' >&2\nexit 1\n");
        let handler = ScriptHookHandler::new(dir.path().to_path_buf(), Duration::from_secs(5));
        let v = handler.handle(HookPoint::PreTool, &test_payload("pre-tool")).await;
        match v {
            HookVerdict::Deny { reason } => assert_eq!(reason, "blocked"),
            _ => panic!("expected Deny"),
        }
    }

    #[tokio::test]
    async fn script_exit_1_post_hook_continues() {
        let dir = TempDir::new().unwrap();
        make_script(dir.path(), "post-tool", "#!/bin/sh\nexit 1\n");
        let handler = ScriptHookHandler::new(dir.path().to_path_buf(), Duration::from_secs(5));
        let v = handler.handle(HookPoint::PostTool, &test_payload("post-tool")).await;
        assert!(matches!(v, HookVerdict::Continue));
    }

    #[tokio::test]
    async fn script_stdout_json_modifies() {
        let dir = TempDir::new().unwrap();
        make_script(dir.path(), "pre-tool", "#!/bin/sh\necho '{\"modified\": true}'\n");
        let handler = ScriptHookHandler::new(dir.path().to_path_buf(), Duration::from_secs(5));
        let v = handler.handle(HookPoint::PreTool, &test_payload("pre-tool")).await;
        match v {
            HookVerdict::Modify(val) => assert_eq!(val["modified"], true),
            _ => panic!("expected Modify, got {:?}", v),
        }
    }

    #[tokio::test]
    async fn script_receives_payload_on_stdin() {
        let dir = TempDir::new().unwrap();
        let output_file = dir.path().join("received.json");
        let script = format!(
            "#!/bin/sh\ncat > {}\n",
            output_file.display()
        );
        make_script(dir.path(), "post-tool", &script);

        let handler = ScriptHookHandler::new(dir.path().to_path_buf(), Duration::from_secs(5));
        let payload = HookPayload::tool("post-tool", "s1", "bash", "c1", serde_json::json!({"command": "ls"}), None);
        handler.handle(HookPoint::PostTool, &payload).await;

        let content = fs::read_to_string(&output_file).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["hook"], "post-tool");
        assert_eq!(parsed["tool_name"], "bash");
    }

    #[tokio::test]
    async fn script_timeout_denies_pre_hook() {
        let dir = TempDir::new().unwrap();
        make_script(dir.path(), "pre-tool", "#!/bin/sh\nsleep 10\n");
        let handler = ScriptHookHandler::new(dir.path().to_path_buf(), Duration::from_millis(100));
        let v = handler.handle(HookPoint::PreTool, &test_payload("pre-tool")).await;
        assert!(matches!(v, HookVerdict::Deny { .. }));
    }

    #[tokio::test]
    async fn subscribes_only_when_script_exists() {
        let dir = TempDir::new().unwrap();
        let handler = ScriptHookHandler::new(dir.path().to_path_buf(), Duration::from_secs(5));
        assert!(!handler.subscribes_to(HookPoint::PreTool));

        make_script(dir.path(), "pre-tool", "#!/bin/sh\nexit 0\n");
        assert!(handler.subscribes_to(HookPoint::PreTool));
        assert!(!handler.subscribes_to(HookPoint::PostTool));
    }

    #[tokio::test]
    async fn env_vars_set_for_tool_hook() {
        let dir = TempDir::new().unwrap();
        let output_file = dir.path().join("env.txt");
        let script = format!(
            "#!/bin/sh\necho \"$CLANKERS_HOOK|$CLANKERS_TOOL_NAME|$CLANKERS_CALL_ID\" > {}\n",
            output_file.display()
        );
        make_script(dir.path(), "pre-tool", &script);

        let handler = ScriptHookHandler::new(dir.path().to_path_buf(), Duration::from_secs(5));
        let payload = HookPayload::tool("pre-tool", "s1", "bash", "call-42", serde_json::json!({}), None);
        handler.handle(HookPoint::PreTool, &payload).await;

        let content = fs::read_to_string(&output_file).unwrap();
        assert_eq!(content.trim(), "pre-tool|bash|call-42");
    }
}
