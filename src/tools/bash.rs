//! Shell command execution with live streaming

use async_trait::async_trait;
use regex::Regex;
use serde_json::Value;
use serde_json::json;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::time::Duration;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;
use crate::util::ansi::strip_ansi;

/// Confirmation channel for dangerous commands.
/// When set, the bash tool sends a request and waits for approval.
pub type ConfirmTx = tokio::sync::mpsc::UnboundedSender<(String, tokio::sync::oneshot::Sender<bool>)>;
pub type ConfirmRx = tokio::sync::mpsc::UnboundedReceiver<(String, tokio::sync::oneshot::Sender<bool>)>;

/// Create a confirmation channel pair.
pub fn confirm_channel() -> (ConfirmTx, ConfirmRx) {
    tokio::sync::mpsc::unbounded_channel()
}

/// Patterns that trigger a confirmation prompt before execution.
static DANGEROUS_PATTERNS: std::sync::LazyLock<Vec<(Regex, &'static str)>> = std::sync::LazyLock::new(|| {
    vec![
        (Regex::new(r"\brm\s+(-[a-zA-Z]*f[a-zA-Z]*\s+|.*-rf\b|.*--force\b)").unwrap(), "forced removal"),
        (Regex::new(r"\bsudo\s+rm\b").unwrap(), "sudo removal"),
        (Regex::new(r"(?i)\b(DROP|TRUNCATE|DELETE\s+FROM)\b").unwrap(), "destructive SQL"),
        (Regex::new(r"\bchmod\s+777\b").unwrap(), "world-writable permissions"),
        (Regex::new(r"\bmkfs\b").unwrap(), "filesystem format"),
        (Regex::new(r"\bdd\s+if=").unwrap(), "raw disk write"),
        (Regex::new(r">\s*/dev/sd[a-z]").unwrap(), "raw device write"),
        (Regex::new(r"\bgit\s+push\s+.*--force\b").unwrap(), "force push"),
        (Regex::new(r"\bgit\s+push\s+-f\b").unwrap(), "force push"),
        (Regex::new(r"\bgit\s+reset\s+--hard\b").unwrap(), "hard reset"),
        (Regex::new(r"\bgit\s+clean\s+-[a-zA-Z]*f").unwrap(), "forced clean"),
        (Regex::new(r"\bnix\s+profile\s+install\b").unwrap(), "nix profile install (use nix-shell instead)"),
    ]
});

/// Check whether a command matches a dangerous pattern.
/// Returns the reason string if dangerous, None if safe.
pub fn check_dangerous(command: &str) -> Option<&'static str> {
    for (re, reason) in DANGEROUS_PATTERNS.iter() {
        if re.is_match(command) {
            return Some(reason);
        }
    }
    None
}

pub struct BashTool {
    definition: ToolDefinition,
    confirm_tx: Option<ConfirmTx>,
}

impl BashTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "bash".to_string(),
                description: "Execute bash commands. Returns stdout + stderr combined, along with exit code. Output is truncated to 2000 lines or 50KB.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "Shell command to execute"
                        },
                        "timeout": {
                            "type": "number",
                            "description": "Timeout in seconds (optional, 0 = no timeout)"
                        }
                    },
                    "required": ["command"]
                }),
            },
            confirm_tx: None,
        }
    }

    /// Create a BashTool with a confirmation channel for dangerous commands.
    pub fn with_confirm(confirm_tx: ConfirmTx) -> Self {
        let mut tool = Self::new();
        tool.confirm_tx = Some(confirm_tx);
        tool
    }
}

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for BashTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        let command = match params.get("command").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return ToolResult::error("Missing required parameter: command"),
        };

        // Check for dangerous commands
        if let Some(reason) = check_dangerous(command) {
            if let Some(ref tx) = self.confirm_tx {
                let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
                let msg = format!("⚠️  Dangerous command detected ({}): {}", reason, command);
                if tx.send((msg, resp_tx)).is_ok() {
                    match resp_rx.await {
                        Ok(true) => { /* approved, continue */ }
                        _ => {
                            return ToolResult::error(format!(
                                "Command blocked by user ({}). Rephrase the command or ask the user for approval.",
                                reason
                            ));
                        }
                    }
                }
            } else {
                // No confirm channel (headless mode) — block with explanation
                return ToolResult::error(format!(
                    "⚠️  Dangerous command blocked ({}): {}\n\
                     This command was flagged as potentially destructive. \
                     In interactive mode, a confirmation prompt would appear. \
                     In headless mode, such commands are blocked by default.",
                    reason, command
                ));
            }
        }

        let timeout_secs = params.get("timeout").and_then(|v| v.as_u64()).unwrap_or(0);

        // Build sanitized environment (strips secrets, API keys, SSH agent, etc.)
        let clean_env = crate::tools::sandbox::sanitized_env();

        let mut cmd = Command::new("bash");
        cmd.arg("-c")
            .arg(command)
            .env_clear()
            .envs(clean_env)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        // On Linux: apply Landlock filesystem sandbox to the child process.
        // This restricts what files the shell command can access at the kernel
        // level, even if the command tries to read ~/.ssh or /etc/shadow.
        #[cfg(target_os = "linux")]
        {
            let cwd_for_landlock = std::env::current_dir().unwrap_or_default();
            unsafe {
                cmd.pre_exec(move || {
                    if let Err(e) = crate::tools::sandbox::apply_landlock_to_current(&cwd_for_landlock) {
                        tracing::warn!("sandbox: landlock on bash child failed: {}", e);
                        // Don't fail the command — degrade gracefully
                    }
                    Ok(())
                });
            }
        }

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => return ToolResult::error(format!("Failed to spawn bash: {}", e)),
        };

        let stdout = match child.stdout.take() {
            Some(s) => s,
            None => return ToolResult::error("Failed to capture stdout from child process"),
        };
        let stderr = match child.stderr.take() {
            Some(s) => s,
            None => return ToolResult::error("Failed to capture stderr from child process"),
        };

        // Stream both stdout and stderr line-by-line
        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();

        let mut collected = String::new();
        let mut line_count: usize = 0;

        let deadline = if timeout_secs > 0 {
            Some(tokio::time::Instant::now() + Duration::from_secs(timeout_secs))
        } else {
            None
        };

        // Read lines from both streams, emitting updates as we go
        loop {
            // Check timeout
            if let Some(dl) = deadline
                && tokio::time::Instant::now() >= dl
            {
                let _ = child.start_kill();
                return ToolResult::error(format!("Command timeout after {}s", timeout_secs));
            }

            tokio::select! {
                _ = ctx.signal.cancelled() => {
                    let _ = child.start_kill();
                    return ToolResult::error("Command cancelled");
                }
                line = stdout_reader.next_line() => {
                    match line {
                        Ok(Some(raw)) => {
                            let line = strip_ansi(&raw);
                            ctx.emit_progress(&line);
                            if !collected.is_empty() {
                                collected.push('\n');
                            }
                            collected.push_str(&line);
                            line_count += 1;
                        }
                        Ok(None) => {
                            // stdout closed — drain remaining stderr then break
                            while let Ok(Some(raw)) = stderr_reader.next_line().await {
                                let line = strip_ansi(&raw);
                                ctx.emit_progress(&line);
                                if !collected.is_empty() {
                                    collected.push('\n');
                                }
                                collected.push_str(&line);
                                line_count += 1;
                            }
                            break;
                        }
                        Err(e) => return ToolResult::error(format!("Read error: {}", e)),
                    }
                }
                line = stderr_reader.next_line() => {
                    match line {
                        Ok(Some(raw)) => {
                            let line = strip_ansi(&raw);
                            ctx.emit_progress(&line);
                            if !collected.is_empty() {
                                collected.push('\n');
                            }
                            collected.push_str(&line);
                            line_count += 1;
                        }
                        Ok(None) => {
                            // stderr closed — drain remaining stdout then break
                            while let Ok(Some(raw)) = stdout_reader.next_line().await {
                                let line = strip_ansi(&raw);
                                ctx.emit_progress(&line);
                                if !collected.is_empty() {
                                    collected.push('\n');
                                }
                                collected.push_str(&line);
                                line_count += 1;
                            }
                            break;
                        }
                        Err(e) => return ToolResult::error(format!("Read error: {}", e)),
                    }
                }
            }
        }

        let status = match child.wait().await {
            Ok(s) => s,
            Err(e) => return ToolResult::error(format!("Failed to wait for command: {}", e)),
        };

        // Apply truncation to final result
        const MAX_LINES: usize = 2000;
        const MAX_BYTES: usize = 50 * 1024;

        let _ = line_count; // used for streaming; truncation re-checks
        let (truncated_output, full_output_path) =
            crate::tools::truncation::truncate_tail(&collected, MAX_LINES, MAX_BYTES);

        let exit_code = status.code().unwrap_or(-1);

        let result_text = if exit_code == 0 {
            truncated_output
        } else {
            format!("Exit code: {}\n\n{}", exit_code, truncated_output)
        };

        let mut result = ToolResult::text(result_text);
        if let Some(path) = full_output_path {
            result.full_output_path = Some(path.display().to_string());
        }

        if exit_code != 0 {
            result.is_error = true;
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::check_dangerous;
    use crate::util::ansi::strip_ansi;

    #[test]
    fn strip_plain_text_unchanged() {
        assert_eq!(strip_ansi("hello world"), "hello world");
    }

    #[test]
    fn strip_csi_color_codes() {
        assert_eq!(strip_ansi("\x1b[32mOK\x1b[0m"), "OK");
        assert_eq!(strip_ansi("\x1b[1;31merror\x1b[0m: bad"), "error: bad");
    }

    #[test]
    fn strip_cursor_movement() {
        // CSI sequences like \x1b[2K (erase line), \x1b[1A (cursor up)
        assert_eq!(strip_ansi("\x1b[2Khello\x1b[1A"), "hello");
    }

    #[test]
    fn strip_osc_sequences() {
        // OSC title set: \x1b]0;title\x07
        assert_eq!(strip_ansi("\x1b]0;my title\x07content"), "content");
    }

    #[test]
    fn strip_carriage_returns() {
        assert_eq!(strip_ansi("progress\r100%"), "progress100%");
    }

    #[test]
    fn strip_complex_cargo_output() {
        let input = "\x1b[0m\x1b[1m\x1b[32m   Compiling\x1b[0m clankers v0.1.0";
        assert_eq!(strip_ansi(input), "   Compiling clankers v0.1.0");
    }

    // ── Dangerous command detection tests ─────────────────────────────

    #[test]
    fn dangerous_rm_rf() {
        assert!(check_dangerous("rm -rf /tmp/foo").is_some());
        assert!(check_dangerous("rm -f important.txt").is_some());
        assert!(check_dangerous("rm --force file").is_some());
    }

    #[test]
    fn dangerous_sudo_rm() {
        assert!(check_dangerous("sudo rm /etc/something").is_some());
    }

    #[test]
    fn dangerous_sql() {
        assert!(check_dangerous("psql -c 'DROP TABLE users'").is_some());
        assert!(check_dangerous("mysql -e 'TRUNCATE orders'").is_some());
        assert!(check_dangerous("sqlite3 db.sqlite 'DELETE FROM logs'").is_some());
    }

    #[test]
    fn dangerous_git() {
        assert!(check_dangerous("git push --force origin main").is_some());
        assert!(check_dangerous("git push -f").is_some());
        assert!(check_dangerous("git reset --hard HEAD~5").is_some());
        assert!(check_dangerous("git clean -fd").is_some());
    }

    #[test]
    fn dangerous_disk() {
        assert!(check_dangerous("mkfs.ext4 /dev/sda1").is_some());
        assert!(check_dangerous("dd if=/dev/zero of=/dev/sda").is_some());
        assert!(check_dangerous("chmod 777 /var/www").is_some());
    }

    #[test]
    fn dangerous_nix_profile_install() {
        assert!(check_dangerous("nix profile install nixpkgs#python3").is_some());
    }

    #[test]
    fn safe_commands_not_flagged() {
        assert!(check_dangerous("ls -la").is_none());
        assert!(check_dangerous("cat file.txt").is_none());
        assert!(check_dangerous("grep -r pattern .").is_none());
        assert!(check_dangerous("git push origin main").is_none());
        assert!(check_dangerous("git commit -m 'fix'").is_none());
        assert!(check_dangerous("rm file.txt").is_none()); // no -f
        assert!(check_dangerous("cargo build").is_none());
        assert!(check_dangerous("nix-shell -p python3 --run 'python3 script.py'").is_none());
        assert!(check_dangerous("nix run nixpkgs#hello").is_none());
    }
}
